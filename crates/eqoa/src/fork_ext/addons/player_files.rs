//! Player-authorized native file selection and bounded copies, away from the frame.
use super::{
    host::Host,
    native::{bind, error},
    sandbox::Mounts,
};
use mlua::{Lua, Result, Value};
use serde_json::json;
use std::{
    cell::RefCell,
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
};
const CAP: usize = 16 * 1024 * 1024;
struct Completion {
    id: u64,
    kind: &'static str,
    result: std::result::Result<String, (&'static str, String)>,
}
struct Pending {
    kind: &'static str,
    rx: mpsc::Receiver<Completion>,
}
#[derive(Default)]
struct Service {
    next: u64,
    pending: Option<Pending>,
    permit: Option<Arc<AtomicBool>>,
}
impl Drop for Service {
    fn drop(&mut self) {
        if let Some(p) = &self.permit {
            p.store(false, Ordering::Release);
        }
    }
}
type Shared = Rc<RefCell<Service>>;
fn service(lua: &Lua) -> Shared {
    if let Some(s) = lua.app_data_ref::<Shared>() {
        return s.clone();
    }
    let s = Rc::new(RefCell::new(Service::default()));
    lua.set_app_data(s.clone());
    s
}
pub(crate) fn close(lua: &Lua) {
    drop(lua.remove_app_data::<Shared>());
}
pub(crate) fn poll(lua: &Lua) -> Result<()> {
    let Some(s) = lua.app_data_ref::<Shared>().map(|s| s.clone()) else {
        return Ok(());
    };
    let done = s
        .borrow()
        .pending
        .as_ref()
        .and_then(|r| match r.rx.try_recv() {
            Ok(v) => Some(v),
            Err(mpsc::TryRecvError::Disconnected) => Some(Completion {
                id: s.borrow().next,
                kind: r.kind,
                result: Err(("failed", "file worker disconnected".into())),
            }),
            Err(mpsc::TryRecvError::Empty) => None,
        });
    if let Some(done) = done {
        let mut s = s.borrow_mut();
        s.pending = None;
        s.permit = None;
        drop(s);
        let (data, failed) = match done.result {
            Ok(path) => (
                json!({"kind":done.kind,"request":done.id,"path":path}),
                false,
            ),
            Err((code, message)) => (
                json!({"kind":done.kind,"request":done.id,"error":{"code":code,"message":message,"capability":format!("eqoa.files.{}",done.kind)}}),
                true,
            ),
        };
        super::native_events::file_action(lua, data, failed)?;
    }
    Ok(())
}
fn context(lua: &Lua, kind: &str) -> std::result::Result<Mounts, (&'static str, &'static str)> {
    let m = lua
        .app_data_ref::<Mounts>()
        .map(|m| m.clone())
        .ok_or(("permission_denied", "file permission was not declared"))?;
    if !m.addon.extensions.permissions.iter().any(|p| p == kind) {
        return Err(("permission_denied", "file permission was not declared"));
    }
    if !super::exchange::has_player(lua) {
        return Err((
            "requires_player",
            "a player key or pointer activation is required",
        ));
    }
    Ok(m)
}
static REQUESTS: std::sync::Mutex<std::collections::BTreeSet<PathBuf>> =
    std::sync::Mutex::new(std::collections::BTreeSet::new());
struct RequestGate(PathBuf);
impl Drop for RequestGate {
    fn drop(&mut self) {
        REQUESTS.lock().unwrap().remove(&self.0);
    }
}
fn request_gate(mut owner: PathBuf) -> Option<RequestGate> {
    #[cfg(any(windows, target_os = "macos"))]
    {
        owner = PathBuf::from(owner.to_string_lossy().to_lowercase());
    }
    REQUESTS
        .lock()
        .unwrap()
        .insert(owner.clone())
        .then(|| RequestGate(owner))
}
fn start(
    lua: &Lua,
    kind: &'static str,
    owner: PathBuf,
    job: impl FnOnce(u64, Arc<AtomicBool>) -> std::result::Result<String, (&'static str, String)>
    + Send
    + 'static,
) -> Result<mlua::MultiValue> {
    let s = service(lua);
    if s.borrow().pending.is_some() {
        return error(
            lua,
            &format!("eqoa.files.{kind}"),
            "busy",
            "one import/export request at a time",
        );
    }
    let Some(gate) = request_gate(owner) else {
        return error(
            lua,
            &format!("eqoa.files.{kind}"),
            "busy",
            "this mod still has a dialog or file action open",
        );
    };
    s.borrow_mut().next += 1;
    let id = s.borrow().next;
    let permit = Arc::new(AtomicBool::new(true));
    let worker_permit = permit.clone();
    let (tx, rx) = mpsc::sync_channel(1);
    if !super::exchange::spend_player(lua, false) {
        return error(
            lua,
            &format!("eqoa.files.{kind}"),
            "requires_player",
            "player activation was already used",
        );
    }
    thread::Builder::new()
        .name("addon-file-action".into())
        .spawn(move || {
            let _gate = gate;
            let result = job(id, worker_permit);
            let _ = tx.send(Completion { id, kind, result });
        })
        .map_err(mlua::Error::external)?;
    let mut s = s.borrow_mut();
    s.pending = Some(Pending { kind, rx });
    s.permit = Some(permit);
    lua.pack_multi((id, Value::Nil))
}
static IMPORTS: std::sync::Mutex<()> = std::sync::Mutex::new(());
fn next_import(root: &Path) -> std::result::Result<u64, String> {
    let counter = root.join("import-sequence");
    let _guard = super::sandbox::lock_parents(&counter, true)?;
    super::file_io::no_links(&counter)?;
    let previous = if counter.exists() {
        let bytes = super::sandbox::read_bounded(&counter, 64)?;
        std::str::from_utf8(&bytes)
            .map_err(|e| e.to_string())?
            .trim()
            .parse::<u64>()
            .map_err(|e| e.to_string())?
    } else {
        0
    };
    let next = previous.checked_add(1).ok_or("import sequence exhausted")?;
    super::storage::atomic_replace(&counter, next.to_string().as_bytes())
        .map_err(|e| e.to_string())?;
    Ok(next)
}
fn prune_imports(dir: &Path) -> std::result::Result<(), String> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        super::file_io::no_links(&path)?;
        if !entry.file_type().map_err(|e| e.to_string())?.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if let Some(id) = name
            .strip_prefix("import-")
            .and_then(|s| s.strip_suffix(".json"))
            .and_then(|s| s.split(' ').next())
            .and_then(|s| s.parse::<u64>().ok())
        {
            let modified = entry.metadata().map_err(|e| e.to_string())?.modified().ok();
            files.push((id, modified, path));
        }
        if files.len() > 10000 {
            return Err("import staging entry limit exceeded".into());
        }
    }
    files.sort_by(|a, b| b.cmp(a));
    for (_, _, path) in files.into_iter().skip(3) {
        fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}
// The OS/player chose these endpoints. They may be cloud placeholders, links,
// redirected profiles or UNC paths; only the copy into private storage is sandboxed.
fn read_selected(path: &Path) -> std::result::Result<Vec<u8>, String> {
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(CAP as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > CAP {
        return Err("selected file exceeds 16 MiB".into());
    }
    Ok(bytes)
}
#[cfg(test)]
static COPY_PAUSE: std::sync::Mutex<Option<(PathBuf, mpsc::Sender<()>, mpsc::Receiver<()>)>> =
    std::sync::Mutex::new(None);
#[cfg(test)]
pub(super) fn pause_copy(dir: PathBuf) -> (mpsc::Receiver<()>, mpsc::Sender<()>) {
    let (entered, rx) = mpsc::channel();
    let (release, wait) = mpsc::channel();
    *COPY_PAUSE.lock().unwrap() = Some((dir, entered, wait));
    (rx, release)
}
#[cfg(test)]
fn paused_copy(dir: &Path) {
    let pause = {
        let mut p = COPY_PAUSE.lock().unwrap();
        if p.as_ref().is_some_and(|p| p.0 == dir) {
            p.take()
        } else {
            None
        }
    };
    if let Some((_, entered, release)) = pause {
        let _ = entered.send(());
        let _ = release.recv();
    }
}
#[cfg(test)]
pub(super) fn disconnect_worker(lua: &Lua) {
    let (tx, rx) = mpsc::sync_channel(1);
    drop(tx);
    service(lua)
        .borrow_mut()
        .pending
        .as_mut()
        .expect("file action pending")
        .rx = rx;
}
fn active(permit: &AtomicBool) -> std::result::Result<(), String> {
    if permit.load(Ordering::Acquire) {
        Ok(())
    } else {
        Err("file action cancelled because mod was disabled".into())
    }
}
// Publish a synced temporary file atomically, without replacing any destination.
#[cfg(windows)]
fn rename_new(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    fn wide(path: &Path) -> std::io::Result<Vec<u16>> {
        let path = std::path::absolute(path)?;
        let raw = path.as_os_str().encode_wide().collect::<Vec<_>>();
        let mut out = if raw.starts_with(&[92, 92, 63, 92]) {
            raw
        } else if raw.starts_with(&[92, 92]) {
            let mut v = vec![92, 92, 63, 92, 85, 78, 67, 92];
            v.extend_from_slice(&raw[2..]);
            v
        } else {
            let mut v = vec![92, 92, 63, 92];
            v.extend(raw);
            v
        };
        out.push(0);
        Ok(out)
    }
    let from = wide(from)?;
    let to = wide(to)?;
    // Both buffers are terminated and remain live; no REPLACE_EXISTING flag.
    let moved = unsafe {
        windows_sys::Win32::Storage::FileSystem::MoveFileExW(
            from.as_ptr(),
            to.as_ptr(),
            windows_sys::Win32::Storage::FileSystem::MOVEFILE_WRITE_THROUGH,
        )
    };
    if moved != 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}
#[cfg(not(windows))]
fn rename_new(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::hard_link(from, to)?;
    fs::remove_file(from)
}
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
fn unique_copy(
    dir: &Path,
    name: &str,
    bytes: &[u8],
    permit: &AtomicBool,
    quota_root: Option<&Path>,
) -> std::result::Result<PathBuf, String> {
    if quota_root.is_some() {
        super::file_io::no_links(dir)?;
    }
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let _guard = if quota_root.is_some() {
        Some(super::sandbox::lock_parents(&dir.join(name), false)?)
    } else {
        None
    };
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let (temporary, mut file) = loop {
        active(permit)?;
        let path = dir.join(format!(
            ".eqoa-file-{}-{}.tmp",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        match OpenOptions::new().create_new(true).write(true).open(&path) {
            Ok(file) => break (Temporary(path), file),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    };
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| e.to_string())?;
    drop(file);
    #[cfg(test)]
    paused_copy(dir);
    let source = Path::new(name);
    let stem = source
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("invalid file name")?;
    let ext = source
        .extension()
        .and_then(|s| s.to_str())
        .map(|e| format!(".{e}"))
        .unwrap_or_default();
    for n in 1..=10000 {
        active(permit)?;
        let path = dir.join(if n == 1 {
            name.to_owned()
        } else {
            format!("{stem} ({n}){ext}")
        });
        if let Some(root) = quota_root {
            super::file_io::no_links(&path)?;
            if !super::sandbox::import_path(root, &path) {
                super::sandbox::check_quota(root, &path, bytes.len())?;
            }
        }
        match rename_new(&temporary.0, &path) {
            Ok(()) => return Ok(path),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("export name collision limit exceeded".into())
}
#[cfg(test)]
#[derive(Clone)]
pub(super) struct TestIo {
    pub pick: Option<PathBuf>,
    pub desktop: PathBuf,
}
#[cfg(test)]
pub(super) struct BlockPick {
    pub entered: mpsc::Sender<()>,
    pub release: Rc<RefCell<Option<mpsc::Receiver<()>>>>,
}
#[cfg(test)]
fn pick(
    lua: &Lua,
    title: String,
    _ext: Vec<String>,
) -> Box<dyn FnOnce() -> Option<PathBuf> + Send> {
    let picked = lua.app_data_ref::<TestIo>().and_then(|t| t.pick.clone());
    lua.set_named_registry_value("player_file_picker_title", title)
        .unwrap();
    let blocked = lua.app_data_ref::<BlockPick>().and_then(|b| {
        b.release
            .borrow_mut()
            .take()
            .map(|r| (b.entered.clone(), r))
    });
    Box::new(move || {
        if let Some((entered, release)) = blocked {
            let _ = entered.send(());
            let _ = release.recv();
        }
        picked
    })
}
#[cfg(all(not(test), any(target_os = "linux", target_os = "macos", windows)))]
fn pick(
    _lua: &Lua,
    title: String,
    ext: Vec<String>,
) -> Box<dyn FnOnce() -> Option<PathBuf> + Send> {
    Box::new(move || {
        rfd::FileDialog::new()
            .set_title(title)
            .add_filter("Files", &ext)
            .pick_file()
    })
}
#[cfg(all(not(test), not(any(target_os = "linux", target_os = "macos", windows))))]
fn pick(
    _lua: &Lua,
    _title: String,
    _ext: Vec<String>,
) -> Box<dyn FnOnce() -> Option<PathBuf> + Send> {
    Box::new(|| None)
}
fn desktop(lua: &Lua) -> Option<PathBuf> {
    #[cfg(test)]
    {
        return lua.app_data_ref::<TestIo>().map(|t| t.desktop.clone());
    }
    #[cfg(not(test))]
    {
        let _ = lua;
        crate::fork_ext::report::desktop_dir()
    }
}
pub(crate) fn install(host: &Host) -> Result<()> {
    bind(
        &host.lua,
        "files",
        "import",
        host.lua.create_function(|lua, spec: mlua::Table| {
            let m = match context(lua, "import") {
                Ok(m) => m,
                Err((c, e)) => return error(lua, "eqoa.files.import", c, e),
            };
            let title = spec
                .get::<Option<String>>("title")?
                .unwrap_or_else(|| "Import file".into());
            let ext = spec.get::<Vec<String>>("extensions")?;
            if title.len() > 256
                || title.chars().any(char::is_control)
                || ext.is_empty()
                || ext.len() > 16
                || ext.iter().any(|e| {
                    e.is_empty() || e.len() > 16 || !e.bytes().all(|b| b.is_ascii_alphanumeric())
                })
            {
                return error(
                    lua,
                    "eqoa.files.import",
                    "invalid_argument",
                    "invalid title or extensions",
                );
            }
            let Some(root) = m.data else {
                return error(
                    lua,
                    "eqoa.files.import",
                    "unavailable",
                    "private storage unavailable",
                );
            };
            let title = format!("{}: {}", m.addon.name, title);
            let picker = pick(lua, title, ext);
            start(lua, "import", root.clone(), move |_id, permit| {
                let path = picker().ok_or(("cancelled", "file selection cancelled".into()))?;
                let run = || -> std::result::Result<String, String> {
                    active(&permit)?;
                    let bytes = read_selected(&path)?;
                    let dir = root.join("files/data/imports");
                    let _serial = IMPORTS.lock().unwrap();
                    let sequence = next_import(&root)?;
                    let name = format!("import-{sequence}.json");
                    let target = dir.join(&name);
                    let _guard = super::sandbox::lock_parents(&target, true)?;

                    let copied = unique_copy(&dir, &name, &bytes, &permit, Some(&root))?;
                    prune_imports(&dir)?;
                    Ok(format!(
                        "imports/{}",
                        copied.file_name().unwrap().to_string_lossy()
                    ))
                };
                run().map_err(|e| ("io_error", e))
            })
        })?,
    )?;
    bind(
        &host.lua,
        "files",
        "export",
        host.lua
            .create_function(|lua, (path, name): (String, String)| {
                let m = match context(lua, "export") {
                    Ok(m) => m,
                    Err((c, e)) => return error(lua, "eqoa.files.export", c, e),
                };
                let extension = Path::new(&name)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if name.starts_with('.')
                    || name.trim() != name
                    || !matches!(extension.as_str(), "json" | "txt" | "csv" | "png")
                    || name.is_empty()
                    || name.len() > 128
                    || name.contains(['/', '\\'])
                    || m.key(&format!("imports/{name}")).is_err()
                {
                    return error(
                        lua,
                        "eqoa.files.export",
                        "invalid_argument",
                        "file_name must be one safe file name",
                    );
                }
                let key = match m.key(&path) {
                    Ok(k) if k.starts_with("data") => k,
                    _ => {
                        return error(
                            lua,
                            "eqoa.files.export",
                            "permission_denied",
                            "exports require this mod's private file",
                        );
                    }
                };
                let fence = super::json_save::export_fence(lua, &key);
                let source = match m.read_path(&key) {
                    Ok(p) => p,
                    Err(_) if fence.is_some() => m.data.as_ref().unwrap().join("files").join(&key),
                    Err(e) => return error(lua, "eqoa.files.export", "not_found", &e),
                };
                let Some(dest) = desktop(lua) else {
                    return error(
                        lua,
                        "eqoa.files.export",
                        "unavailable",
                        "Desktop folder unavailable",
                    );
                };
                let Some(owner) = m.data.clone() else {
                    return error(
                        lua,
                        "eqoa.files.export",
                        "unavailable",
                        "private storage unavailable",
                    );
                };
                start(lua, "export", owner, move |_, permit| {
                    let run = || -> std::result::Result<String, String> {
                        active(&permit)?;
                        if let Some(fence) = fence {
                            fence.wait(&permit)?;
                        }
                        let bytes = super::sandbox::read_bounded(&source, CAP)?;
                        let dest = unique_copy(&dest, &name, &bytes, &permit, None)?;
                        Ok(dest.file_name().unwrap().to_string_lossy().into_owned())
                    };
                    run().map_err(|e| ("io_error", e))
                })
            })?,
    )
}
