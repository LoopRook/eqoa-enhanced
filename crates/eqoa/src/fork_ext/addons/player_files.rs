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
    io::Write,
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
#[derive(Default)]
struct Service {
    next: u64,
    pending: Option<mpsc::Receiver<Completion>>,
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
        .and_then(|r| match r.try_recv() {
            Ok(v) => Some(v),
            Err(mpsc::TryRecvError::Disconnected) => Some(Completion {
                id: s.borrow().next,
                kind: "file",
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
fn start(
    lua: &Lua,
    kind: &'static str,
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
            let result = job(id, worker_permit);
            let _ = tx.send(Completion { id, kind, result });
        })
        .map_err(mlua::Error::external)?;
    let mut s = s.borrow_mut();
    s.pending = Some(rx);
    s.permit = Some(permit);
    lua.pack_multi((id, Value::Nil))
}
fn active(permit: &AtomicBool) -> std::result::Result<(), String> {
    if permit.load(Ordering::Acquire) {
        Ok(())
    } else {
        Err("file action cancelled because mod was disabled".into())
    }
}
// CreateNew preserves every existing file even under concurrent exports.
fn unique_copy(
    dir: &Path,
    name: &str,
    bytes: &[u8],
    permit: &AtomicBool,
    quota_root: Option<&Path>,
) -> std::result::Result<PathBuf, String> {
    super::file_io::no_links(dir)?;
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
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
        let name = if n == 1 {
            name.to_owned()
        } else {
            format!("{stem} ({n}){ext}")
        };
        let path = dir.join(name);
        let _guard = super::sandbox::lock_parents(&path, false)?;
        super::file_io::no_links(&path)?;
        if let Some(root) = quota_root {
            super::sandbox::check_quota(root, &path, bytes.len())?;
        }
        match OpenOptions::new().create_new(true).write(true).open(&path) {
            Ok(mut f) => {
                let result = f
                    .write_all(bytes)
                    .and_then(|()| f.sync_all())
                    .map_err(|e| e.to_string())
                    .and_then(|()| active(permit));
                drop(f);
                if let Err(e) = result {
                    let _ = fs::remove_file(&path);
                    return Err(e);
                }
                return Ok(path);
            }
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
fn pick(
    lua: &Lua,
    _title: String,
    _ext: Vec<String>,
) -> Box<dyn FnOnce() -> Option<PathBuf> + Send> {
    let picked = lua.app_data_ref::<TestIo>().and_then(|t| t.pick.clone());
    Box::new(move || picked)
}
#[cfg(not(test))]
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
            let picker = pick(lua, title, ext);
            start(lua, "import", move |id, permit| {
                let path = picker().ok_or(("cancelled", "file selection cancelled".into()))?;
                let run = || -> std::result::Result<String, String> {
                    active(&permit)?;
                    let bytes = super::sandbox::read_bounded(&path, CAP)?;
                    let dir = root.join("files/data/imports");
                    let name = format!("import-{id}.json");
                    let target = dir.join(&name);
                    let _guard = super::sandbox::lock_parents(&target, true)?;
                    super::sandbox::check_quota(&root, &target, bytes.len())?;
                    let copied = unique_copy(&dir, &name, &bytes, &permit, Some(&root))?;
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
                let source = match m.read_path(&key) {
                    Ok(p) => p,
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
                start(lua, "export", move |_, permit| {
                    let run = || -> std::result::Result<String, String> {
                        active(&permit)?;
                        let bytes = super::sandbox::read_bounded(&source, CAP)?;
                        let dest = unique_copy(&dest, &name, &bytes, &permit, None)?;
                        Ok(dest.file_name().unwrap().to_string_lossy().into_owned())
                    };
                    run().map_err(|e| ("io_error", e))
                })
            })?,
    )
}
