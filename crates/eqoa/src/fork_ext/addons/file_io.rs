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
            super::storage::atomic_replace(target, &self.bytes).map_err(|e| e.to_string())?;
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
    lua.set_app_data(super::sandbox::Mounts {
        addon: addon.clone(),
        data: data.clone(),
    });
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
                    super::storage::atomic_replace(&target, &[]).map_err(|e| e.to_string())?;
                    if let Some(deleted) = deleted {
                        if deleted.exists() {
                            fs::remove_file(deleted).map_err(|e| e.to_string())?;
                        }
                    }
                    Ok(FileState {
                        bytes: Vec::new(),
                        target: Some(target),
                        closed: false,
                        cursor: 0,
                        dirty: false,
                        registry: registry.clone(),
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
                    super::native::diagnose(lua, "io.open", code, &error);
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

pub(super) fn install_native(host: &Host) -> Result<()> {
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
                            } else if e.contains("invalid") || e.contains("outside") {
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
    fn fixture() -> (Fixture, Host, DiscoveredAddon) {
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
            requested_capabilities: vec![],
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
}
