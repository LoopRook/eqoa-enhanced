// Code in this file is LoopRook's addition for EQOA Enhanced and is under the GNU Affero General Public License v3 or later (see LICENSE).
//! A mod's `eqoa.settings.save` no longer waits for the disk (owner log 2026-11-24: a freeze of 1.3 to 1.8 s each
//! time the chat window was moved, the atomic replace's `sync_all` running on the frame thread).
//!
//! These run on a bare `Host` with the real writers the manager installs (`native_settings::queued_*`), so they
//! need no GPU. The queue's worker sleeps (`set_delay_for_test`) the way a slow disk does.
use super::{
    host::Host,
    named_profiles::{Document, Store},
    native_settings::{self, Writer},
    storage::Storage,
    write_queue::WriteQueue,
};
use std::{
    fs,
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

struct Dir(PathBuf);
impl Dir {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "eqoa-settings-write-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Dir {
    fn drop(&mut self) {
        assert!(self.0.starts_with(std::env::temp_dir()));
        let _ = fs::remove_dir_all(&self.0);
    }
}

const DELAY: Duration = Duration::from_millis(600);
/// A save that returns inside this did not wait for the 600 ms the disk takes.
const QUICK: Duration = Duration::from_millis(300);

fn slow_queue() -> Arc<WriteQueue> {
    let queue = Arc::new(WriteQueue::new());
    queue.set_delay_for_test(DELAY);
    queue
}

/// What the profile encoder makes of a Lua value: the form the files hold.
fn encoded(lua_source: &str) -> serde_json::Value {
    let lua = mlua::Lua::new();
    super::profile::encode(lua.load(lua_source).eval().unwrap()).unwrap()
}

fn host_with(writer: Writer) -> Host {
    let host = Host::new().unwrap();
    native_settings::configure(&host, None, writer);
    host
}

fn save(host: &Host, lua_table: &str) -> Duration {
    let start = Instant::now();
    host.execute(&format!("assert(eqoa.settings.save({lua_table}))")).unwrap();
    start.elapsed()
}

fn on_disk(storage: &Storage, key: &str) -> Option<serde_json::Value> {
    fs::read(storage.profile_path(key).unwrap())
        .ok()
        .map(|b| serde_json::from_slice(&b).unwrap())
}

/// The frame does not wait for the disk; the value is the mod's at once; the file has the value after the flush.
/// Negative control: a writer that waits for the disk (how the code behaved before) takes the whole 600 ms, so the
/// timing check does tell the two apart.
#[test]
fn settings_save_returns_before_a_slow_disk_finishes_and_the_file_holds_the_value_after_the_flush() {
    let dir = Dir::new();
    let storage = Storage::new(Some(dir.0.clone()));
    let key = Storage::key_for_addon_id("mod_a");
    let queue = slow_queue();
    let host = host_with(native_settings::queued_writer(
        storage.clone(),
        queue.clone(),
        key.clone(),
        "mod_a".into(),
    ));
    let took = save(&host, "{n=1, name='chat'}");
    assert!(took < QUICK, "save waited for the disk: {took:?}");
    // the mod's own view has the value already: what it would save at disable and what a load would hand back
    let value = native_settings::slot(&host.lua).borrow().committed.clone();
    assert_eq!(value, Some(encoded("return {n=1, name='chat'}")));
    assert!(native_settings::callback(&host.lua, 2, &mlua::Value::Nil).is_some_and(|r| r.is_ok()));
    // the disk is still on its way
    assert_eq!(on_disk(&storage, &key), None, "the write has not landed yet");
    queue.flush();
    assert_eq!(on_disk(&storage, &key), value, "the file holds the value after the flush");
    assert!(queue.take_failures().is_empty());

    // control: wait for the disk after each save, as the frame thread used to
    let waiting: Writer = {
        let inner = native_settings::queued_writer(storage.clone(), queue.clone(), key.clone(), "mod_a".into());
        let queue = queue.clone();
        Rc::new(move |v| {
            let r = inner(v);
            queue.flush();
            r
        })
    };
    let host = host_with(waiting);
    let took = save(&host, "{n=2}");
    assert!(took >= DELAY, "control: a writer that waits for the disk takes its time: {took:?}");
    assert_eq!(on_disk(&storage, &key), Some(encoded("return {n=2}")));
}

/// Several saves in a row: the file ends with the last value (and, as the queue test shows, the saves that had not
/// started are replaced by the newest, so the disk is not asked for all of them).
#[test]
fn settings_saves_in_a_row_leave_the_last_value_on_disk() {
    let dir = Dir::new();
    let storage = Storage::new(Some(dir.0.clone()));
    let key = Storage::key_for_addon_id("mod_a");
    let queue = slow_queue();
    let host = host_with(native_settings::queued_writer(storage.clone(), queue.clone(), key.clone(), "mod_a".into()));
    let start = Instant::now();
    for n in 1..=6 {
        save(&host, &format!("{{n={n}}}"));
    }
    assert!(start.elapsed() < QUICK, "six saves did not wait for the disk: {:?}", start.elapsed());
    queue.flush();
    // 6 saves, the first already being written, the rest collapsed: two disk writes at most
    assert!(start.elapsed() < DELAY * 3, "the saves in between were not written one by one: {:?}", start.elapsed());
    assert_eq!(on_disk(&storage, &key), Some(encoded("return {n=6}")), "last value wins");
}

/// The refusals stay at the call: a value over the quota is refused at once, nothing is queued.
#[test]
fn settings_save_still_refuses_an_over_quota_value_at_the_call() {
    let dir = Dir::new();
    let storage = Storage::new(Some(dir.0.clone()));
    let key = Storage::key_for_addon_id("mod_a");
    let queue = slow_queue();
    let host = host_with(native_settings::queued_writer(storage.clone(), queue.clone(), key.clone(), "mod_a".into()));
    host.execute(
        "local big=string.rep('x',1024*1024+16);local ok,e=eqoa.settings.save({big=big});assert(ok==nil and e.code=='quota',tostring(e and e.code))",
    )
    .unwrap();
    assert!(!queue.is_busy(), "a refused save queues nothing");
    queue.flush();
    assert_eq!(on_disk(&storage, &key), None);
}

/// A write that fails on the worker is reported for its mod. Control: with a writable folder nothing is reported.
/// `save` still answered true (the value was accepted).
#[test]
fn settings_a_failing_background_write_is_reported_for_its_mod() {
    let dir = Dir::new();
    let queue = Arc::new(WriteQueue::new());
    let key = Storage::key_for_addon_id("mod_a");
    // control
    let good = Storage::new(Some(dir.0.join("good")));
    let host = host_with(native_settings::queued_writer(good.clone(), queue.clone(), key.clone(), "mod_a".into()));
    save(&host, "{n=1}");
    queue.flush();
    assert!(queue.take_failures().is_empty(), "control: a writable folder reports nothing");
    assert_eq!(on_disk(&good, &key), Some(encoded("return {n=1}")));
    // the addons folder is a file, so the settings file cannot be made
    fs::create_dir_all(dir.0.join("bad")).unwrap();
    fs::write(dir.0.join("bad").join("addons"), b"not a folder").unwrap();
    let bad = Storage::new(Some(dir.0.join("bad")));
    let host = host_with(native_settings::queued_writer(bad, queue.clone(), key, "mod_a".into()));
    save(&host, "{n=2}");
    queue.flush();
    let failures = queue.take_failures();
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert_eq!(failures[0].owner.as_deref(), Some("mod_a"));
    assert!(!failures[0].message.is_empty());
}

/// Under named profiles: the same, through the profile file. Read-only profiles refuse at the call; a profile
/// file damaged on disk is still never overwritten (the check runs on the worker and the failure is reported).
#[test]
fn settings_under_named_profiles_do_not_wait_for_the_disk_and_keep_their_refusals() {
    let dir = Dir::new();
    let store = Store::new(Some(dir.0.clone()));
    let key = Storage::key_for_addon_id("mod_a");
    let queue = slow_queue();
    let writer = |writable: bool| {
        native_settings::queued_profile_writer(
            store.clone(),
            queue.clone(),
            Document::empty(),
            "Default".into(),
            writable,
            key.clone(),
            "mod_a".into(),
        )
    };
    // read-only: refused at the call
    let host = host_with(writer(false));
    host.execute("local ok,e=eqoa.settings.save({n=0});assert(ok==nil and e.code=='permission_denied',tostring(e and e.code))")
        .unwrap();
    assert!(!queue.is_busy());
    // writable: quick, then on disk after the flush
    let host = host_with(writer(true));
    let took = save(&host, "{n=3}");
    assert!(took < QUICK, "save waited for the profile file: {took:?}");
    assert!(store.read("Default").is_err(), "the write has not landed yet");
    queue.flush();
    assert_eq!(store.read("Default").unwrap().addons.get(&key), Some(&encoded("return {n=3}")));
    // damaged on disk afterwards: the next write is refused on the worker, the damaged file stays, the mod hears of it
    let path = store.path("Default").unwrap();
    fs::write(&path, b"{ damaged").unwrap();
    let took = save(&host, "{n=4}");
    assert!(took < QUICK);
    queue.flush();
    assert_eq!(fs::read(&path).unwrap(), b"{ damaged", "a damaged profile is never overwritten");
    let failures = queue.take_failures();
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert_eq!(failures[0].owner.as_deref(), Some("mod_a"));
}
