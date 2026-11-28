// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Small Lua 5.1 text-file subset. Resource paths are a virtual overlay: reads
//! can import existing resources; all writes/deletions live in addon config.
//! No source package, other addon, or arbitrary host file is writable.
use super::{discovery::DiscoveredAddon, host::Host};
use mlua::{Result, UserData, UserDataMethods, Value};
use std::{
    cell::RefCell,
    fs,
    io::Read,
    path::{Path, PathBuf},
    rc::Rc,
};

const MAX_BYTES: usize = 1024 * 1024;

#[derive(Default)]
struct FileRegistry {
    files: Vec<std::rc::Weak<RefCell<FileState>>>,
    diagnostics: Vec<String>,
    #[cfg(test)]
    replacements: usize,
}
struct RegistryHandle(Rc<RefCell<FileRegistry>>);
impl UserData for RegistryHandle {}

struct FileState {
    bytes: Vec<u8>,
    target: Option<PathBuf>,
    closed: bool,
    cursor: usize,
    dirty: bool,
    registry: Rc<RefCell<FileRegistry>>,
    /// The mod the file belongs to: a failed background sync is shown for it.
    owner: Option<String>,
}
impl FileState {
    fn flush(&mut self) -> std::result::Result<(), String> {
        if self.closed {
            return Err("file is closed".into());
        }
        if !self.dirty {
            return Ok(());
        }
        if let Some(target) = &self.target {
            let _guard = super::sandbox::lock_parents(target, true)?;
            no_links(target)?;
            if let Some(files) = target
                .ancestors()
                .find(|p| p.file_name().is_some_and(|n| n == "files"))
            {
                if let Some(root) = files.parent() {
                    super::sandbox::check_quota(root, target, self.bytes.len())?;
                }
            }
            super::deferred_sync::atomic_replace_deferred(target, &self.bytes, self.owner.as_deref()).map_err(|e| e.to_string())?;
            #[cfg(test)]
            {
                self.registry.borrow_mut().replacements += 1;
            }
        }
        self.dirty = false;
        Ok(())
    }
    fn close(&mut self) -> std::result::Result<(), String> {
        let result = self.flush();
        self.closed = true;
        self.bytes = Vec::new();
        self.target = None;
        result
    }
    fn diagnostic(&self, error: &str) {
        let message = format!("addon file flush failed: {error}");
        let mut registry = self.registry.borrow_mut();
        if registry.diagnostics.len() < 64 && !registry.diagnostics.contains(&message) {
            registry.diagnostics.push(message);
        }
    }
}
impl Drop for FileState {
    fn drop(&mut self) {
        // Lua 5.1 __gc closes files. Iterators retain this same state, just as
        // Lua file:lines retains the original file. Last-reference GC and VM
        // destruction therefore flush each dirty writer once.
        if !self.closed {
            if let Err(error) = self.close() {
                self.diagnostic(&error);
            }
        }
    }
}
struct TextFile(Rc<RefCell<FileState>>);
impl UserData for TextFile {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_method("lines", |lua, file, ()| {
            let state = file.0.borrow();
            if state.closed || state.target.is_some() {
                return Err(mlua::Error::runtime("file is not readable"));
            }
            drop(state);
            let state = file.0.clone();
            lua.create_function_mut(move |lua, ()| {
                let mut state = state.borrow_mut();
                if state.closed {
                    return Err(mlua::Error::runtime("file is closed"));
                }
                if state.cursor >= state.bytes.len() {
                    return Ok(None);
                }
                let start = state.cursor;
                let end = state.bytes[start..]
                    .iter()
                    .position(|c| *c == b'\n')
                    .map(|n| start + n)
                    .unwrap_or(state.bytes.len());
                state.cursor = (end + 1).min(state.bytes.len());
                let end = if end > start && state.bytes[end - 1] == b'\r' {
                    end - 1
                } else {
                    end
                };
                Ok(Some(lua.create_string(&state.bytes[start..end])?))
            })
        });
        m.add_function(
            "write",
            |_, (ud, values): (mlua::AnyUserData, mlua::Variadic<mlua::String>)| {
                {
                    let file = ud.borrow::<TextFile>()?;
                    let mut state = file.0.borrow_mut();
                    if state.closed || state.target.is_none() {
                        return Err(mlua::Error::runtime("file is not writable"));
                    }
                    let len = values
                        .iter()
                        .try_fold(state.bytes.len(), |len, v| {
                            len.checked_add(v.as_bytes().len())
                        })
                        .filter(|len| *len <= MAX_BYTES)
                        .ok_or_else(|| mlua::Error::runtime("file exceeds 1 MiB"))?;
                    let extra = len - state.bytes.len();
                    state.bytes.reserve(extra);
                    for v in values {
                        state.bytes.extend_from_slice(&v.as_bytes());
                    }
                    state.dirty = true;
                }
                Ok(ud)
            },
        );
        for (name, close) in [("flush", false), ("close", true)] {
            m.add_method(name, move |lua, file, ()| {
                let mut state = file.0.borrow_mut();
                if state.closed {
                    return Err(mlua::Error::runtime("file is closed"));
                }
                let result = if close { state.close() } else { state.flush() };
                match result {
                    Ok(()) => lua.pack_multi(true),
                    Err(error) => {
                        state.diagnostic(&error);
                        lua.pack_multi((Value::Nil, error))
                    }
                }
            });
        }
    }
}

fn registry(lua: &mlua::Lua) -> Option<Rc<RefCell<FileRegistry>>> {
    let handle = lua
        .named_registry_value::<mlua::AnyUserData>("forge_file_registry")
        .ok()?;
    Some(handle.borrow::<RegistryHandle>().ok()?.0.clone())
}
pub(super) fn drain_diagnostics(lua: &mlua::Lua) -> Vec<String> {
    registry(lua)
        .map(|r| std::mem::take(&mut r.borrow_mut().diagnostics))
        .unwrap_or_default()
}
pub(super) fn close_all(lua: &mlua::Lua) -> Vec<String> {
    if let Some(registry) = registry(lua) {
        let files: Vec<_> = registry
            .borrow()
            .files
            .iter()
            .filter_map(std::rc::Weak::upgrade)
            .collect();
        for file in files {
            let mut state = file.borrow_mut();
            if !state.closed {
                if let Err(error) = state.close() {
                    state.diagnostic(&error);
                }
            }
        }
    }
    drain_diagnostics(lua)
}

fn normalized(path: &Path) -> String {
    path.to_string_lossy()
        .replace("\\\\?\\", "")
        .replace('\\', "/")
}

pub(super) fn relative(
    addon: &DiscoveredAddon,
    path: &str,
) -> std::result::Result<PathBuf, String> {
    if path.starts_with('@') {
        return Err("shared paths are read-only JSON mounts".into());
    }
    let path = path.replace("\\\\?\\", "").replace('\\', "/");
    if path.split('/').any(|part| part == "..") {
        return Err("parent traversal in addon resource path".into());
    }
    if path.len() > 4096 || path.chars().any(char::is_control) {
        return Err("invalid addon resource path".into());
    }
    // Absolute Windows paths contain a drive prefix; compare normalized roots
    // case-insensitively there, preserving the original spelling for I/O.
    let root = normalized(&addon.script_root);
    let prefix = format!("{root}/");
    let starts = if cfg!(windows) {
        path.get(..prefix.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(&prefix))
    } else {
        path.starts_with(&prefix)
    };
    let mapped;
    let tail = if starts {
        &path[prefix.len()..]
    } else {
        if path.contains(':') || path.starts_with('/') {
            return Err("file path outside issued addon mounts".into());
        }
        mapped = if let Some(tail) = path.strip_prefix("package_resources/") {
            let package = addon
                .package_root
                .strip_prefix(&addon.script_root)
                .map_err(|_| "package outside script root")?;
            normalized(&package.join("resources").join(tail))
        } else if path.starts_with("resources/") {
            path.clone()
        } else {
            format!("data/{path}")
        };
        &mapped
    };
    if tail.split('/').count() > 8
        || tail.split('/').any(|s| {
            s == ".."
                || s == "."
                || s.is_empty()
                || s.contains(':')
                || s.ends_with([' ', '.'])
                || reserved_device(s)
        })
    {
        return Err("invalid addon resource path".into());
    }
    let rel = PathBuf::from(tail);
    let source = addon.script_root.join(&rel);
    let private = addon.package_root.join("resources");
    let shared = addon.script_root.join("resources");
    if !source.starts_with(&private)
        && !source.starts_with(&shared)
        && !(!starts && tail.starts_with("data/"))
    {
        return Err("file path outside addon resources".into());
    }
    Ok(rel)
}

fn reserved_device(component: &str) -> bool {
    let stem = component
        .split('.')
        .next()
        .unwrap_or("")
        .trim_end_matches(' ')
        .to_ascii_uppercase();
    if matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) {
        return true;
    }
    ["COM", "LPT"].iter().any(|prefix| {
        stem.strip_prefix(prefix).is_some_and(|suffix| {
            let mut chars = suffix.chars();
            matches!(
                (chars.next(), chars.next()),
                (Some('1'..='9' | '\u{b9}' | '\u{b2}' | '\u{b3}'), None)
            )
        })
    })
}

fn read(path: &Path) -> std::result::Result<Vec<u8>, String> {
    super::sandbox::read_bounded(path, MAX_BYTES)
}

// Reject symlinks in every existing component, including dangling links. This
// guards both reads and writes before create_dir_all/atomic_replace follows them.
pub(super) fn no_links(path: &Path) -> std::result::Result<(), String> {
    // Linux and macOS: no link of any owner below the root the path was registered under (`rooted.rs`).
    #[cfg(unix)]
    return super::rooted::links_below_root(path);
    #[cfg(not(unix))]
    for p in path.ancestors() {
        if let Ok(meta) = fs::symlink_metadata(p) {
            if meta.file_type().is_symlink() {
                return Err("symlink in addon file path".into());
            }
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if meta.file_attributes() & 0x400 != 0 {
                    return Err("reparse point in addon file path".into());
                }
            }
        }
    }
    #[cfg(not(unix))]
    Ok(())
}

fn overlay_room(
    root: &Path,
    target: &Path,
    replaced: Option<&Path>,
) -> std::result::Result<(), String> {
    no_links(target)?;
    if target.exists() {
        return Ok(());
    }
    let mut pending = vec![root.to_path_buf()];
    let mut files = 0;
    let mut dirs = 0;
    while let Some(dir) = pending.pop() {
        dirs += 1;
        if dirs > 1024 {
            return Err("addon file directory limit exceeded".into());
        }
        if !dir.exists() {
            continue;
        }
        no_links(&dir)?;
        for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if replaced == Some(entry.path().as_path()) {
                continue;
            }
            no_links(&entry.path())?;
            if super::sandbox::import_path(root, &entry.path()) {
                continue;
            }
            if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
                pending.push(entry.path());
            } else if !entry.path().starts_with(root.join("removed")) {
                files += 1;
                if files >= 64 {
                    return Err("addon file count limit exceeded".into());
                }
            }
        }
    }
    Ok(())
}

pub(super) fn install(host: &Host, addon: &DiscoveredAddon, data: Option<PathBuf>) -> Result<()> {
    let lua = &host.lua;
    if addon
        .requested_capabilities
        .iter()
        .any(|c| c == "large_data")
    {
        lua.set_memory_limit(super::async_json::JSON_HEAP)?;
    }
    // Linux and macOS: the roots a mod's files live under are resolved once, here, and their handles held
    // (`rooted.rs`). The data folder is not a root of its own: it is descended from the configuration folder's
    // `addons` folder, so a link made of it is a link below a root and is refused.
    #[cfg(unix)]
    {
        super::rooted::register_addon(addon);
        if let Some(parent) = data.as_deref().and_then(Path::parent) {
            super::rooted::register_root(parent);
        }
    }
    lua.set_app_data(super::sandbox::Mounts {
        addon: addon.clone(),
        data: data.clone(),
    });
    super::exchange::register(lua);
    let io = lua.create_table()?;
    let addon_open = addon.clone();
    let data_open = data.clone();
    // One shared registry spans reinstall, GC and unload. Weak entries do not
    // keep files alive, while an iterator's strong state reference retains its
    // 1 MiB allocation and counts against the 16-buffer quota.
    let registry = match registry(lua) {
        Some(registry) => registry,
        None => {
            let registry = Rc::new(RefCell::new(FileRegistry::default()));
            lua.set_named_registry_value(
                "forge_file_registry",
                lua.create_userdata(RegistryHandle(registry.clone()))?,
            )?;
            registry
        }
    };
    io.set(
        "open",
        lua.create_function(move |lua, (path, mode): (String, Option<String>)| {
            {
                let mut registry = registry.borrow_mut();
                registry
                    .files
                    .retain(|f| f.upgrade().is_some_and(|f| !f.borrow().closed));
                if registry.files.len() >= 16 {
                    return lua.pack_multi((Value::Nil, "file handle limit exceeded"));
                }
            }
            let result = (|| -> std::result::Result<FileState, String> {
                let rel = relative(&addon_open, &path)?;
                let mode = mode.as_deref().unwrap_or("r");
                if mode == "w" && super::sandbox::import_key(&rel) {
                    return Err("imports staging is read-only".into());
                }
                if !matches!(mode, "r" | "w") {
                    return Err("only text modes r and w are supported".into());
                }
                let target = data_open.as_ref().map(|root| root.join("files").join(&rel));
                let deleted = data_open
                    .as_ref()
                    .map(|root| root.join("removed").join(&rel));
                if let Some(target) = &target {
                    no_links(target)?;
                }
                if let Some(deleted) = &deleted {
                    no_links(deleted)?;
                }
                if mode == "w" {
                    let target = target.ok_or("addon data directory unavailable")?;
                    let _guard = super::sandbox::lock_parents(&target, true)?;
                    overlay_room(data_open.as_ref().unwrap(), &target, deleted.as_deref())?;
                    super::deferred_sync::atomic_replace_deferred(&target, &[], Some(&addon_open.id)).map_err(|e| e.to_string())?;
                    if let Some(deleted) = deleted {
                        if deleted.exists() {
                            super::sandbox::remove_file(&deleted).map_err(|e| e.to_string())?;
                        }
                    }
                    Ok(FileState {
                        bytes: Vec::new(),
                        target: Some(target),
                        closed: false,
                        cursor: 0,
                        dirty: false,
                        registry: registry.clone(),
                        owner: Some(addon_open.id.clone()),
                    })
                } else {
                    if deleted.as_ref().is_some_and(|p| p.exists()) {
                        return Err("file not found".into());
                    }
                    let source = target
                        .filter(|p| p.exists())
                        .unwrap_or_else(|| addon_open.script_root.join(rel));
                    no_links(&source)?;
                    Ok(FileState {
                        bytes: read(&source)?,
                        target: None,
                        closed: false,
                        cursor: 0,
                        dirty: false,
                        registry: registry.clone(),
                        owner: None,
                    })
                }
            })();
            match result {
                Ok(file) => {
                    let state = Rc::new(RefCell::new(file));
                    registry.borrow_mut().files.push(Rc::downgrade(&state));
                    let ud = lua.create_userdata(TextFile(state))?;
                    lua.pack_multi(ud)
                }
                Err(error) => {
                    let code = if error.contains("not found")
                        || error.contains("os error 2")
                        || error.contains("os error 3")
                    {
                        "not_found"
                    } else {
                        "io_error"
                    };
                    // Forge scripts probe optional legacy settings with io.open.
                    // A missing file is a normal nil,error result, not an addon
                    // failure. Native eqoa.files keeps its explicit diagnostics.
                    if code != "not_found" {
                        super::native::diagnose(lua, "io.open", code, &error);
                    }
                    lua.pack_multi((Value::Nil, error))
                }
            }
        })?,
    )?;
    lua.globals().set("io", io)?;
    let os: mlua::Table = lua.globals().get("os")?;
    os.set(
        "remove",
        lua.create_function(|lua, path: String| {
            let mounts = lua
                .app_data_ref::<super::sandbox::Mounts>()
                .ok_or_else(|| mlua::Error::runtime("file mounts unavailable"))?
                .clone();
            match mounts.remove(&path) {
                Ok(true) => lua.pack_multi(true),
                Ok(false) => lua.pack_multi((Value::Nil, "file not found")),
                Err(e) => {
                    super::native::diagnose(lua, "os.remove", "io_error", &e);
                    lua.pack_multi((Value::Nil, e))
                }
            }
        })?,
    )?;
    os.set(
        "rename",
        lua.create_function(|lua, (from, to): (String, String)| {
            let mounts = lua
                .app_data_ref::<super::sandbox::Mounts>()
                .ok_or_else(|| mlua::Error::runtime("file mounts unavailable"))?
                .clone();
            let result = (|| {
                let bytes = mounts.read(&from)?;
                mounts.write(&to, &bytes)?;
                mounts.remove(&from).and_then(|removed| {
                    removed
                        .then_some(())
                        .ok_or("rename source disappeared".into())
                })
            })();
            match result {
                Ok(()) => lua.pack_multi(true),
                Err(e) => {
                    super::native::diagnose(lua, "os.rename", "io_error", &e);
                    lua.pack_multi((Value::Nil, e))
                }
            }
        })?,
    )?;
    Ok(())
}

// Filename-only glob with bounded memory and linear backtracking at the last '*'.
fn filename_matches(pattern: &str, name: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let n: Vec<char> = name.chars().collect();
    let (mut i, mut j, mut star) = (0, 0, None);
    while j < n.len() {
        if i < p.len() && (p[i] == '?' || p[i] == n[j]) {
            i += 1;
            j += 1;
        } else if i < p.len() && p[i] == '*' {
            star = Some((i, j));
            i += 1;
        } else if let Some((at, matched)) = star {
            star = Some((at, matched + 1));
            i = at + 1;
            j = matched + 1;
        } else {
            return false;
        }
    }
    while i < p.len() && p[i] == '*' {
        i += 1;
    }
    i == p.len()
}

fn list_entries(
    mounts: &super::sandbox::Mounts,
    dir: &str,
    glob: &str,
) -> std::result::Result<Vec<serde_json::Value>, (&'static str, String)> {
    let denied = |e| ("permission_denied", e);
    if dir.starts_with(['/', '\\']) || dir.contains(':') {
        return Err(denied("native file paths must be relative".into()));
    }
    let dir = dir.replace('\\', "/");
    // A checked sentinel allows the private root (empty dir) and resource roots
    // without weakening the resolver's validation of individual path components.
    let probe = if dir.is_empty() {
        "__list_probe".into()
    } else {
        format!("{dir}/__list_probe")
    };
    let key = mounts.key(&probe).map_err(denied)?;
    let key = key
        .parent()
        .ok_or_else(|| denied("invalid directory".into()))?;
    let mut roots = Vec::new();
    if let Some(data) = &mounts.data {
        roots.push(data.join("files").join(key));
    }
    if !key.starts_with("data") {
        roots.push(mounts.addon.script_root.join(key));
    }
    let mut names = std::collections::BTreeSet::new();
    for root in roots {
        no_links(&root).map_err(denied)?;
        if !root.exists() {
            continue;
        }
        let _guard =
            super::sandbox::lock_parents(&root.join("__list_guard"), false).map_err(denied)?;
        // Linux and macOS: read from the folder's held handle, the glob and the cap applied entry by entry
        // as the folder is read, in the order the Windows loop below does (`rooted.rs`).
        #[cfg(unix)]
        super::rooted::list_each::<(&'static str, String)>(
            &root,
            |name, is_link| {
                if is_link {
                    return Err(denied("symlink in addon file path".into()));
                }
                let name = name
                    .into_string()
                    .map_err(|_| ("io_error", "directory contains a non-UTF-8 name".into()))?;
                if !filename_matches(glob, &name) {
                    return Ok(());
                }
                names.insert(name);
                if names.len() > 1024 {
                    return Err((
                        "quota",
                        "directory enumeration exceeds 1,024 entries".into(),
                    ));
                }
                Ok(())
            },
            |e| if e.contains("symlink") { denied(e) } else { ("io_error", e) },
        )?;
        #[cfg(not(unix))]
        for entry in fs::read_dir(&root).map_err(|e| ("io_error", e.to_string()))? {
            let entry = entry.map_err(|e| ("io_error", e.to_string()))?;
            no_links(&entry.path()).map_err(denied)?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| ("io_error", "directory contains a non-UTF-8 name".into()))?;
            if !filename_matches(glob, &name) {
                continue;
            }
            names.insert(name);
            if names.len() > 1024 {
                return Err((
                    "quota",
                    "directory enumeration exceeds 1,024 entries".into(),
                ));
            }
        }
    }
    let mut records = Vec::new();
    for name in names {
        let path = if dir.is_empty() {
            name
        } else {
            format!("{dir}/{name}")
        };
        let entry_key = mounts.key(&path).map_err(denied)?;
        let source = match mounts.read_path(&entry_key) {
            Ok(p) => p,
            Err(e) if e.starts_with("file not found") => continue,
            Err(e) => return Err(denied(e)),
        };
        // Directories require FILE_FLAG_BACKUP_SEMANTICS, provided by the
        // parent-directory guard; file resources use the file guard instead.
        let _guard = if source.is_dir() {
            super::sandbox::lock_parents(&source.join("__list_guard"), false)
        } else {
            super::sandbox::lock_resource(&source)
        }
        .map_err(denied)?;
        #[cfg(unix)]
        let (is_file, is_dir, len) = {
            let st = super::rooted::stat(&source).map_err(denied)?;
            (st.is_file, st.is_dir, st.len)
        };
        #[cfg(not(unix))]
        let (is_file, is_dir, len) = {
            let metadata = fs::metadata(&source).map_err(|e| ("io_error", e.to_string()))?;
            (metadata.is_file(), metadata.is_dir(), metadata.len())
        };
        if !is_file && !is_dir {
            return Err(denied("special files are excluded".into()));
        }
        records.push(serde_json::json!({"path":path,"kind":if is_dir{"directory"}else{"file"},"size":if is_file{Some(len)}else{None}}));
    }
    Ok(records)
}

pub(super) fn install_native(host: &Host) -> Result<()> {
    for path in [
        "eqoa.files.FileEntry",
        "eqoa.files.FileEntry.path",
        "eqoa.files.FileEntry.kind",
        "eqoa.files.FileEntry.size",
    ] {
        super::native::describe(&host.lua, path, "supported", Some(7), "");
    }
    super::native::bind(
        &host.lua,
        "files",
        "list",
        host.lua
            .create_function(|lua, (dir, glob): (Option<Value>, Option<Value>)| {
                let text = |value: Option<Value>, default: &str| -> Option<String> {
                    match value {
                        None | Some(Value::Nil) => Some(default.into()),
                        Some(Value::String(s)) => s.to_str().ok().map(|s| s.to_string()),
                        _ => None,
                    }
                };
                let Some(dir) = text(dir, "") else {
                    return super::native::error(
                        lua,
                        "eqoa.files.list",
                        "invalid_argument",
                        "directory must be a relative UTF-8 string",
                    );
                };
                let Some(glob) = text(glob, "*") else {
                    return super::native::error(
                        lua,
                        "eqoa.files.list",
                        "invalid_argument",
                        "glob must be a UTF-8 filename pattern",
                    );
                };
                if glob.len() > 4096
                    || glob.contains(['/', '\\', ':'])
                    || glob.chars().any(char::is_control)
                {
                    return super::native::error(
                        lua,
                        "eqoa.files.list",
                        "invalid_argument",
                        "glob supports filename-only * and ? patterns",
                    );
                }
                let Some(mounts) = lua
                    .app_data_ref::<super::sandbox::Mounts>()
                    .map(|m| m.clone())
                else {
                    return super::native::error(
                        lua,
                        "eqoa.files.list",
                        "unavailable",
                        "file mounts unavailable",
                    );
                };
                match list_entries(&mounts, &dir, &glob) {
                    Ok(rows) => {
                        let result = lua.create_table()?;
                        for row in rows {
                            result.raw_push(super::native::record(
                                lua,
                                "eqoa.files.FileEntry",
                                &row,
                            )?)?;
                        }
                        lua.pack_multi((result, Value::Nil))
                    }
                    Err((code, reason)) => {
                        super::native::error(lua, "eqoa.files.list", code, &reason)
                    }
                }
            })?,
    )?;

    for name in ["read", "write", "remove"] {
        super::native::bind(
            &host.lua,
            "files",
            name,
            host.lua
                .create_function(move |lua, (path, bytes): (Value, Option<Value>)| {
                    let symbol = format!("eqoa.files.{name}");
                    let Some(path) = path
                        .as_string()
                        .and_then(|s| s.to_str().ok().map(|s| s.to_string()))
                    else {
                        return super::native::error(
                            lua,
                            &symbol,
                            "invalid_argument",
                            "path must be a relative UTF-8 string",
                        );
                    };
                    if path.starts_with(['/', '\\']) || path.contains(':') {
                        return super::native::error(
                            lua,
                            &symbol,
                            "permission_denied",
                            "native file paths must be relative",
                        );
                    }
                    let Some(mounts) = lua
                        .app_data_ref::<super::sandbox::Mounts>()
                        .map(|m| m.clone())
                    else {
                        return super::native::error(
                            lua,
                            &symbol,
                            "unavailable",
                            "file mounts unavailable",
                        );
                    };
                    if let Err(reason) = mounts.key(&path) {
                        return super::native::error(lua, &symbol, "permission_denied", &reason);
                    }
                    let result = match name {
                        "read" => mounts.read(&path).and_then(|b| {
                            lua.create_string(&b)
                                .map(Value::String)
                                .map_err(|e| e.to_string())
                        }),
                        "write" => {
                            let Some(bytes) = bytes.and_then(|v| v.as_string().cloned()) else {
                                return super::native::error(
                                    lua,
                                    &symbol,
                                    "invalid_argument",
                                    "bytes must be a binary string",
                                );
                            };
                            mounts
                                .write(&path, &bytes.as_bytes())
                                .map(|()| Value::Boolean(true))
                        }
                        _ => mounts.remove(&path).map(Value::Boolean),
                    };
                    match result {
                        Ok(v) => lua.pack_multi((v, Value::Nil)),
                        Err(e) => super::native::error(
                            lua,
                            &symbol,
                            if e.contains("not found") {
                                "not_found"
                            } else if e.contains("reparse point")
                                || e.contains("symlink")
                                || e.contains("not a regular file")
                                || e.contains("imports staging")
                            {
                                "permission_denied"
                            } else if e.contains("quota") || e.contains("exceeds") {
                                "quota"
                            } else {
                                "io_error"
                            },
                            &e,
                        ),
                    }
                })?,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn map_search_retries_pending_journal_after_first_render() {
        let lua = mlua::Lua::new();
        lua.load(
            r#"
            renders, attempts = {}, 0
            require = function() return {step=function() end, close=function() end} end
            eqoa = {
              files = {load_json=function(path)
                if path:sub(1,1) ~= '@' then return 1 end
                attempts = attempts + 1
                if attempts == 1 then return nil, {code='not_yet'} end
                return 2
              end},
              fonts = {load=function() end},
              input = {bind=function() end},
              events = {on=function(name, callback) renders[name] = callback end},
              map = {get=function() end},
            }
        "#,
        )
        .exec()
        .unwrap();
        lua.load(include_str!(
            "../../../../../bundled-mods/eqoa_map_search/main.lua"
        ))
        .exec()
        .unwrap();
        lua.load(
            r#"
            assert(attempts == 0, 'journal requested during entry')
            renders.render({time=1})
            assert(attempts == 1)
            renders.render({time=1.5})
            assert(attempts == 1)
            renders.render({time=2})
            renders.render({time=3})
            assert(attempts == 2, 'closed panel must retry once the owner is ready')
        "#,
        )
        .exec()
        .unwrap();
    }

    #[test]
    fn shared_json_waits_for_enabled_owner_before_loading() {
        use super::super::{exchange, native, native_events};
        let (dir, owner, mut addon) = fixture();
        addon.id = "pending_owner".into();
        addon.extensions.shares = vec!["data/journal.json".into()];
        install(&owner, &addon, Some(dir.0.join("config"))).unwrap();
        owner
            .execute("assert(eqoa.files.write('journal.json','{}'))")
            .unwrap();
        native_events::bus(&owner.lua).borrow_mut().active = false;
        let (_dir, reader, _) = fixture();
        {
            let _scope = exchange::pending_owners(["pending_owner"]);
            reader
                .execute(
                    r#"
                local path = '@pending_owner/data/journal.json'
                local request, err = eqoa.files.load_json(path)
                assert(request == nil and err.code == 'not_yet', err and err.code)
            "#,
                )
                .unwrap();
            assert!(native::finish(&reader.lua).is_empty());
            native_events::prime(&owner);
            reader
                .execute("assert(eqoa.files.load_json('@pending_owner/data/journal.json'))")
                .unwrap();
        }
        native_events::bus(&owner.lua).borrow_mut().active = false;
        reader
            .execute(
                r#"
            local request, err = eqoa.files.load_json('@pending_owner/data/journal.json')
            assert(request == nil and err.code == 'permission_denied')
        "#,
            )
            .unwrap();
        assert_eq!(native::finish(&reader.lua).len(), 1);
    }

    #[test]
    fn round4b_export_waits_for_accepted_async_snapshot_before_first_file_exists() {
        use super::super::{json_save, native_events, options, player_files::TestIo};
        struct Release(Option<std::sync::mpsc::Sender<()>>);
        impl Drop for Release {
            fn drop(&mut self) {
                if let Some(tx) = self.0.take() {
                    let _ = tx.send(());
                }
            }
        }
        let (dir, host, mut addon) = fixture_with_large_data(true);
        addon.extensions.permissions = vec!["export".into()];
        let root = dir.0.join("config");
        install(&host, &addon, Some(root.clone())).unwrap();
        let desktop = dir.0.join("Desktop");
        host.lua.set_app_data(TestIo {
            pick: None,
            desktop: desktop.clone(),
        });
        let (entered, release) = json_save::pause_next_commit();
        let mut release = Release(Some(release));
        host.execute("accepted=false;done=false;failed='';button_id=assert(eqoa.options.button({id='export',label='Export journal'},function() assert(eqoa.files.save_json_async('journal.json',{stage=42,payload=string.rep('x',2*1024*1024)}));local id,e=eqoa.files.export('journal.json','journal.json');accepted=id~=nil;failed=e and e.message or '' end)).item_id;eqoa.events.on('resource.ready',function(e) if e.data.kind=='export' then done=true end end);eqoa.events.on('resource.failed',function(e) if e.data.kind=='export' then failed=e.data.error.message end end)").unwrap();
        assert!(options::activate_button(
            host.lua.globals().get("button_id").unwrap()
        ));
        native_events::dispatch(&host).unwrap();
        entered
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        assert!(
            host.lua.globals().get::<bool>("accepted").unwrap(),
            "{:?}",
            host.lua.globals().get::<String>("failed")
        );
        assert!(!desktop.join("journal.json").exists());
        release.0.take().unwrap().send(()).unwrap();
        let end = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !host.lua.globals().get::<bool>("done").unwrap() {
            assert!(
                std::time::Instant::now() < end,
                "{:?}",
                host.lua.globals().get::<String>("failed")
            );
            native_events::dispatch(&host).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let saved: serde_json::Value =
            serde_json::from_slice(&fs::read(desktop.join("journal.json")).unwrap()).unwrap();
        assert_eq!(saved["stage"], 42);
        assert_eq!(saved["payload"].as_str().unwrap().len(), 2 * 1024 * 1024);
    }

    #[test]
    fn round4b_review_remove_notifies_shared_readers() {
        let (dir, owner, mut addon) = fixture();
        addon.id = "owner_remove".into();
        addon.extensions.shares = vec!["data/journal.json".into()];
        install(&owner, &addon, Some(dir.0.join("config"))).unwrap();
        owner
            .execute("assert(eqoa.files.write('journal.json','{}'))")
            .unwrap();
        let (_dir, reader, _) = fixture();
        reader.execute("changed=false;eqoa.events.on('shared.changed',function(e) changed=e.data.mod=='owner_remove' and e.data.path=='data/journal.json' end)").unwrap();
        owner
            .execute("assert(eqoa.files.remove('journal.json'))")
            .unwrap();
        super::super::native_events::dispatch(&reader).unwrap();
        assert!(reader.lua.globals().get::<bool>("changed").unwrap());
    }

    #[test]
    fn round4b_review_disconnect_reports_original_import_kind() {
        use super::super::{
            native_events, options,
            player_files::{self, TestIo},
        };
        let (dir, host, mut addon) = fixture();
        addon.extensions.permissions = vec!["import".into()];
        install(&host, &addon, Some(dir.0.join("config"))).unwrap();
        host.lua.set_app_data(TestIo {
            pick: None,
            desktop: dir.0.join("Desktop"),
        });
        host.execute("got_kind='';capability='';button_id=assert(eqoa.options.button({id='import',label='Import journal...'},function() req=assert(eqoa.files.import({title='Journal',extensions={'json'}})) end)).item_id;eqoa.events.on('resource.failed',function(e) assert(e.data.request==req);got_kind=e.data.kind;capability=e.data.error.capability end)").unwrap();
        assert!(options::activate_button(
            host.lua.globals().get("button_id").unwrap()
        ));
        native_events::dispatch(&host).unwrap();
        player_files::disconnect_worker(&host.lua);
        native_events::dispatch(&host).unwrap();
        assert_eq!(
            host.lua.globals().get::<String>("got_kind").unwrap(),
            "import"
        );
        assert_eq!(
            host.lua.globals().get::<String>("capability").unwrap(),
            "eqoa.files.import"
        );
    }

    #[test]
    fn round4b_review_export_has_no_partial_final_file() {
        use super::super::{
            native_events, options,
            player_files::{self, TestIo},
        };
        struct Release(Option<std::sync::mpsc::Sender<()>>);
        impl Drop for Release {
            fn drop(&mut self) {
                if let Some(tx) = self.0.take() {
                    let _ = tx.send(());
                }
            }
        }
        let (dir, host, mut addon) = fixture();
        addon.extensions.permissions = vec!["export".into()];
        install(&host, &addon, Some(dir.0.join("config"))).unwrap();
        let desktop = dir.0.join("Desktop");
        host.lua.set_app_data(TestIo {
            pick: None,
            desktop: desktop.clone(),
        });
        let (rx, release) = player_files::pause_copy(desktop.clone());
        let mut release = Release(Some(release));
        host.execute("assert(eqoa.files.write('journal.json','complete journal'));done=false;button_id=assert(eqoa.options.button({id='export',label='Export journal'},function() assert(eqoa.files.export('journal.json','journal.json')) end)).item_id;eqoa.events.on('resource.ready',function(e) if e.data.kind=='export' then done=true end end)").unwrap();
        assert!(options::activate_button(
            host.lua.globals().get("button_id").unwrap()
        ));
        native_events::dispatch(&host).unwrap();
        rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        assert!(
            !desktop.join("journal.json").exists(),
            "a final export name was visible before publication"
        );
        let temp = fs::read_dir(&desktop)
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| p.extension().is_some_and(|e| e == "tmp"))
            .expect("synced temporary export");
        assert_eq!(fs::read_to_string(temp).unwrap(), "complete journal");
        release.0.take().unwrap().send(()).unwrap();
        let end = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !host.lua.globals().get::<bool>("done").unwrap() {
            assert!(std::time::Instant::now() < end);
            native_events::dispatch(&host).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(
            fs::read_to_string(desktop.join("journal.json")).unwrap(),
            "complete journal"
        );
        assert_eq!(fs::read_dir(desktop).unwrap().count(), 1);
    }

    #[test]
    fn round4b_review_unused_delegated_action_expires_after_dispatch() {
        use super::super::{native_core::CORE, native_events};
        use std::sync::atomic::Ordering;
        let (dir, receiver, mut addon) = fixture();
        addon.id = "unused_receiver".into();
        addon.extensions.commands = vec!["noop".into()];
        install(&receiver, &addon, Some(dir.0.join("config"))).unwrap();
        receiver
            .execute("seen=false;eqoa.events.on('mod.command',function() seen=true end)")
            .unwrap();
        let (_dir, sender, _) = fixture();
        sender.execute("eqoa.events.on('input.key',function() assert(eqoa.mods.send('unused_receiver','noop')) end)").unwrap();
        let action = CORE.with(|c| {
            let mut c = c.borrow_mut();
            c.emit(
                "input.key",
                serde_json::json!({"key":"f2","down":true,"repeat":false,"synthetic":false}),
            );
            c.events.back().unwrap().player_action.clone().unwrap()
        });
        assert_eq!(action.load(Ordering::Acquire), 0);
        native_events::dispatch(&sender).unwrap();
        assert_eq!(action.load(Ordering::Acquire), 1);
        native_events::dispatch(&receiver).unwrap();
        assert!(receiver.lua.globals().get::<bool>("seen").unwrap());
        assert_eq!(
            action.load(Ordering::Acquire),
            2,
            "unused command authority survived its dispatch"
        );
    }

    #[test]
    fn round4b_review_disabled_dialog_stays_single_and_has_mod_title() {
        use super::super::{
            native_events, options,
            player_files::{BlockPick, TestIo},
        };
        struct Release(Option<std::sync::mpsc::Sender<()>>);
        impl Drop for Release {
            fn drop(&mut self) {
                if let Some(tx) = self.0.take() {
                    let _ = tx.send(());
                }
            }
        }
        let (dir, host, mut addon) = fixture();
        addon.name = "Journal addon".into();
        addon.extensions.permissions = vec!["import".into()];
        let root = dir.0.join("config");
        install(&host, &addon, Some(root.clone())).unwrap();
        let source = dir.0.join("picked.json");
        fs::write(&source, "{}").unwrap();
        host.lua.set_app_data(TestIo {
            pick: Some(source),
            desktop: dir.0.join("Desktop"),
        });
        let (entered, rx) = std::sync::mpsc::channel();
        let (tx, release_rx) = std::sync::mpsc::channel();
        let mut release = Release(Some(tx));
        host.lua.set_app_data(BlockPick {
            entered,
            release: Rc::new(RefCell::new(Some(release_rx))),
        });
        let setup = || {
            host.execute("last=nil;done=false;code='';button_id=assert(eqoa.options.button({id='import',label='Import journal...'},function() local id,e=eqoa.files.import({title='Journal',extensions={'json'}});last=id;code=e and e.code or '' end)).item_id;eqoa.events.on('resource.ready',function(e) if e.data.kind=='import' then done=true end end)").unwrap()
        };
        setup();
        assert!(options::activate_button(
            host.lua.globals().get("button_id").unwrap()
        ));
        native_events::dispatch(&host).unwrap();
        rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        assert_eq!(
            host.lua
                .named_registry_value::<String>("player_file_picker_title")
                .unwrap(),
            "Journal addon: Journal"
        );
        native_events::clear(&host);
        install(&host, &addon, Some(root.clone())).unwrap();
        native_events::prime(&host);
        setup();
        assert!(options::activate_button(
            host.lua.globals().get("button_id").unwrap()
        ));
        native_events::dispatch(&host).unwrap();
        assert_eq!(
            host.lua.globals().get::<String>("code").unwrap(),
            "busy",
            "re-enable must not open a second dialog while the old OS dialog is open"
        );
        release.0.take().unwrap().send(()).unwrap();
        let end = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while host
            .lua
            .globals()
            .get::<Option<u64>>("last")
            .unwrap()
            .is_none()
        {
            assert!(std::time::Instant::now() < end);
            assert!(options::activate_button(
                host.lua.globals().get("button_id").unwrap()
            ));
            native_events::dispatch(&host).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        while !host.lua.globals().get::<bool>("done").unwrap() {
            assert!(std::time::Instant::now() < end);
            native_events::dispatch(&host).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(
            fs::read_dir(root.join("files/data/imports"))
                .unwrap()
                .count(),
            1,
            "disabled dialog copied no file"
        );
    }

    #[test]
    fn round4b_review_one_physical_action_is_spent_across_vms() {
        use super::super::{native_core::CORE, native_events, player_files::TestIo};
        let (ad, a, mut aa) = fixture();
        aa.extensions.permissions = vec!["export".into()];
        install(&a, &aa, Some(ad.0.join("config"))).unwrap();
        let (bd, b, mut ba) = fixture();
        ba.id = "second".into();
        ba.extensions.permissions = vec!["export".into()];
        install(&b, &ba, Some(bd.0.join("config"))).unwrap();
        a.lua.set_app_data(TestIo {
            pick: None,
            desktop: ad.0.join("Desktop"),
        });
        b.lua.set_app_data(TestIo {
            pick: None,
            desktop: bd.0.join("Desktop"),
        });
        let setup = "assert(eqoa.files.write('journal.json','{}'));accepted=false;done=false;local function attempt() local id,e=eqoa.files.export('journal.json','journal.json');accepted=id~=nil;code=e and e.code end;eqoa.events.on('input.key',attempt);eqoa.events.on('pointer.pressed',attempt);eqoa.events.on('resource.ready',function(e) if e.data.kind=='export' then done=true end end)";
        a.execute(setup).unwrap();
        b.execute(setup).unwrap();
        CORE.with(|c| {
            c.borrow_mut().emit(
                "input.key",
                serde_json::json!({"key":"f2","down":true,"synthetic":true,"repeat":false}),
            )
        });
        native_events::dispatch(&a).unwrap();
        native_events::dispatch(&b).unwrap();
        assert!(
            !a.lua.globals().get::<bool>("accepted").unwrap()
                && !b.lua.globals().get::<bool>("accepted").unwrap()
        );
        CORE.with(|c| {
            c.borrow_mut().emit(
                "input.key",
                serde_json::json!({"key":"f2","down":true,"synthetic":false,"repeat":false}),
            )
        });
        native_events::dispatch(&a).unwrap();
        native_events::dispatch(&b).unwrap();
        let accepted = u32::from(a.lua.globals().get::<bool>("accepted").unwrap())
            + u32::from(b.lua.globals().get::<bool>("accepted").unwrap());
        assert_eq!(
            accepted, 1,
            "one native action must not authorize two mods' exports"
        );
        let end = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !a.lua.globals().get::<bool>("done").unwrap() {
            assert!(std::time::Instant::now() < end);
            native_events::dispatch(&a).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        CORE.with(|c| {
            c.borrow_mut().emit(
                "pointer.pressed",
                serde_json::json!({"inside_window":false,"x":0,"y":0,"button":"left"}),
            )
        });
        native_events::dispatch(&a).unwrap();
        native_events::dispatch(&b).unwrap();
        assert!(
            !a.lua.globals().get::<bool>("accepted").unwrap()
                && !b.lua.globals().get::<bool>("accepted").unwrap()
        );
    }

    #[test]
    fn round4b_review_import_ids_survive_enable_and_staging_cannot_starve_saves() {
        use super::super::{native_events, options, player_files::TestIo};
        let (dir, host, mut addon) = fixture_with_large_data(true);
        addon.extensions.permissions = vec!["import".into()];
        let root = dir.0.join("config");
        install(&host, &addon, Some(root.clone())).unwrap();
        let picked = dir.0.join("large.json");
        fs::write(
            &picked,
            format!("{{\"payload\":\"{}\"}}", "x".repeat(6 * 1024 * 1024)),
        )
        .unwrap();
        host.lua.set_app_data(TestIo {
            pick: Some(picked),
            desktop: dir.0.join("Desktop"),
        });
        let setup = || {
            host.execute("done=false;failure='';button_id=assert(eqoa.options.button({id='import',label='Import journal...'},function() req=assert(eqoa.files.import({title='Journal',extensions={'json'}})) end)).item_id;eqoa.events.on('resource.ready',function(e) if e.data.kind=='import' or e.data.kind=='json_saved' then assert(e.data.request==req);done=true;import_path=e.data.path end end);eqoa.events.on('resource.failed',function(e) failure=e.data.error.message end)").unwrap()
        };
        let wait = || {
            let end = std::time::Instant::now() + std::time::Duration::from_secs(10);
            while !host.lua.globals().get::<bool>("done").unwrap() {
                assert!(
                    std::time::Instant::now() < end,
                    "{:?}",
                    host.lua.globals().get::<String>("failure")
                );
                native_events::dispatch(&host).unwrap();
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        };
        setup();
        let mut paths = Vec::new();
        for i in 0..4 {
            if i == 2 {
                native_events::clear(&host);
                install(&host, &addon, Some(root.clone())).unwrap();
                native_events::prime(&host);
                setup();
            }
            host.execute("done=false;failure=''").unwrap();
            assert!(options::activate_button(
                host.lua.globals().get("button_id").unwrap()
            ));
            wait();
            paths.push(host.lua.globals().get::<String>("import_path").unwrap());
        }
        assert_eq!(
            paths,
            vec![
                "imports/import-1.json",
                "imports/import-2.json",
                "imports/import-3.json",
                "imports/import-4.json"
            ]
        );
        let imports = root.join("files/data/imports");
        assert_eq!(fs::read_dir(&imports).unwrap().count(), 3);
        assert!(!imports.join("import-1.json").exists());
        host.execute("local ok,e=eqoa.files.write('imports/escape.json','{}');assert(ok==nil and e.code=='permission_denied');local ok,e=eqoa.files.save_json_async('imports/escape.json',{});assert(ok==nil and e.code=='permission_denied');done=false;req=assert(eqoa.files.save_json_async('journal.json',{payload=string.rep('j',14*1024*1024-128)}))").unwrap();
        wait();
        assert!(
            fs::metadata(root.join("files/data/journal.json"))
                .unwrap()
                .len()
                > 13 * 1024 * 1024
        );
    }

    #[cfg(windows)]
    #[test]
    fn round4b_review_redirected_desktop_and_picked_file_keep_private_checks() {
        use super::super::{native_events, options, player_files::TestIo};
        let (dir, host, mut addon) = fixture();
        addon.extensions.permissions = vec!["import".into(), "export".into()];
        install(&host, &addon, Some(dir.0.join("config"))).unwrap();
        let actual = dir.0.join("cloud-folder");
        fs::create_dir_all(&actual).unwrap();
        let desktop = dir.0.join("Desktop");
        let result = std::process::Command::new("cmd")
            .args(["/D", "/C", "mklink", "/J"])
            .arg(&desktop)
            .arg(&actual)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "junction creation failed: {:?}",
            result
        );
        fs::write(actual.join("picked.json"), "{\"Bat\":{}}").unwrap();
        host.lua.set_app_data(TestIo {
            pick: Some(desktop.join("picked.json")),
            desktop: desktop.clone(),
        });
        host.execute("assert(eqoa.files.write('journal.json','{}'));mode='export';finished=false;success=false;failure='';button_id=assert(eqoa.options.button({id='file',label='File action'},function() if mode=='export' then assert(eqoa.files.export('journal.json','journal.json')) else assert(eqoa.files.import({title='Journal',extensions={'json'}})) end end)).item_id;eqoa.events.on('resource.ready',function(e) if e.data.kind=='import' or e.data.kind=='export' then finished=true;success=true;import_path=e.data.path end end);eqoa.events.on('resource.failed',function(e) if e.data.kind=='import' or e.data.kind=='export' then finished=true;failure=e.data.error.message end end)").unwrap();
        let wait = || {
            let end = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while !host.lua.globals().get::<bool>("finished").unwrap() {
                assert!(std::time::Instant::now() < end);
                native_events::dispatch(&host).unwrap();
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            assert!(
                host.lua.globals().get::<bool>("success").unwrap(),
                "{:?}",
                host.lua.globals().get::<String>("failure")
            );
        };
        let id = host.lua.globals().get("button_id").unwrap();
        assert!(options::activate_button(id));
        wait();
        assert_eq!(
            fs::read_to_string(actual.join("journal.json")).unwrap(),
            "{}"
        );
        host.execute("finished=false;success=false;mode='import'")
            .unwrap();
        assert!(options::activate_button(id));
        wait();
        let imported = host.lua.globals().get::<String>("import_path").unwrap();
        assert_eq!(
            fs::read_to_string(dir.0.join("config/files/data").join(imported)).unwrap(),
            "{\"Bat\":{}}"
        );
        let private_link = dir.0.join("private-link");
        let result = std::process::Command::new("cmd")
            .args(["/D", "/C", "mklink", "/J"])
            .arg(&private_link)
            .arg(&actual)
            .output()
            .unwrap();
        assert!(result.status.success());
        install(&host, &addon, Some(private_link)).unwrap();
        host.execute("local ok,e=eqoa.files.write('forbidden.json','{}');assert(ok==nil and e.code=='permission_denied')").unwrap();
        assert!(!actual.join("files/data/forbidden.json").exists());
    }

    #[test]
    fn round4b_review_commands_preserve_other_events_and_sender_fairness() {
        use super::super::{native_core::CORE, native_events};
        let (rd, receiver, mut ra) = fixture();
        ra.id = "receiver".into();
        ra.extensions.commands = vec!["ping".into()];
        install(&receiver, &ra, Some(rd.0.join("config"))).unwrap();
        let (ad, a, mut aa) = fixture();
        aa.id = "sender_a".into();
        install(&a, &aa, Some(ad.0.join("config"))).unwrap();
        let (bd, b, mut ba) = fixture();
        ba.id = "sender_b".into();
        install(&b, &ba, Some(bd.0.join("config"))).unwrap();
        receiver.execute("entity_seen=false;resource_seen=false;commands=0;eqoa.events.on('entity.added',function(e) entity_seen=e.data.after.name=='sentinel' end);eqoa.events.on('resource.ready',function(e) if e.data.kind=='json_saved' then resource_seen=e.data.request==19 end end);eqoa.events.on('mod.command',function(e) commands=commands+1 end)").unwrap();
        native_events::json_saved(&receiver.lua, 19, None).unwrap();
        CORE.with(|c|c.borrow_mut().emit("entity.added",serde_json::json!({"after":{"name":"sentinel","padding":"x".repeat(2*1024*1024-2048)}})));
        a.execute("local no,e=eqoa.mods.send('receiver','ping',{p=string.rep('x',4096)});assert(no==nil and e.code=='quota','4 KiB payload cap');for i=1,4 do assert(eqoa.mods.send('receiver','ping',{p=string.rep('a',1000)})) end;local no,e=eqoa.mods.send('receiver','ping',{});assert(no==nil and e.code=='quota','per-sender cap')").unwrap();
        b.execute("assert(eqoa.mods.send('receiver','ping',{p='other sender'}))")
            .unwrap();
        native_events::dispatch(&receiver).unwrap();
        assert!(
            receiver.lua.globals().get::<bool>("entity_seen").unwrap(),
            "command traffic evicted an entity event"
        );
        assert!(
            receiver.lua.globals().get::<bool>("resource_seen").unwrap(),
            "command traffic evicted an import/save completion"
        );
        assert_eq!(receiver.lua.globals().get::<u32>("commands").unwrap(), 5);
        a.execute("assert(eqoa.mods.send('receiver','ping',{}))")
            .unwrap();
    }

    #[test]
    fn round4b_review_export_refuses_executable_and_reserved_names() {
        use super::super::{native_events, options, player_files::TestIo};
        let (dir, host, mut addon) = fixture();
        addon.extensions.permissions = vec!["export".into()];
        install(&host, &addon, Some(dir.0.join("config"))).unwrap();
        let desktop = dir.0.join("Desktop");
        host.lua.set_app_data(TestIo {
            pick: None,
            desktop: desktop.clone(),
        });
        host.execute("assert(eqoa.files.write('journal.json','{}'));rejected=false;done=false;button_id=assert(eqoa.options.button({id='export',label='Export journal'},function() for _,name in ipairs({'evil.bat','evil.lnk','evil.exe','desktop.ini','.hidden.json','.json','CON.txt','x/y.json','x'..string.char(92)..'y.json','file.json.'}) do local ok,e=eqoa.files.export('journal.json',name);assert(ok==nil and e.code=='invalid_argument',name) end;rejected=true;assert(eqoa.files.export('journal.json','Journal.JSON')) end)).item_id;eqoa.events.on('resource.ready',function(e) if e.data.kind=='export' then done=true end end)").unwrap();
        assert!(options::activate_button(
            host.lua.globals().get("button_id").unwrap()
        ));
        native_events::dispatch(&host).unwrap();
        assert!(
            host.lua.globals().get::<bool>("rejected").unwrap(),
            "unsafe export accepted: {:?}",
            super::super::native::finish(&host.lua)
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !host.lua.globals().get::<bool>("done").unwrap() {
            assert!(std::time::Instant::now() < deadline);
            native_events::dispatch(&host).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(
            fs::read_to_string(desktop.join("Journal.JSON")).unwrap(),
            "{}"
        );
        assert_eq!(fs::read_dir(&desktop).unwrap().count(), 1);
    }

    #[test]
    fn round4b_option_buttons_are_player_activations_and_disable_cleanly() {
        use super::super::{native_events, options, player_files::TestIo};
        let (dir, host, mut addon) = fixture();
        addon.extensions.permissions = vec!["export".into()];
        install(&host, &addon, Some(dir.0.join("config"))).unwrap();
        let desktop = dir.0.join("Desktop");
        host.lua.set_app_data(TestIo {
            pick: None,
            desktop: desktop.clone(),
        });
        host.execute("assert(eqoa.files.write('journal.json','{}'));clicked=0;done=false;local row=assert(eqoa.options.button({id='export',label='Export journal'},function() clicked=clicked+1;assert(eqoa.files.export('journal.json','journal.json')) end));button_id=row.item_id;eqoa.events.on('resource.ready',function(e) if e.data.kind=='export' then done=true end end)").unwrap();
        let id = host.lua.globals().get::<i32>("button_id").unwrap();
        assert!(options::active_mod_items().contains(&(id, "Export journal".into())));
        assert!(
            !options::toggle_by_item(id),
            "stepping over an action must not execute it"
        );
        assert_eq!(host.lua.globals().get::<u32>("clicked").unwrap(), 0);
        assert!(options::activate_button(id));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !host.lua.globals().get::<bool>("done").unwrap() {
            assert!(std::time::Instant::now() < deadline);
            native_events::dispatch(&host).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(
            fs::read_to_string(desktop.join("journal.json")).unwrap(),
            "{}"
        );
        native_events::clear(&host);
        assert!(!options::activate_button(id));
        assert!(
            !options::active_mod_items()
                .iter()
                .any(|(item, _)| *item == id)
        );
    }

    #[test]
    fn round4b_mod_command_can_target_its_own_vm() {
        let (dir, host, mut addon) = fixture();
        addon.extensions.commands = vec!["self".into()];
        install(&host, &addon, Some(dir.0.join("config"))).unwrap();
        host.execute("count=0;eqoa.events.on('mod.command',function(e) count=count+1 end);assert(eqoa.mods.send('pack','self'))").unwrap();
        super::super::native_events::dispatch(&host).unwrap();
        assert_eq!(host.lua.globals().get::<u32>("count").unwrap(), 1);
    }

    #[test]
    fn round4b_player_command_import_export_copy_files_and_cancel() {
        use super::super::{native_core::CORE, native_events as events, player_files::TestIo};
        use serde_json::json;
        let (owner_dir, owner, mut journal) = fixture();
        journal.id = "journal_actions".into();
        journal.extensions.commands = vec!["import_journal".into(), "export_journal".into()];
        journal.extensions.permissions = vec!["import".into(), "export".into()];
        install(&owner, &journal, Some(owner_dir.0.join("config"))).unwrap();
        let (sender_dir, sender, mut addon) = fixture();
        addon.extensions.commands = vec!["forward".into()];
        addon.extensions.permissions = vec!["export".into()];
        install(&sender, &addon, Some(sender_dir.0.join("config"))).unwrap();
        let desktop = owner_dir.0.join("Desktop");
        fs::create_dir_all(&desktop).unwrap();
        fs::write(
            desktop.join("EQOA-Creature-Journal.json"),
            "existing export",
        )
        .unwrap();
        let picked = owner_dir.0.join("Forge.json");
        let forge = r#"{"Bat":{"42":{"world":0,"level":2,"pos":[1,2,3]}}}"#;
        fs::write(&picked, forge).unwrap();
        owner.lua.set_app_data(TestIo {
            pick: Some(picked),
            desktop: desktop.clone(),
        });
        sender.lua.set_app_data(TestIo {
            pick: None,
            desktop: desktop.clone(),
        });
        owner.execute("assert(eqoa.files.write('journal.json','{\"Bat\":{}}'));last=0;completed=false;cancelled=false;eqoa.events.on('mod.command',function(e) assert(eqoa.mods.send('pack','forward'));if e.data.command=='export_journal' then local no,err=eqoa.files.export('../outside','oops.json');assert(no==nil and err.code=='permission_denied');last=assert(eqoa.files.export('journal.json','EQOA-Creature-Journal.json')) else last=assert(eqoa.files.import({title='Import journal',extensions={'json'}})) end;local no,err=eqoa.files.export('journal.json','again.json');assert(no==nil and err.code=='requires_player') end);eqoa.events.on('resource.ready',function(e) if e.data.kind=='export' or e.data.kind=='import' then assert(e.data.request==last);completed=true;import_path=e.data.path end end);file_failure='';eqoa.events.on('resource.failed',function(e) file_failure=e.data.kind..':'..e.data.error.code..':'..e.data.error.message;if e.data.kind=='import' then assert(e.data.request==last);cancelled=e.data.error.code=='cancelled' end end)").unwrap();
        sender.execute("forward_denied=false;eqoa.events.on('mod.command',function(e) local no,err=eqoa.files.export('journal.json','forwarded.json');forward_denied=no==nil and err.code=='requires_player' end);command='export_journal';eqoa.events.on('input.key',function(e) if e.data.key=='f2' and e.data.down then assert(eqoa.mods.send('missing','x')==nil);assert(eqoa.mods.send('journal_actions',command)) end end)").unwrap();
        let press = || {
            CORE.with(|c| {
                c.borrow_mut().emit(
                    "input.key",
                    json!({"key":"f2","down":true,"synthetic":false,"repeat":false}),
                )
            });
        };
        let wait = |flag: &str| {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while !owner.lua.globals().get::<bool>(flag).unwrap() {
                assert!(
                    std::time::Instant::now() < deadline,
                    "{flag} did not complete; failure={:?}; request={:?}; owner={:?}; sender={:?}",
                    owner.lua.globals().get::<String>("file_failure"),
                    owner.lua.globals().get::<u64>("last"),
                    super::super::native::finish(&owner.lua),
                    super::super::native::finish(&sender.lua)
                );
                events::dispatch(&owner).unwrap();
                events::dispatch(&sender).unwrap();
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        };
        press();
        events::dispatch(&sender).unwrap();
        wait("completed");
        assert_eq!(
            fs::read_to_string(desktop.join("EQOA-Creature-Journal.json")).unwrap(),
            "existing export"
        );
        assert_eq!(
            fs::read_to_string(desktop.join("EQOA-Creature-Journal (2).json")).unwrap(),
            r#"{"Bat":{}}"#
        );
        events::dispatch(&sender).unwrap();
        assert!(sender.lua.globals().get::<bool>("forward_denied").unwrap());
        owner.execute("completed=false").unwrap();
        sender.execute("command='import_journal'").unwrap();
        press();
        events::dispatch(&sender).unwrap();
        wait("completed");
        let path = owner.lua.globals().get::<String>("import_path").unwrap();
        assert!(path.starts_with("imports/"));
        assert_eq!(
            fs::read_to_string(owner_dir.0.join("config/files/data").join(path)).unwrap(),
            forge
        );
        owner.lua.set_app_data(TestIo {
            pick: None,
            desktop: desktop.clone(),
        });
        press();
        events::dispatch(&sender).unwrap();
        wait("cancelled");
        assert!(!desktop.join("again.json").exists());
        assert!(!desktop.join("forwarded.json").exists());
    }

    #[test]
    fn round4b_file_actions_require_permission_and_player_activation() {
        let (dir, host, mut addon) = fixture();
        host.execute("local ok,e=eqoa.files.import({title='Journal',extensions={'json'}});assert(ok==nil and e.code=='permission_denied');local ok,e=eqoa.files.export('journal.json','journal.json');assert(ok==nil and e.code=='permission_denied')").unwrap();
        addon.extensions.permissions = vec!["import".into(), "export".into()];
        install(&host, &addon, Some(dir.0.join("config"))).unwrap();
        host.execute("local ok,e=eqoa.files.import({title='Journal',extensions={'json'}});assert(ok==nil and e.code=='requires_player');local ok,e=eqoa.files.export('journal.json','journal.json');assert(ok==nil and e.code=='requires_player');denied=false;eqoa.events.after(0,function() local ok,e=eqoa.files.export('journal.json','journal.json');denied=ok==nil and e.code=='requires_player' end)").unwrap();
        for _ in 0..3 {
            super::super::native_events::dispatch(&host).unwrap();
        }
        assert!(host.lua.globals().get::<bool>("denied").unwrap());
    }

    #[test]
    fn round4b_mod_commands_are_declared_bounded_and_delayed() {
        let (dir, receiver, mut addon) = fixture();
        addon.id = "journal_receiver".into();
        addon.extensions.commands = vec!["import_journal".into()];
        install(&receiver, &addon, Some(dir.0.join("config"))).unwrap();
        let (_dir, sender, _) = fixture();
        receiver.execute("count=0;eqoa.events.on('mod.command',function(e) assert(e.data.from=='pack' and e.data.command=='import_journal' and e.data.args.value==42);count=count+1 end)").unwrap();
        sender.execute("local args={value=42};assert(eqoa.mods.send('journal_receiver','import_journal',args));args.value=7;local ok,e=eqoa.mods.send('journal_receiver','undeclared');assert(ok==nil and e.code=='invalid_argument');assert(eqoa.mods.send('missing','import_journal')==nil)").unwrap();
        assert_eq!(receiver.lua.globals().get::<u32>("count").unwrap(), 0);
        super::super::native_events::dispatch(&receiver).unwrap();
        assert_eq!(receiver.lua.globals().get::<u32>("count").unwrap(), 1);
        sender.execute("for i=1,4 do assert(eqoa.mods.send('journal_receiver','import_journal',{value=42})) end;local ok,e=eqoa.mods.send('journal_receiver','import_journal',{value=42});assert(ok==nil and e.code=='quota');local huge={};for i=1,1024 do huge[i]=i end;assert(eqoa.mods.send('journal_receiver','import_journal',huge)==nil)").unwrap();
        super::super::native_events::clear(&receiver);
        sender.execute("local ok,e=eqoa.mods.send('journal_receiver','import_journal');assert(ok==nil and e.code=='unavailable')").unwrap();
    }

    #[test]
    fn round4b_shared_json_requires_running_owner_and_declared_path() {
        let (_owner_dir, owner, mut addon) = fixture();
        addon.id = "journal_owner".into();
        addon.extensions.shares = vec!["data/journal.json".into()];
        let root = _owner_dir.0.join("config");
        install(&owner, &addon, Some(root.clone())).unwrap();
        fs::create_dir_all(root.join("files/data")).unwrap();
        fs::write(
            root.join("files/data/journal.json"),
            r#"{"Bat":{"42":{"world":0,"level":2,"pos":[1,2,3]}}}"#,
        )
        .unwrap();
        fs::write(root.join("files/data/secret.json"), "{}").unwrap();
        let (_reader_dir, reader, _) = fixture();
        reader.execute("loaded=false;eqoa.events.on('resource.ready',function(e) if e.data.kind=='json' then loaded=e.data.value.Bat['42'].level==2 end end);local id,e=eqoa.files.load_json('@journal_owner/data/journal.json');assert(id,tostring(e));local no,e=eqoa.files.load_json('@journal_owner/data/secret.json');assert(no==nil and e.code=='permission_denied','undeclared read accepted');assert(eqoa.files.load_json('@journal_owner/data/../secret.json')==nil,'traversal accepted');assert(eqoa.files.write('@journal_owner/data/journal.json','{}')==nil,'shared write accepted')").unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !reader.lua.globals().get::<bool>("loaded").unwrap() {
            assert!(std::time::Instant::now() < deadline);
            super::super::native_events::dispatch(&reader).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        reader.execute("changes=0;eqoa.events.on('shared.changed',function(e) assert(e.data.mod=='journal_owner' and e.data.path=='data/journal.json');changes=changes+1 end)").unwrap();
        owner
            .execute("assert(eqoa.files.save_json_async('journal.json',{Bat={}}))")
            .unwrap();
        while reader.lua.globals().get::<u32>("changes").unwrap() == 0 {
            assert!(std::time::Instant::now() < deadline);
            super::super::native_events::dispatch(&owner).unwrap();
            super::super::native_events::dispatch(&reader).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(
            fs::read_to_string(root.join("files/data/journal.json")).unwrap(),
            r#"{"Bat":{}}"#
        );
        super::super::native_events::clear(&owner);
        reader.execute("local id,e=eqoa.files.load_json('@journal_owner/data/journal.json');assert(id==nil and e.code=='permission_denied')").unwrap();
    }

    #[test]
    fn live_regression_optional_forge_file_probe_is_not_a_visible_error() {
        let (_scratch, host, _addon) = fixture();
        host.execute("local f,err=io.open(resource..'/optional-missing.cfg','r');assert(f==nil and type(err)=='string')").unwrap();
        let diagnostics = super::super::native::finish(&host.lua);
        assert!(
            diagnostics.is_empty(),
            "normal missing-file probe must be quiet: {diagnostics:?}"
        );
        host.execute(
            "local f,err=io.open(resource..'/bad.cfg','a');assert(f==nil and type(err)=='string')",
        )
        .unwrap();
        assert!(
            !super::super::native::finish(&host.lua).is_empty(),
            "unsupported modes must stay visible"
        );
    }

    #[test]
    fn forge_review_writes_stay_buffered_until_close() {
        let (scratch, host, _addon) = fixture();
        let registry = registry(&host.lua).unwrap();
        host.execute("pending=assert(io.open(resource..'/buffer.cfg','w'));for i=1,64 do pending:write('x') end").unwrap();
        let target = scratch.0.join("config/files/pack/resources/buffer.cfg");
        assert_eq!(
            fs::read(&target).unwrap(),
            b"",
            "write must not replace the disk file"
        );
        assert_eq!(registry.borrow().replacements, 0);
        host.execute("assert(pending:close());pending=nil").unwrap();
        assert_eq!(fs::read(target).unwrap(), vec![b'x'; 64]);
        assert_eq!(registry.borrow().replacements, 1);
    }

    #[test]
    fn forge_review_file_flush_gc_and_unload_persist_once() {
        let (scratch, host, _addon) = fixture();
        let registry = registry(&host.lua).unwrap();
        host.execute("f=assert(io.open(resource..'/flush.cfg','w'));f:write('one');assert(f:flush());assert(f:flush());assert(f:close())").unwrap();
        assert_eq!(
            registry.borrow().replacements,
            1,
            "clean flush and close must not write again"
        );
        host.execute("local f=assert(io.open(resource..'/gc.cfg','w'));f:write('gc')")
            .unwrap();
        host.lua.gc_collect().unwrap();
        assert_eq!(registry.borrow().replacements, 2);
        host.execute(
            "pending=assert(io.open(resource..'/unload.cfg','w'));pending:write('unload')",
        )
        .unwrap();
        assert!(close_all(&host.lua).is_empty());
        assert_eq!(registry.borrow().replacements, 3);
        host.execute("local ok=pcall(function() pending:write('closed') end);assert(not ok)")
            .unwrap();
        host.execute("pending=assert(io.open(resource..'/drop.cfg','w'));pending:write('drop')")
            .unwrap();
        drop(host);
        assert_eq!(registry.borrow().replacements, 4);
        for (name, data) in [
            ("flush", "one"),
            ("gc", "gc"),
            ("unload", "unload"),
            ("drop", "drop"),
        ] {
            assert_eq!(
                fs::read(
                    scratch
                        .0
                        .join(format!("config/files/pack/resources/{name}.cfg"))
                )
                .unwrap(),
                data.as_bytes()
            );
        }
    }

    #[test]
    fn forge_review_reserved_devices_rejected_before_os_open() {
        let (_scratch, host, addon) = fixture();
        for name in [
            "CON",
            "prn.cfg",
            "Aux",
            "nul.txt",
            "COM1",
            "com9.log",
            "LPT1",
            "lpt9.cfg",
            "CONIN$",
            "conout$.txt",
            "COM\u{b9}",
            "LPT\u{b2}.cfg",
            "COM\u{b3}",
        ] {
            let path = normalized(&addon.script_root.join("resources").join(name));
            assert!(
                relative(&addon, &path).is_err(),
                "reserved device accepted: {name}"
            );
            host.lua.globals().set("device_path", path).unwrap();
            host.execute(
                "local f,err=io.open(device_path,'r');assert(f==nil and type(err)=='string')",
            )
            .unwrap();
        }
        assert!(
            relative(
                &addon,
                &normalized(&addon.script_root.join("resources/COM0.cfg"))
            )
            .is_ok()
        );
        assert!(
            relative(
                &addon,
                &normalized(&addon.script_root.join("resources/CON/child.cfg"))
            )
            .is_err()
        );
    }

    #[test]
    fn forge_review_non_ascii_root_never_panics_on_case_comparison() {
        let (_scratch, _host, mut addon) = fixture();
        addon.script_root = PathBuf::from("C:/\u{130}\u{130}/\u{e9}");
        addon.package_root = addon.script_root.join("pack");
        let result = std::panic::catch_unwind(|| {
            relative(&addon, "C:/i\u{307}i\u{307}/\u{e9}/resources/a.cfg")
        });
        assert!(result.is_ok(), "non-ASCII case folding must not panic");
        assert!(result.unwrap().is_err());
    }
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn drive_json(h: &Host, flag: &str) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        while !h.lua.globals().get::<bool>(flag).unwrap() {
            assert!(
                std::time::Instant::now() < deadline,
                "JSON completion timeout"
            );
            super::super::native_events::dispatch(h).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
    #[test]
    fn round4_json_calls_keep_ordinary_heap_and_manifest_opt_in() {
        let (_scratch, h, addon) = fixture();
        fs::write(addon.package_root.join("resources/a.json"), b"{}").unwrap();
        h.execute("assert(eqoa.files.load_json('package_resources/a.json'))")
            .unwrap();
        assert_eq!(
            h.lua.set_memory_limit(8 * 1024 * 1024).unwrap(),
            8 * 1024 * 1024,
            "ordinary mod may not raise its heap by calling JSON"
        );
    }
    #[test]
    fn round4_json_memory_failure_delivers_failed_without_faulting_host() {
        let (_scratch, h, addon) = fixture();
        fs::write(
            addon.package_root.join("resources/nodes.json"),
            format!(
                "[{}]",
                vec!["{\"name\":\"a long name\",\"pos\":[1,2,3]}"; 25000].join(",")
            ),
        )
        .unwrap();
        h.execute("finished=false;eqoa.events.on('resource.failed',function(e) assert(e.data.request==req and e.data.kind=='json');finished=true end);req=assert(eqoa.files.load_json('package_resources/nodes.json'))").unwrap();
        h.lua
            .set_memory_limit(h.lua.used_memory() + 256 * 1024)
            .unwrap();
        drive_json(&h, "finished");
        h.execute("assert(1+1==2)").unwrap();
    }
    #[test]
    fn round4_json_private_journal_flushes_on_drop_and_reloads() {
        let (scratch, h, addon) = fixture();
        h.lua.set_memory_limit(crate::fork_ext::addons::async_json::JSON_HEAP).unwrap();
        let data = h
            .lua
            .app_data_ref::<super::super::sandbox::Mounts>()
            .unwrap()
            .data
            .clone()
            .unwrap();
        h.execute("assert(eqoa.files.save_json_async('journal.json',{stage=99,payload=string.rep('x',14*1024*1024-128)}))").unwrap();
        drop(h);
        let file = data.join("files/data/journal.json");
        let bytes = fs::read(&file).expect("shutdown must flush accepted save before returning");
        assert!(bytes.len() > 13 * 1024 * 1024);
        let next = Host::new().unwrap();
        next.lua.set_memory_limit(crate::fork_ext::addons::async_json::JSON_HEAP).unwrap();
        install(&next, &addon, Some(scratch.0.join("config"))).unwrap();
        next.execute("finished=false;eqoa.events.on('resource.ready',function(e) assert(e.data.request==req and e.data.value.stage==99 and #e.data.value.payload==14*1024*1024-128);finished=true end);req=assert(eqoa.files.load_json('journal.json'))").unwrap();
        drive_json(&next, "finished");
    }
    #[test]
    fn round4_save_generations_reject_old_commit_and_disable_flushes() {
        let (scratch, old, addon) = fixture();
        let (started, release) = super::super::json_save::pause_next_commit();
        old.execute("finished=false;eqoa.events.on('resource.failed',function(e) assert(e.data.request==req and e.data.kind=='json_saved');finished=true end);req=assert(eqoa.files.save_json_async('journal.json',{stage=1}))").unwrap();
        started
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        let next = Host::new().unwrap();
        install(&next, &addon, Some(scratch.0.join("config"))).unwrap();
        next.lua
            .globals()
            .set(
                "new_path",
                if cfg!(windows) {
                    "JOURNAL.json"
                } else {
                    "journal.json"
                },
            )
            .unwrap();
        next.execute("assert(eqoa.files.save_json_async(new_path,{stage=2}))")
            .unwrap();
        release.send(()).unwrap();
        drive_json(&old, "finished");
        drop(old);
        let (started, release) = super::super::json_save::pause_next_commit();
        next.execute("assert(eqoa.files.save_json_async('journal.json',{stage=3}))")
            .unwrap();
        started
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        let release_thread = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(50));
            release.send(()).unwrap();
        });
        let data = next
            .lua
            .app_data_ref::<super::super::sandbox::Mounts>()
            .unwrap()
            .data
            .clone()
            .unwrap();
        super::super::native_events::clear(&next);
        let file = data.join("files/data/journal.json");
        let v: serde_json::Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
        assert_eq!(v["stage"], 3);
        release_thread.join().unwrap();
        fs::remove_file(&file).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(!file.exists(), "no late write after disable");
    }
    #[test]
    fn round4_save_timeout_is_bounded_and_cancels_late_rename() {
        let (_scratch, h, _addon) = fixture();
        let data = h
            .lua
            .app_data_ref::<super::super::sandbox::Mounts>()
            .unwrap()
            .data
            .clone()
            .unwrap();
        let (started, release) = super::super::json_save::pause_next_commit();
        h.execute("assert(eqoa.files.save_json_async('timeout.json',{stage=1}))")
            .unwrap();
        started
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        let now = std::time::Instant::now();
        drop(h);
        let elapsed = now.elapsed();
        assert!(
            elapsed >= std::time::Duration::from_secs(2)
                && elapsed < std::time::Duration::from_millis(2500),
            "bounded flush: {elapsed:?}"
        );
        release.send(()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert!(
            !data.join("files/data/timeout.json").exists(),
            "timed out writer may not commit after disable"
        );
    }
    fn rejected_json(bytes: &[u8], message: &str) {
        let (_scratch, h, addon) = fixture_with_large_data(true);
        h.lua.globals().set("expected", message).unwrap();
        fs::write(addon.package_root.join("resources/limit.json"), bytes).unwrap();
        h.execute("finished=false;eqoa.events.on('resource.failed',function(e) assert(e.data.request==req and e.data.kind=='json' and string.find(e.data.error.message,expected,1,true));finished=true end);req=assert(eqoa.files.load_json('package_resources/limit.json'))").unwrap();
        drive_json(&h, "finished");
    }
    #[test]
    fn round4_json_read_cap_is_enforced() {
        rejected_json(
            format!("{{\"v\":\"{}\"}}", "x".repeat(16 * 1024 * 1024)).as_bytes(),
            "byte limit",
        );
    }
    #[test]
    fn round4_json_root_is_enforced() {
        rejected_json(b"123", "root");
    }
    #[test]
    fn round4_json_depth_is_enforced() {
        rejected_json(
            format!("{}0{}", "[".repeat(34), "]".repeat(34)).as_bytes(),
            "depth/node",
        );
    }
    #[test]
    fn round4_json_nodes_are_enforced() {
        rejected_json(
            format!("[{}0]", "0,".repeat(2_000_000)).as_bytes(),
            "depth/node",
        );
    }
    #[test]
    fn round4_json_shared_mount_and_inflight_quota() {
        let (_scratch, h, addon) = fixture();
        fs::write(
            addon.script_root.join("resources/shared.json"),
            b"{\"shared\":true}",
        )
        .unwrap();
        h.execute("finished=false;eqoa.events.on('resource.ready',function(e) assert(e.data.value.shared and (e.data.request==a or e.data.request==b));finished=true end);a=assert(eqoa.files.load_json('resources/shared.json'));b=assert(eqoa.files.load_json('resources/shared.json'));local v,e=eqoa.files.load_json('resources/shared.json');assert(v==nil and e.code=='quota')").unwrap();
        drive_json(&h, "finished");
    }
    #[test]
    fn round4_json_worker_save_quota_reports_correlated_failure() {
        let (_scratch, h, _addon) = fixture();
        let data = h
            .lua
            .app_data_ref::<super::super::sandbox::Mounts>()
            .unwrap()
            .data
            .clone()
            .unwrap();
        fs::create_dir_all(data.join("files/data")).unwrap();
        fs::write(data.join("files/data/full"), vec![b'x'; 15 * 1024 * 1024]).unwrap();
        h.execute("finished=false;eqoa.events.on('resource.failed',function(e) assert(e.data.request==req and e.data.kind=='json_saved');finished=true end);local p='x';for i=1,21 do p=p..p end;req=assert(eqoa.files.save_json_async('journal.json',{payload=p}))").unwrap();
        drive_json(&h, "finished");
    }
    #[test]
    fn round4_catalogue_materialisation_benchmark_asserts_real_workload() {
        let (_scratch, h, addon) = fixture();
        h.lua.set_memory_limit(crate::fork_ext::addons::async_json::JSON_HEAP).unwrap();
        let bytes = include_bytes!("../../../../../mods/resources/map-entities.json");
        let v: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        fn nodes(v: &serde_json::Value) -> usize {
            1 + match v {
                serde_json::Value::Array(a) => a.iter().map(nodes).sum(),
                serde_json::Value::Object(o) => o.values().map(nodes).sum(),
                _ => 0,
            }
        }
        assert_eq!(nodes(&v), 70579);
        assert!(bytes.len() > 1_700_000);
        fs::write(addon.package_root.join("resources/catalogue.json"), bytes).unwrap();
        h.execute("finished=false;eqoa.events.on('resource.ready',function(e) assert(e.data.request==req and type(e.data.value)=='table');finished=true end);req=assert(eqoa.files.load_json('package_resources/catalogue.json'))").unwrap();
        let mut max = 0.0f64;
        let mut frames = 0;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        while !h.lua.globals().get::<bool>("finished").unwrap() {
            assert!(std::time::Instant::now() < deadline);
            let now = std::time::Instant::now();
            super::super::native_events::dispatch(&h).unwrap();
            max = max.max(now.elapsed().as_secs_f64() * 1000.);
            frames += 1;
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        println!(
            "ROUND4_CATALOGUE bytes={} nodes=70579 frames={frames} max_dispatch_ms={max:.3} release={}",
            bytes.len(),
            !cfg!(debug_assertions)
        );
        assert!(
            frames >= 70579_usize.div_ceil(512),
            "catalogue materialisation must span bounded slices"
        );
        if !cfg!(debug_assertions) {
            assert!(max < 4.0, "catalogue dispatch exceeded 4 ms: {max}");
        }
    }

    #[test]
    fn round4_disabling_releases_a_partially_materialised_json_tree() {
        let (_scratch, h, addon) = fixture_with_large_data(true);
        fs::write(
            addon.package_root.join("resources/pending.json"),
            include_bytes!("../../../../../mods/resources/map-entities.json"),
        )
        .unwrap();
        h.execute("eqoa.events.on('resource.ready',function() end);assert(eqoa.files.load_json('package_resources/pending.json'))").unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !super::super::async_json::has_build(&h.lua) {
            assert!(std::time::Instant::now() < deadline);
            super::super::native_events::dispatch(&h).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        super::super::native_events::clear(&h);
        assert!(
            !super::super::async_json::has_service(&h.lua),
            "disable drops the partial tree's Lua handles and worker channels"
        );
    }
    #[test]
    fn round4_async_json_loads_large_readonly_resource_and_reports_failure() {
        let (scratch, h, addon) = fixture_with_large_data(true);
        fs::write(
            addon.package_root.join("resources/large.json"),
            format!("{{\"payload\":\"{}\"}}", "x".repeat(1024 * 1024 + 64)),
        )
        .unwrap();
        fs::write(addon.package_root.join("resources/bad.json"), b"not json").unwrap();
        h.execute("loaded=false;failed=false;eqoa.events.on('resource.ready',function(e) if e.data.kind=='json' then assert(e.data.request==req);loaded=#e.data.value.payload==1048640 end end);eqoa.events.on('resource.failed',function(e) if e.data.kind=='json' then assert(e.data.request==req);failed=true end end);req=assert(eqoa.files.load_json('package_resources/large.json'));local x,e=eqoa.files.load_json('../outside.json');assert(x==nil and e.code=='permission_denied')").unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !h.lua.globals().get::<bool>("loaded").unwrap() {
            assert!(
                std::time::Instant::now() < deadline,
                "JSON worker did not finish"
            );
            super::super::native_events::dispatch(&h).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        h.execute("req=assert(eqoa.files.load_json('package_resources/bad.json'))")
            .unwrap();
        while !h.lua.globals().get::<bool>("failed").unwrap() {
            assert!(std::time::Instant::now() < deadline);
            super::super::native_events::dispatch(&h).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(scratch.0.exists());
        assert!(super::super::async_json::worked_off_thread(&h.lua));
    }

    #[test]
    fn round4_async_json_save_is_private_and_latest_value_wins() {
        let (_scratch, h, addon) = fixture_with_large_data(true);
        h.execute("saved=false;eqoa.events.on('resource.ready',function(e) if e.data.kind=='json_saved' and e.data.request==last then saved=true end end);assert(eqoa.files.save_json_async('journal.json',{stage=0}));payload=string.rep('x',14*1024*1024-128) ;local n,e=eqoa.files.save_json_async('../escape',{});assert(n==nil and e.code=='permission_denied');local n,e=eqoa.files.save_json_async('package_resources/no.json',{});assert(n==nil and e.code=='permission_denied');local c={};c.self=c;local n,e=eqoa.files.save_json_async('cycle.json',c);assert(n==nil and e.code=='invalid_argument')").unwrap();
        let now = std::time::Instant::now();
        h.execute("for i=1,3 do last=assert(eqoa.files.save_json_async('journal.json',{stage=i,payload=payload})) end").unwrap();
        let caller_ms = now.elapsed().as_secs_f64() * 1000.0;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        while !h.lua.globals().get::<bool>("saved").unwrap() {
            assert!(
                std::time::Instant::now() < deadline,
                "save worker did not finish"
            );
            super::super::native_events::dispatch(&h).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let mounts = h
            .lua
            .app_data_ref::<super::super::sandbox::Mounts>()
            .unwrap();
        let key = mounts.key("journal.json").unwrap();
        let file = mounts.data.as_ref().unwrap().join("files").join(key);
        let saved: serde_json::Value = serde_json::from_slice(&fs::read(file).unwrap()).unwrap();
        assert_eq!(saved["stage"], 3);
        assert_eq!(
            saved["payload"].as_str().unwrap().len(),
            14 * 1024 * 1024 - 128
        );
        assert!(!addon.package_root.join("resources/no.json").exists());
        assert!(super::super::json_save::worked_off_thread(&h.lua));
        println!(
            "ROUND4_ASYNC_SAVE snapshots=3 payload_bytes={} caller_ms={:.3}",
            14 * 1024 * 1024 - 128,
            caller_ms
        );
    }

    fn fixture() -> (Fixture, Host, DiscoveredAddon) {
        fixture_with_large_data(false)
    }
    fn fixture_with_large_data(large_data: bool) -> (Fixture, Host, DiscoveredAddon) {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "eqoa-addon-io-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("scripts/pack/resources")).unwrap();
        fs::create_dir_all(root.join("scripts/resources")).unwrap();
        let scripts = root.join("scripts").canonicalize().unwrap();
        let addon = DiscoveredAddon {
            id: "pack".into(),
            name: "pack".into(),
            entry: scripts.join("pack/pack.lua"),
            script_root: scripts.clone(),
            package_root: scripts.join("pack"),
            module_dirs: vec![],
            resource_dirs: vec![],
            api: None,
            entry_mode: Default::default(),
            capabilities_declared: true,
            requested_capabilities: if large_data {
                vec!["large_data".into()]
            } else {
                vec![]
            },
            extensions: Default::default(),
            initially_enabled: false,
            mods_root: Some(scripts.clone()),
        };
        fs::write(scripts.join("resources/legacy.cfg"), "one\r\ntwo\n").unwrap();
        let host = Host::new().unwrap();
        install(&host, &addon, Some(root.join("config"))).unwrap();
        host.lua
            .globals()
            .set("resource", normalized(&scripts.join("pack/resources")))
            .unwrap();
        host.lua
            .globals()
            .set("legacy", normalized(&scripts.join("resources/legacy.cfg")))
            .unwrap();
        (Fixture(root), host, addon)
    }

    #[test]
    fn forge_modern_io_roundtrip_lines_write_close_and_overlay_removal() {
        let (scratch, host, addon) = fixture();
        host.execute(r#"
            local missing,err=io.open(resource..'/missing.cfg');assert(missing==nil and type(err)=='string')
            local f=assert(io.open(resource..'/notes.cfg','w'))
            assert(f:write('Bat Eyes|Get eyes\n','The Militia|Return\n')==f);assert(f:close()==true)
            assert(not pcall(function() f:write('closed') end))
            local r=assert(io.open(resource..'/notes.cfg'));local lines=r:lines()
            assert(lines()=='Bat Eyes|Get eyes' and lines()=='The Militia|Return' and lines()==nil);r:close()
            local old=assert(io.open(legacy));local n=0;for line in old:lines() do n=n+1;assert(line==({'one','two'})[n]) end;assert(n==2);old:close()
            assert(select('#',os.remove(legacy))==1);assert(io.open(legacy)==nil);assert(os.remove(legacy)==nil)
        "#).unwrap();
        assert!(
            !addon.script_root.join("pack/resources/notes.cfg").exists(),
            "source package must remain read-only"
        );
        assert!(
            addon.script_root.join("resources/legacy.cfg").is_file(),
            "migration tombstones must preserve the source"
        );
        install(&host, &addon, Some(scratch.0.join("config"))).unwrap();
        host.execute("assert(io.open(legacy)==nil);local f=assert(io.open(resource..'/notes.cfg'));assert(f:lines()()=='Bat Eyes|Get eyes');f:close();assert(os.remove(resource..'/notes.cfg'));assert(io.open(resource..'/notes.cfg')==nil)").unwrap();
    }

    #[test]
    fn forge_modern_io_rejects_traversal_code_other_addons_and_oversized_files() {
        let (scratch, host, addon) = fixture();
        host.execute(r#"
            for _,path in ipairs({'C:/Windows/win.ini', resource..'/../../pack.lua', resource..'/../resources/a', resource..'/a:stream', resource..'/../../../other/resources/a'}) do
                assert(io.open(path,'w')==nil);assert(os.remove(path)==nil)
            end
            assert(io.open(resource..'/bad.cfg','a')==nil)
            local f=assert(io.open(resource..'/large.cfg','w'))
            assert(not pcall(function() f:write(string.rep('x',1048577)) end));f:close()
        "#).unwrap();
        fs::write(
            addon.script_root.join("resources/oversize.cfg"),
            vec![b'x'; MAX_BYTES + 1],
        )
        .unwrap();
        host.lua
            .globals()
            .set(
                "oversize",
                normalized(&addon.script_root.join("resources/oversize.cfg")),
            )
            .unwrap();
        host.execute("assert(io.open(oversize)==nil)").unwrap();
        assert!(!scratch.0.join("other").exists());
    }

    #[test]
    fn forge_modern_io_bounds_file_count_before_creating_more_files() {
        let (_scratch, host, _addon) = fixture();
        for i in 1..=64 {
            host.execute(&format!(
                "local f,err=io.open(resource..'/{i}.cfg','w');assert(f,err);f:close()"
            ))
            .unwrap();
            host.lua.gc_collect().unwrap();
        }
        host.execute("assert(io.open(resource..'/65.cfg','w')==nil)")
            .unwrap();
        host.execute("assert(os.remove(resource..'/1.cfg'));local f=assert(io.open(resource..'/65.cfg','w'));f:close()").unwrap();
    }

    #[test]
    fn forge_modern_io_iterator_gc_keeps_memory_budget_and_close_releases_it() {
        let (_scratch, host, _addon) = fixture();
        host.execute("closed={};for i=1,32 do local f=assert(io.open(legacy));closed[i]=f;f:close() end;iters={}").unwrap();
        for i in 1..=16 {
            host.execute(&format!(
                "local f=assert(io.open(legacy));iters[{i}]=f:lines()"
            ))
            .unwrap();
            host.lua.gc_collect().unwrap();
        }
        host.execute("assert(io.open(legacy)==nil);assert(iters[1]()=='one');assert(iters[1]()=='two');iters=nil").unwrap();
        host.lua.gc_collect().unwrap();
        host.execute("local f=assert(io.open(legacy));local next=f:lines();assert(next()=='one');local other=f:lines();assert(other()=='two');f:close();assert(not pcall(next));local reopened=assert(io.open(legacy));reopened:close()").unwrap();
    }
    #[test]
    fn review_fix_m3_native_files_report_boundary_denials_separately_from_io() {
        let (_scratch, host, _addon) = fixture();
        host.execute(r#"for _,path in ipairs({'../neighbor/secret.cfg','resources/../secret.cfg','..\\secret.cfg','CON','data/./bad'}) do
            for _,op in ipairs({'read','remove','write'}) do
                local value,err=eqoa.files[op](path,'bytes')
                assert(value==nil and err.code=='permission_denied',path..' '..op..' '..tostring(err and err.code))
            end
        end
        local value,err=eqoa.files.read('absent.cfg');assert(value==nil and err.code=='not_found')
        "#).unwrap();
    }
    #[test]
    fn round4b_optional_json_missing_is_not_found_without_warning() {
        let (_scratch, host, _addon) = fixture();
        host.execute("local value,err=eqoa.files.load_json('missing.json');assert(value==nil and err.code=='not_found')").unwrap();
        assert!(
            super::super::native::finish(&host.lua).is_empty(),
            "fresh journal probe must stay quiet"
        );
        host.execute("local value,err=eqoa.files.load_json('../denied.json');assert(value==nil and err.code=='permission_denied')").unwrap();
        assert!(
            !super::super::native::finish(&host.lua).is_empty(),
            "denied path must still diagnose"
        );
    }
}
