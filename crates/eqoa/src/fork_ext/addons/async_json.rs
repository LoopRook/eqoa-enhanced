//! Bounded read-only JSON decoding off the game thread. Lua table materialization stays on its VM.
use super::{
    host::Host,
    native::{bind, error},
    sandbox::Mounts,
};
use mlua::{Lua, Result, Table, UserData, Value};
use serde_json::Value as Json;
use std::{
    cell::RefCell,
    path::PathBuf,
    rc::Rc,
    sync::mpsc::{self, Receiver, SyncSender},
    thread,
};
pub(crate) const READ_CAP: usize = 16 * 1024 * 1024;
pub(crate) const JSON_HEAP: usize = 128 * 1024 * 1024;
const NODES: usize = 2_000_000;
struct JsonNull;
impl UserData for JsonNull {}
struct Load {
    id: u64,
    path: PathBuf,
}
struct Done {
    id: u64,
    result: std::result::Result<Json, String>,
    worker: thread::ThreadId,
}
struct Service {
    tx: SyncSender<Load>,
    rx: Receiver<Done>,
    next: u64,
    pending: usize,
    last_worker: Option<thread::ThreadId>,
}
type Shared = Rc<RefCell<Service>>;
fn validate(value: &Json, depth: usize, left: &mut usize) -> std::result::Result<(), String> {
    if depth > 32 || *left == 0 {
        return Err("JSON exceeds depth/node quota".into());
    }
    *left -= 1;
    match value {
        Json::Array(a) => {
            for v in a {
                validate(v, depth + 1, left)?;
            }
        }
        Json::Object(o) => {
            for v in o.values() {
                validate(v, depth + 1, left)?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn service(lua: &Lua) -> Result<Shared> {
    if let Some(s) = lua.app_data_ref::<Shared>() {
        return Ok(s.clone());
    }
    let (tx, requests) = mpsc::sync_channel::<Load>(2);
    let (done, rx) = mpsc::sync_channel(2);
    thread::Builder::new()
        .name("addon-json".into())
        .spawn(move || {
            while let Ok(job) = requests.recv() {
                let result = (|| {
                    let bytes = super::sandbox::read_bounded(&job.path, READ_CAP)?;
                    let v: Json = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
                    if !v.is_object() && !v.is_array() {
                        return Err("JSON root must be an object or array".into());
                    }
                    let mut left = NODES;
                    validate(&v, 0, &mut left)?;
                    Ok(v)
                })();
                if done
                    .send(Done {
                        id: job.id,
                        result,
                        worker: thread::current().id(),
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .map_err(mlua::Error::external)?;
    let s = Rc::new(RefCell::new(Service {
        tx,
        rx,
        next: 0,
        pending: 0,
        last_worker: None,
    }));
    lua.set_app_data(s.clone());
    Ok(s)
}
fn resource_path(m: &Mounts, path: &str) -> std::result::Result<PathBuf, String> {
    if path.starts_with(['/', '\\']) || path.contains(':') {
        return Err("JSON loads require a relative resource path".into());
    }
    let key = m.key(path)?;
    if key.starts_with("data") {
        return Err("JSON loads accept read-only resource mounts only".into());
    }
    // Deliberately bypass private overlays: load_json never reads mutable addon data.
    Ok(m.addon.script_root.join(key))
}
fn from_json(lua: &Lua, v: &Json) -> Result<Value> {
    Ok(match v {
        Json::Null => lua.named_registry_value("eqoa_json_null")?,
        Json::Bool(v) => Value::Boolean(*v),
        Json::Number(v) => Value::Number(
            v.as_f64()
                .ok_or_else(|| mlua::Error::runtime("invalid JSON number"))?,
        ),
        Json::String(v) => Value::String(lua.create_string(v)?),
        Json::Array(a) => {
            let t = lua.create_table_with_capacity(a.len(), 0)?;
            for (i, v) in a.iter().enumerate() {
                t.raw_set(i + 1, from_json(lua, v)?)?;
            }
            t.set_metatable(Some(lua.named_registry_value("eqoa_json_array")?))?;
            Value::Table(t)
        }
        Json::Object(o) => {
            let t = lua.create_table_with_capacity(0, o.len())?;
            for (k, v) in o {
                t.raw_set(k.as_str(), from_json(lua, v)?)?;
            }
            Value::Table(t)
        }
    })
}
pub(crate) fn poll(lua: &Lua) -> Result<()> {
    let Some(s) = lua.app_data_ref::<Shared>().map(|s| s.clone()) else {
        return Ok(());
    };
    loop {
        let item = s.borrow_mut().rx.try_recv();
        let Ok(done) = item else { break };
        {
            let mut s = s.borrow_mut();
            s.pending = s.pending.saturating_sub(1);
            s.last_worker = Some(done.worker);
        }
        match done.result {
            Ok(v) => {
                super::native_events::json_resource(lua, done.id, Some(from_json(lua, &v)?), None)?
            }
            Err(e) => super::native_events::json_resource(lua, done.id, None, Some(&e))?,
        }
    }
    Ok(())
}
pub(crate) fn install(host: &Host) -> Result<()> {
    let lua = &host.lua;
    let null = lua.create_userdata(JsonNull)?;
    lua.set_named_registry_value("eqoa_json_null", null.clone())?;
    let array = lua.create_table()?;
    array.set("__json_array", true)?;
    array.set("__metatable", "JSON array")?;
    lua.set_named_registry_value("eqoa_json_array", array)?;
    let modules: Table = lua.named_registry_value("eqoa_modules")?;
    modules.get::<Table>("files")?.set("null", null)?;
    super::native::describe(lua, "eqoa.files.null", "supported", Some(4), "");
    bind(
        lua,
        "files",
        "load_json",
        lua.create_function(|lua, path: String| {
            let Some(m) = lua.app_data_ref::<Mounts>().map(|m| m.clone()) else {
                return error(
                    lua,
                    "eqoa.files.load_json",
                    "unavailable",
                    "file mounts unavailable",
                );
            };
            let path = match resource_path(&m, &path) {
                Ok(p) => p,
                Err(e) => return error(lua, "eqoa.files.load_json", "permission_denied", &e),
            };
            let shared = service(lua)?;
            let mut s = shared.borrow_mut();
            if s.pending >= 2 {
                return error(
                    lua,
                    "eqoa.files.load_json",
                    "quota",
                    "at most two JSON requests in flight",
                );
            }
            lua.set_memory_limit(JSON_HEAP)?;
            s.next += 1;
            let id = s.next;
            if s.tx.try_send(Load { id, path }).is_err() {
                return error(
                    lua,
                    "eqoa.files.load_json",
                    "unavailable",
                    "JSON worker queue unavailable",
                );
            }
            s.pending += 1;
            lua.pack_multi((id, Value::Nil))
        })?,
    )
}
#[cfg(test)]
pub(crate) fn worked_off_thread(lua: &Lua) -> bool {
    lua.app_data_ref::<Shared>().is_some_and(|s| {
        s.borrow()
            .last_worker
            .is_some_and(|id| id != thread::current().id())
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn round4_shared_catalogue_retains_forge_pin() {
        use sha2::{Digest, Sha256};
        let bytes = include_bytes!("../../../../../mods/resources/map-entities.json");
        let hash = format!("{:x}", Sha256::digest(bytes));
        assert_eq!(hash, crate::map::search::CATALOG_SHA256);
        assert!(bytes.len() > 1024 * 1024);
        let mut bad = bytes.to_vec();
        bad[0] ^= 1;
        assert_ne!(format!("{:x}", Sha256::digest(&bad)), hash);
    }

    #[test]
    fn round4_measure_lua_journal_save_14_mib() {
        let lua = mlua::Lua::new();
        let root =
            std::env::temp_dir().join(format!("eqoa-journal-measure-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("journal.json");
        lua.globals().set("path", file.to_str().unwrap()).unwrap();
        lua.load(r#"journal={};for i=1,62000 do journal[string.rep('N',160)..i]={[tostring(i)]={world=0,level=15,pos={1,2,3}}} end"#).exec().unwrap();
        let now = std::time::Instant::now();
        lua.load(r#"local b={'{'};local first=true;for name,spawns in pairs(journal) do if not first then b[#b+1]=',' end;first=false;for id,s in pairs(spawns) do b[#b+1]=string.format('%q:{%q:{"world":%d,"level":%d,"pos":[%d,%d,%d]}}',name,id,s.world,s.level,s.pos[1],s.pos[2],s.pos[3]) end end;b[#b+1]='}';local text=table.concat(b);assert(#text<=14*1024*1024);text=text..string.rep(' ',14*1024*1024-#text);local f=assert(io.open(path,'wb'));assert(f:write(text));assert(f:close())"#).exec().unwrap();
        let elapsed = now.elapsed();
        let bytes = std::fs::read(&file).unwrap();
        assert_eq!(bytes.len(), 14 * 1024 * 1024);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&bytes)
                .unwrap()
                .as_object()
                .unwrap()
                .len(),
            62000
        );
        println!(
            "ROUND4_LUA_SAVE entries=62000 bytes={} frame_ms={:.3}",
            bytes.len(),
            elapsed.as_secs_f64() * 1000.0
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}

pub(super) fn is_null(v: &mlua::AnyUserData) -> bool {
    v.is::<JsonNull>()
}
