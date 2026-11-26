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
pub(crate) const JSON_HEAP: usize = 256 * 1024 * 1024;
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
    building: Option<Build>,
}
type Shared = Rc<RefCell<Service>>;
pub(crate) fn close(lua: &Lua) {
    drop(lua.remove_app_data::<Shared>());
}
#[cfg(test)]
pub(crate) fn has_build(lua: &Lua) -> bool {
    lua.app_data_ref::<Shared>()
        .is_some_and(|s| s.borrow().building.is_some())
}
#[cfg(test)]
pub(crate) fn has_service(lua: &Lua) -> bool {
    lua.app_data_ref::<Shared>().is_some()
}
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
// Bound structure before serde allocates its tree. Keys count too (conservative).
fn preflight(bytes: &[u8]) -> std::result::Result<(), String> {
    let (mut string, mut escape, mut atom, mut depth, mut nodes) =
        (false, false, false, 0usize, 0usize);
    for &b in bytes {
        if string {
            if escape {
                escape = false;
            } else if b == b'\\' {
                escape = true;
            } else if b == b'"' {
                string = false;
            }
            continue;
        }
        match b {
            b'"' => {
                string = true;
                nodes += 1;
                atom = false;
            }
            b'[' | b'{' => {
                depth += 1;
                nodes += 1;
                atom = false;
            }
            b']' | b'}' => {
                depth = depth.saturating_sub(1);
                atom = false;
            }
            b',' | b':' | b' ' | b'\r' | b'\n' | b'\t' => atom = false,
            _ => {
                if !atom {
                    nodes += 1;
                    atom = true;
                }
            }
        }
        if depth > 33 || nodes > NODES {
            return Err("JSON exceeds depth/node quota".into());
        }
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
                    preflight(&bytes)?;
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
        building: None,
    }));
    lua.set_app_data(s.clone());
    Ok(s)
}
fn resource_path(m: &Mounts, path: &str) -> std::result::Result<Option<PathBuf>, String> {
    if path.starts_with('@') {
        return super::exchange::read(path);
    }
    if path.starts_with(['/', '\\']) || path.contains(':') {
        return Err("JSON loads require a relative resource path".into());
    }
    let key = m.key(path)?;
    if key.starts_with("data") {
        return m.read_path(&key).map(Some);
    }
    // Packaged mounts bypass overlays; private names use only this mod's storage.
    Ok(Some(m.addon.script_root.join(key)))
}
enum Key {
    Index(usize),
    Field(String),
}
enum Task {
    Visit(Json, Option<(Table, Key)>),
    Array(Table, std::iter::Enumerate<std::vec::IntoIter<Json>>),
    Object(Table, serde_json::map::IntoIter),
}
struct Build {
    id: u64,
    root: Option<Value>,
    stack: Vec<Task>,
}
impl Build {
    fn new(id: u64, value: Json) -> Self {
        Self {
            id,
            root: None,
            stack: vec![Task::Visit(value, None)],
        }
    }
    fn step(&mut self, lua: &Lua) -> Result<bool> {
        let start = std::time::Instant::now();
        let mut nodes = 0;
        while let Some(task) = self.stack.pop() {
            match task {
                Task::Visit(v, parent) => {
                    nodes += 1;
                    let value = match v {
                        Json::Null => lua.named_registry_value("eqoa_json_null")?,
                        Json::Bool(v) => Value::Boolean(v),
                        Json::Number(v) => Value::Number(v.as_f64().unwrap()),
                        Json::String(v) => Value::String(lua.create_string(v)?),
                        Json::Array(a) => {
                            let t = lua.create_table()?;
                            t.set_metatable(Some(lua.named_registry_value("eqoa_json_array")?))?;
                            self.stack
                                .push(Task::Array(t.clone(), a.into_iter().enumerate()));
                            Value::Table(t)
                        }
                        Json::Object(o) => {
                            let t = lua.create_table()?;
                            self.stack.push(Task::Object(t.clone(), o.into_iter()));
                            Value::Table(t)
                        }
                    };
                    if let Some((t, key)) = parent {
                        match key {
                            Key::Index(i) => t.raw_set(i, value)?,
                            Key::Field(k) => t.raw_set(k, value)?,
                        }
                    } else {
                        self.root = Some(value);
                    }
                }
                Task::Array(t, mut iter) => {
                    if let Some((i, v)) = iter.next() {
                        self.stack.push(Task::Array(t.clone(), iter));
                        self.stack
                            .push(Task::Visit(v, Some((t, Key::Index(i + 1)))));
                    }
                }
                Task::Object(t, mut iter) => {
                    if let Some((k, v)) = iter.next() {
                        self.stack.push(Task::Object(t.clone(), iter));
                        self.stack.push(Task::Visit(v, Some((t, Key::Field(k)))));
                    }
                }
            }
            if nodes >= 512 || start.elapsed() >= std::time::Duration::from_millis(1) {
                break;
            }
        }
        Ok(self.stack.is_empty())
    }
}
pub(crate) fn poll(lua: &Lua) -> Result<()> {
    let Some(s) = lua.app_data_ref::<Shared>().map(|s| s.clone()) else {
        return Ok(());
    };
    let mut build = s.borrow_mut().building.take();
    if build.is_none() {
        let item = s.borrow().rx.try_recv();
        match item {
            Ok(done) => {
                s.borrow_mut().last_worker = Some(done.worker);
                match done.result {
                    Ok(v) => build = Some(Build::new(done.id, v)),
                    Err(e) => {
                        {
                            let mut service = s.borrow_mut();
                            service.pending = service.pending.saturating_sub(1);
                        }
                        super::native_events::json_resource(lua, done.id, None, Some(&e))?;
                        return Ok(());
                    }
                }
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                s.borrow_mut().pending = 0;
                super::native::diagnose(
                    lua,
                    "eqoa.files.load_json",
                    "failed",
                    "JSON worker disconnected",
                );
                return Ok(());
            }
            Err(mpsc::TryRecvError::Empty) => return Ok(()),
        }
    }
    let mut build = build.unwrap();
    match build.step(lua) {
        Ok(false) => s.borrow_mut().building = Some(build),
        Ok(true) => {
            let value = build.root.take().unwrap();
            let id = build.id;
            drop(build);
            s.borrow_mut().pending -= 1;
            super::native_events::json_resource(lua, id, Some(value), None)?;
        }
        Err(_) => {
            let id = build.id;
            drop(build);
            s.borrow_mut().pending -= 1;
            lua.gc_collect()?;
            super::native_events::json_resource(
                lua,
                id,
                None,
                Some("JSON exceeds Lua memory; large data requires manifest capability large_data"),
            )?;
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
                Ok(Some(path)) => path,
                Ok(None) => {
                    return super::native::quiet_error(
                        lua,
                        "eqoa.files.load_json",
                        "not_yet",
                        "shared file owner is still loading",
                    );
                }
                Err(e) => {
                    return error(
                        lua,
                        "eqoa.files.load_json",
                        if e.starts_with("file not found") {
                            "not_found"
                        } else {
                            "permission_denied"
                        },
                        &e,
                    );
                }
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
        assert_eq!(
            hash,
            "37be57d7996fbf7e7ef936c3e6ced1c3a14f611462346bc204eba5929cce11ca"
        );
        assert_eq!(
            bytes,
            include_bytes!(
                "../../../../../bundled-mods/eqoa_map_search/resources/map-entities.json"
            )
        );
        assert!(bytes.len() > 1024 * 1024);
        let mut bad = bytes.to_vec();
        bad[0] ^= 1;
        assert_ne!(format!("{:x}", Sha256::digest(&bad)), hash);
    }

    #[test]
    #[ignore = "historical unsandboxed baseline; live save and catalogue benchmarks cover current paths"]
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
