//! Small Lua 5.1 text-file subset. Resource paths are a virtual overlay: reads
//! can import existing resources; all writes/deletions live in addon config.
//! No source package, other addon, or arbitrary host file is writable.
use super::{discovery::DiscoveredAddon, host::Host};
use mlua::{Result, UserData, UserDataMethods, Value};
use std::{
    cell::{Cell, RefCell},
    fs,
    io::Read,
    path::{Path, PathBuf},
    rc::Rc,
};

const MAX_BYTES: usize = 1024 * 1024;

struct TextFile {
    bytes: Rc<RefCell<Vec<u8>>>,
    target: Option<PathBuf>,
    closed: Rc<Cell<bool>>,
    live: Rc<RefCell<Option<Rc<()>>>>,
    cursor: Rc<Cell<usize>>,
}

impl UserData for TextFile {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_method("lines", |lua, file, ()| {
            if file.closed.get() || file.target.is_some() {
                return Err(mlua::Error::runtime("file is not readable"));
            }
            let bytes = file.bytes.clone();
            let closed = file.closed.clone();
            let cursor = file.cursor.clone();
            let live = file.live.clone();
            lua.create_function_mut(move |lua, ()| {
                let _keep_live = &live; // iterator retains the allocation quota through userdata GC
                if closed.get() {
                    return Err(mlua::Error::runtime("file is closed"));
                }
                let bytes = bytes.borrow();
                if cursor.get() >= bytes.len() {
                    return Ok(None);
                }
                let start = cursor.get();
                let end = bytes[start..]
                    .iter()
                    .position(|c| *c == b'\n')
                    .map(|n| start + n)
                    .unwrap_or(bytes.len());
                cursor.set((end + 1).min(bytes.len()));
                let end = if end > start && bytes[end - 1] == b'\r' {
                    end - 1
                } else {
                    end
                };
                Ok(Some(lua.create_string(&bytes[start..end])?))
            })
        });
        m.add_function(
            "write",
            |_, (ud, values): (mlua::AnyUserData, mlua::Variadic<mlua::String>)| {
                {
                    let file = ud.borrow::<TextFile>()?;
                    if file.closed.get() || file.target.is_none() {
                        return Err(mlua::Error::runtime("file is not writable"));
                    }
                    let mut bytes = file.bytes.borrow_mut();
                    let len = values
                        .iter()
                        .try_fold(bytes.len(), |len, v| len.checked_add(v.as_bytes().len()))
                        .filter(|len| *len <= MAX_BYTES)
                        .ok_or_else(|| mlua::Error::runtime("file exceeds 1 MiB"))?;
                    let extra = len - bytes.len();
                    bytes.reserve(extra);
                    for v in values {
                        bytes.extend_from_slice(&v.as_bytes());
                    }
                    let target = file.target.as_ref().unwrap();
                    no_links(target).map_err(mlua::Error::runtime)?;
                    super::storage::atomic_replace(target, &bytes)
                        .map_err(mlua::Error::external)?;
                }
                Ok(ud)
            },
        );
        m.add_method_mut("close", |_, file, ()| {
            if file.closed.get() {
                return Err(mlua::Error::runtime("file is closed"));
            }
            file.closed.set(true);
            *file.bytes.borrow_mut() = Vec::new();
            file.live.borrow_mut().take();
            Ok(true)
        });
    }
}

fn normalized(path: &Path) -> String {
    path.to_string_lossy()
        .replace("\\\\?\\", "")
        .replace('\\', "/")
}

fn relative(addon: &DiscoveredAddon, path: &str) -> std::result::Result<PathBuf, String> {
    let path = path.replace("\\\\?\\", "").replace('\\', "/");
    if path.len() > 4096 {
        return Err("invalid addon resource path".into());
    }
    // Absolute Windows paths contain a drive prefix; compare normalized roots
    // case-insensitively there, preserving the original spelling for I/O.
    let root = normalized(&addon.script_root);
    let prefix = format!("{root}/");
    let starts = if cfg!(windows) {
        path.to_lowercase().starts_with(&prefix.to_lowercase())
    } else {
        path.starts_with(&prefix)
    };
    if !starts {
        return Err("file path outside addon resources".into());
    }
    let tail = &path[prefix.len()..];
    if tail.split('/').count() > 8
        || tail.split('/').any(|s| {
            s == ".." || s == "." || s.is_empty() || s.contains(':') || s.ends_with([' ', '.'])
        })
    {
        return Err("invalid addon resource path".into());
    }
    let rel = PathBuf::from(tail);
    let source = addon.script_root.join(&rel);
    let private = addon.package_root.join("resources");
    let shared = addon.script_root.join("resources");
    if !source.starts_with(&private) && !source.starts_with(&shared) {
        return Err("file path outside addon resources".into());
    }
    Ok(rel)
}

fn read(path: &Path) -> std::result::Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_BYTES {
        return Err("file exceeds 1 MiB".into());
    }
    Ok(bytes)
}

// Reject symlinks in every existing component, including dangling links. This
// guards both reads and writes before create_dir_all/atomic_replace follows them.
fn no_links(path: &Path) -> std::result::Result<(), String> {
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
            } else {
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
    let io = lua.create_table()?;
    let addon_open = addon.clone();
    let data_open = data.clone();
    // At most 16 live handles and 1 MiB per file; weak references release the
    // live-handle budget on Lua GC without exposing a native file descriptor.
    let handles = Rc::new(RefCell::new(Vec::<std::rc::Weak<()>>::new()));
    io.set(
        "open",
        lua.create_function(move |lua, (path, mode): (String, Option<String>)| {
            {
                let mut handles = handles.borrow_mut();
                handles.retain(|h| h.strong_count() > 0);
                if handles.len() >= 16 {
                    return lua.pack_multi((Value::Nil, "file handle limit exceeded"));
                }
            }
            let result = (|| -> std::result::Result<TextFile, String> {
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
                    overlay_room(data_open.as_ref().unwrap(), &target, deleted.as_deref())?;
                    super::storage::atomic_replace(&target, &[]).map_err(|e| e.to_string())?;
                    if let Some(deleted) = deleted {
                        if deleted.exists() {
                            fs::remove_file(deleted).map_err(|e| e.to_string())?;
                        }
                    }
                    Ok(TextFile {
                        bytes: Rc::new(RefCell::new(Vec::new())),
                        target: Some(target),
                        closed: Rc::new(Cell::new(false)),
                        live: Rc::new(RefCell::new(None)),
                        cursor: Rc::new(Cell::new(0)),
                    })
                } else {
                    if deleted.as_ref().is_some_and(|p| p.exists()) {
                        return Err("file not found".into());
                    }
                    let source = target
                        .filter(|p| p.exists())
                        .unwrap_or_else(|| addon_open.script_root.join(rel));
                    no_links(&source)?;
                    Ok(TextFile {
                        bytes: Rc::new(RefCell::new(read(&source)?)),
                        target: None,
                        closed: Rc::new(Cell::new(false)),
                        live: Rc::new(RefCell::new(None)),
                        cursor: Rc::new(Cell::new(0)),
                    })
                }
            })();
            match result {
                Ok(file) => {
                    let mut handles = handles.borrow_mut();
                    handles.retain(|h| h.strong_count() > 0);
                    let token = Rc::new(());
                    handles.push(Rc::downgrade(&token));
                    *file.live.borrow_mut() = Some(token);
                    let ud = lua.create_userdata(file)?;
                    lua.pack_multi(ud)
                }
                Err(error) => lua.pack_multi((Value::Nil, error)),
            }
        })?,
    )?;
    lua.globals().set("io", io)?;
    let os: mlua::Table = lua.globals().get("os")?;
    let addon = addon.clone();
    os.set(
        "remove",
        lua.create_function(move |lua, path: String| {
            let result = (|| -> std::result::Result<(), String> {
                let rel = relative(&addon, &path)?;
                let root = data.as_ref().ok_or("addon data directory unavailable")?;
                let target = root.join("files").join(&rel);
                let deleted = root.join("removed").join(&rel);
                no_links(&target)?;
                no_links(&deleted)?;
                if deleted.exists() || (!target.exists() && !addon.script_root.join(&rel).is_file())
                {
                    return Err("file not found".into());
                }
                if addon.script_root.join(&rel).is_file() {
                    overlay_room(root, &deleted, Some(&target))?;
                    super::storage::atomic_replace(&deleted, &[]).map_err(|e| e.to_string())?;
                }
                if target.exists() {
                    fs::remove_file(target).map_err(|e| e.to_string())?;
                }
                Ok(())
            })();
            match result {
                Ok(()) => lua.pack_multi(true),
                Err(e) => lua.pack_multi((Value::Nil, e)),
            }
        })?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
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
