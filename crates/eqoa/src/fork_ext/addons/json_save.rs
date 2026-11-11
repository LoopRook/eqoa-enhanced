//! Private JSON saves: snapshot Lua data, encode and atomically replace on a bounded worker.
use super::{
    host::Host,
    native::{bind, error},
    sandbox::Mounts,
};
use mlua::{Lua, Result, Value};
use serde_json::{Map, Value as Json};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    io::Write,
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc, Condvar, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
const CAP: usize = 14 * 1024 * 1024;
struct Job {
    id: u64,
    key: PathBuf,
    root: PathBuf,
    value: Json,
    generation: u64,
    permit: Arc<AtomicBool>,
}
struct Done {
    id: u64,
    key: PathBuf,
    result: std::result::Result<(), String>,
    worker: thread::ThreadId,
}
#[derive(Default)]
struct Queue {
    pending: Option<Job>,
    active: Option<PathBuf>,
    done: Option<Done>,
    closed: bool,
    scheduled: bool,
}
impl Queue {
    fn push(&mut self, job: Job) -> std::result::Result<(), String> {
        if self.active.as_ref().is_some_and(|p| *p != job.key)
            || self.pending.as_ref().is_some_and(|p| p.key != job.key)
        {
            return Err("one JSON save path at a time; poll completion before another path".into());
        }
        self.pending = Some(job);
        Ok(())
    }
}
type Work = Arc<(Mutex<Queue>, Condvar)>;
// All VM generations share one writer and one per-path acceptance sequence.
static NEXT: AtomicU64 = AtomicU64::new(1);
static LATEST: OnceLock<Mutex<BTreeMap<PathBuf, u64>>> = OnceLock::new();
fn latest() -> &'static Mutex<BTreeMap<PathBuf, u64>> {
    LATEST.get_or_init(Default::default)
}
fn ordered_path(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        return PathBuf::from(path.to_string_lossy().to_lowercase());
    }
    #[cfg(not(windows))]
    {
        path
    }
}
struct Writer {
    tx: Option<mpsc::Sender<Work>>,
    join: thread::JoinHandle<()>,
    users: usize,
}
static WRITER: Mutex<Option<Writer>> = Mutex::new(None);
struct Service {
    work: Work,
    permit: Arc<AtomicBool>,
    next: u64,
    last_worker: Option<thread::ThreadId>,
}
impl Drop for Service {
    fn drop(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut q = self.work.0.lock().unwrap();
        q.closed = true;
        while q.active.is_some() || q.pending.is_some() {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            q = self.work.1.wait_timeout(q, remaining).unwrap().0;
        }
        let drained = q.active.is_none() && q.pending.is_none();
        q.pending = None;
        drop(q);
        // Same gate as the final rename: after this boundary an old generation
        // cannot recreate a cleared file, even if encoding or sync was slow.
        {
            let _gate = latest().lock().unwrap();
            self.permit.store(false, Ordering::Release);
        }
        if !drained {
            eprintln!("addon JSON save flush timed out after 2 s; pending commit cancelled");
        }
        let mut writer = WRITER.lock().unwrap();
        if let Some(w) = writer.as_mut() {
            w.users -= 1;
            if w.users == 0 && drained {
                w.tx.take();
                while !w.join.is_finished() && Instant::now() < deadline {
                    thread::sleep(Duration::from_millis(1));
                }
                if w.join.is_finished() {
                    let w = writer.take().unwrap();
                    let _ = w.join.join();
                } else {
                    eprintln!("addon JSON writer join timed out after 2 s");
                }
            }
        }
    }
}
pub(crate) fn close(lua: &Lua) {
    drop(lua.remove_app_data::<Shared>());
}
type Shared = Rc<RefCell<Service>>;
struct Limited(Vec<u8>);
impl Write for Limited {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        if self.0.len().saturating_add(b.len()) > CAP {
            return Err(std::io::Error::other("JSON save exceeds 14 MiB"));
        }
        self.0.extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn write(job: &Job) -> std::result::Result<(), String> {
    let mut out = Limited(Vec::new());
    serde_json::to_writer(&mut out, &job.value).map_err(|e| e.to_string())?;
    let target = job.root.join("files").join(&job.key);
    let deleted = job.root.join("removed").join(&job.key);
    let _guard = super::sandbox::lock_parents(&target, true)?;
    let _deleted = super::sandbox::lock_parents(&deleted, true)?;
    super::file_io::no_links(&target)?;
    super::file_io::no_links(&deleted)?;
    super::sandbox::check_quota(&job.root, &target, out.0.len())?;
    #[cfg(test)]
    if let Some((started, release)) = PAUSE.lock().unwrap().take() {
        let _ = started.send(());
        let _ = release.recv_timeout(Duration::from_secs(5));
    }
    super::storage::atomic_replace_with(&target, &out.0, |from, to| {
        let gate = latest().lock().unwrap();
        if !job.permit.load(Ordering::Acquire)
            || gate.get(&ordered_path(target.clone())) != Some(&job.generation)
        {
            return Err(std::io::Error::other("JSON save superseded or disabled"));
        }
        std::fs::rename(from, to)?;
        if deleted.exists() {
            std::fs::remove_file(&deleted)?;
        }
        Ok(())
    })
    .map_err(|e| e.to_string())?;
    Ok(())
}
#[cfg(test)]
static PAUSE: Mutex<Option<(mpsc::Sender<()>, mpsc::Receiver<()>)>> = Mutex::new(None);
#[cfg(test)]
pub(crate) fn pause_next_commit() -> (mpsc::Receiver<()>, mpsc::Sender<()>) {
    let (started, rx) = mpsc::channel();
    let (tx, release) = mpsc::channel();
    *PAUSE.lock().unwrap() = Some((started, release));
    (rx, tx)
}
fn service(lua: &Lua) -> Result<Shared> {
    if let Some(s) = lua.app_data_ref::<Shared>() {
        return Ok(s.clone());
    }
    let work: Work = Arc::new((Mutex::new(Queue::default()), Condvar::new()));
    let mut global = WRITER.lock().unwrap();
    if global.as_ref().is_some_and(|w| w.tx.is_none()) {
        if global.as_ref().unwrap().join.is_finished() {
            let _ = global.take().unwrap().join.join();
        } else {
            return Err(mlua::Error::runtime("JSON writer is still shutting down"));
        }
    }
    if global.is_none() {
        let (tx, rx) = mpsc::channel::<Work>();
        let join = thread::Builder::new()
            .name("addon-json-save".into())
            .spawn(move || {
                while let Ok(worker) = rx.recv() {
                    loop {
                        let job = {
                            let mut q = worker.0.lock().unwrap();
                            let Some(job) = q.pending.take() else {
                                q.scheduled = false;
                                break;
                            };
                            q.active = Some(job.key.clone());
                            job
                        };
                        let result = write(&job);
                        {
                            let path = ordered_path(job.root.join("files").join(&job.key));
                            let mut gate = latest().lock().unwrap();
                            if gate.get(&path) == Some(&job.generation) {
                                gate.remove(&path);
                            }
                        }
                        let mut q = worker.0.lock().unwrap();
                        q.active = None;
                        q.done = Some(Done {
                            id: job.id,
                            key: job.key.clone(),
                            result,
                            worker: thread::current().id(),
                        });
                        worker.1.notify_all();
                    }
                }
            })
            .map_err(mlua::Error::external)?;
        *global = Some(Writer {
            tx: Some(tx),
            join,
            users: 0,
        });
    }
    global.as_mut().unwrap().users += 1;
    drop(global);
    let s = Rc::new(RefCell::new(Service {
        work,
        permit: Arc::new(AtomicBool::new(true)),
        next: 0,
        last_worker: None,
    }));
    lua.set_app_data(s.clone());
    Ok(s)
}
fn snapshot(value: Value) -> std::result::Result<Json, String> {
    fn visit(
        value: Value,
        depth: usize,
        nodes: &mut usize,
        bytes: &mut usize,
        parents: &mut Vec<usize>,
    ) -> std::result::Result<Json, String> {
        if depth > 32 || *nodes == 0 {
            return Err("JSON depth/node quota exceeded".into());
        }
        *nodes -= 1;
        fn charge(bytes: &mut usize, n: usize) -> std::result::Result<(), String> {
            *bytes = bytes
                .checked_sub(n)
                .ok_or("JSON snapshot exceeds 14 MiB of string data")?;
            Ok(())
        }
        Ok(match value {
            Value::Nil => Json::Null,
            Value::Boolean(v) => Json::Bool(v),
            Value::Integer(v) => Json::from(v),
            Value::Number(v) if v.is_finite() => Json::from(v),
            Value::String(v) => {
                let s = v.to_str().map_err(|e| e.to_string())?;
                charge(bytes, s.len())?;
                Json::String(s.to_string())
            }
            Value::UserData(v) if super::async_json::is_null(&v) => Json::Null,
            Value::Table(t) => {
                let array = t
                    .metatable()
                    .is_some_and(|mt| mt.raw_get::<bool>("__json_array").unwrap_or(false));
                if t.metatable().is_some() && !array {
                    return Err("JSON requires plain tables".into());
                }
                let id = t.to_pointer() as usize;
                if parents.contains(&id) {
                    return Err("JSON cannot contain cycles".into());
                }
                parents.push(id);
                let mut object = Map::new();
                let mut numeric = std::collections::BTreeMap::new();
                for pair in t.pairs::<Value, Value>() {
                    let (k, v) = pair.map_err(|e| e.to_string())?;
                    let v = visit(v, depth + 1, nodes, bytes, parents)?;
                    match k {
                        Value::String(k) => {
                            let k = k.to_str().map_err(|e| e.to_string())?;
                            charge(bytes, k.len())?;
                            object.insert(k.to_string(), v);
                        }
                        Value::Integer(k) if k > 0 => {
                            numeric.insert(k as u64, v);
                        }
                        Value::Number(k)
                            if k.is_finite()
                                && k > 0.0
                                && k.fract() == 0.0
                                && k < (u64::MAX as f64) =>
                        {
                            numeric.insert(k as u64, v);
                        }
                        _ => {
                            return Err(
                                "JSON keys must be strings or dense positive indexes".into()
                            );
                        }
                    }
                }
                parents.pop();
                if !numeric.is_empty() || array {
                    if !object.is_empty()
                        || numeric.keys().enumerate().any(|(i, k)| *k != i as u64 + 1)
                    {
                        return Err("JSON arrays must be dense and cannot mix string keys".into());
                    }
                    Json::Array(numeric.into_values().collect())
                } else {
                    Json::Object(object)
                }
            }
            _ => {
                return Err("JSON requires finite plain data, without functions or handles".into());
            }
        })
    }
    let v = visit(value, 0, &mut 2_000_000, &mut CAP, &mut Vec::new())?;
    if !v.is_object() && !v.is_array() {
        return Err("JSON root must be an object or array".into());
    }
    Ok(v)
}
pub(crate) fn poll(lua: &Lua) -> Result<()> {
    let Some(s) = lua.app_data_ref::<Shared>().map(|s| s.clone()) else {
        return Ok(());
    };
    let done = s.borrow().work.0.lock().unwrap().done.take();
    if let Some(done) = done {
        s.borrow_mut().last_worker = Some(done.worker);
        if done.result.is_ok() {
            if let Some(mounts) = lua.app_data_ref::<Mounts>() {
                super::exchange::saved(&mounts, &done.key);
            }
        }
        super::native_events::json_saved(lua, done.id, done.result.err().as_deref())?;
    }
    Ok(())
}
pub(crate) fn install(host: &Host) -> Result<()> {
    bind(
        &host.lua,
        "files",
        "save_json_async",
        host.lua
            .create_function(|lua, (path, value): (String, Value)| {
                let Some(m) = lua.app_data_ref::<Mounts>().map(|m| m.clone()) else {
                    return error(
                        lua,
                        "eqoa.files.save_json_async",
                        "unavailable",
                        "file mounts unavailable",
                    );
                };
                // Only relative private names; resource aliases must never change packaged data.
                if path.starts_with(['/', '\\'])
                    || path.contains(':')
                    || path.starts_with("resources/")
                    || path.starts_with("package_resources/")
                {
                    return error(
                        lua,
                        "eqoa.files.save_json_async",
                        "permission_denied",
                        "save requires a relative private path",
                    );
                }
                let key = match m.key(&path) {
                    Ok(k) if k.starts_with("data") && !super::sandbox::import_key(&k) => ordered_path(k),
                    _ => {
                        return error(
                            lua,
                            "eqoa.files.save_json_async",
                            "permission_denied",
                            "invalid private path",
                        );
                    }
                };
                let Some(root) = m.data else {
                    return error(
                        lua,
                        "eqoa.files.save_json_async",
                        "unavailable",
                        "private storage unavailable",
                    );
                };

                let s = service(lua)?;
                {
                    let s = s.borrow();
                    let q = s.work.0.lock().unwrap();
                    if q.closed
                        || q.active.as_ref().is_some_and(|p| *p != key)
                        || q.pending.as_ref().is_some_and(|j| j.key != key)
                    {
                        return error(
                            lua,
                            "eqoa.files.save_json_async",
                            "quota",
                            "one JSON save path at a time",
                        );
                    }
                }
                let value = match snapshot(value) {
                    Ok(v) => v,
                    Err(e) => {
                        return error(lua, "eqoa.files.save_json_async", "invalid_argument", &e);
                    }
                };
                let mut s = s.borrow_mut();
                s.next += 1;
                let id = s.next;
                let generation = NEXT.fetch_add(1, Ordering::Relaxed);
                let target = ordered_path(root.join("files").join(&key));
                let mut gate = latest().lock().unwrap();
                let notify = {
                    let mut q = s.work.0.lock().unwrap();
                    if let Err(e) = q.push(Job {
                        id,
                        key,
                        root,
                        value,
                        generation,
                        permit: s.permit.clone(),
                    }) {
                        return error(lua, "eqoa.files.save_json_async", "quota", &e);
                    }
                    let notify = !q.scheduled;
                    q.scheduled = true;
                    notify
                };
                gate.insert(target, generation);
                drop(gate);
                if notify {
                    WRITER
                        .lock()
                        .unwrap()
                        .as_ref()
                        .unwrap()
                        .tx
                        .as_ref()
                        .unwrap()
                        .send(s.work.clone())
                        .map_err(mlua::Error::external)?;
                }
                lua.pack_multi((id, Value::Nil))
            })?,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round4_save_queue_coalesces_and_rejects_another_path() {
        let mut q = Queue::default();
        let job = |id, key: &str| Job {
            id,
            key: key.into(),
            root: PathBuf::new(),
            value: Json::Object(Map::new()),
            generation: id,
            permit: Arc::new(AtomicBool::new(true)),
        };
        q.push(job(1, "data/journal")).unwrap();
        q.push(job(2, "data/journal")).unwrap();
        assert_eq!(q.pending.as_ref().unwrap().id, 2);
        assert!(q.push(job(3, "data/other")).is_err());
        assert_eq!(q.pending.as_ref().unwrap().id, 2);
    }
    #[test]
    fn round4_json_encoder_caps_output_and_preserves_arrays_null() {
        let lua = Lua::new();
        let h = Host::new().unwrap();
        let value = h
            .lua
            .load("return {pos={1,2,3},empty={},null=eqoa.files.null}")
            .eval()
            .unwrap();
        let value = snapshot(value).unwrap();
        assert_eq!(value["pos"], serde_json::json!([1, 2, 3]));
        assert!(value["empty"].is_object());
        assert!(value["null"].is_null());
        assert!(snapshot(lua.load("return {v=0/0}").eval().unwrap()).is_err());
        let mut out = Limited(Vec::new());
        out.write_all(&vec![b' '; CAP]).unwrap();
        assert!(out.write_all(b"x").is_err());
        assert_eq!(out.0.len(), CAP);
    }
}

#[cfg(test)]
pub(crate) fn worked_off_thread(lua: &Lua) -> bool {
    lua.app_data_ref::<Shared>().is_some_and(|s| {
        s.borrow()
            .last_worker
            .is_some_and(|id| id != thread::current().id())
    })
}
