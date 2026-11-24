// Code in this file is LoopRook's addition for EQOA Enhanced and is under the GNU Affero General Public License v3 or later (see LICENSE).
//! Performance round (core): a mod's file writes (`eqoa.files.write` / `remove` / `open("w")`) and the option
//! toggle's write no longer wait for the disk's sync on the frame. The new file is in place at once (so every
//! read sees it); only the sync runs on the background thread. The queue's worker sleeps
//! (`deferred_sync::set_delay_for_test`) the way a slow disk does.
use super::{deferred_sync, discovery::DiscoveredAddon, file_io, host::Host, storage::Storage};
use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

struct Dir(PathBuf);
impl Drop for Dir {
    fn drop(&mut self) {
        assert!(self.0.starts_with(std::env::temp_dir()));
        let _ = fs::remove_dir_all(&self.0);
    }
}

const DELAY: Duration = Duration::from_millis(600);
const QUICK: Duration = Duration::from_millis(300);

fn fixture() -> (Dir, Host) {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "eqoa-deferred-sync-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    fs::create_dir_all(root.join("scripts/pack")).unwrap();
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
        requested_capabilities: vec![],
        extensions: Default::default(),
        initially_enabled: false,
        mods_root: Some(scripts.clone()),
    };
    let host = Host::new().unwrap();
    file_io::install(&host, &addon, Some(root.join("config"))).unwrap();
    (Dir(root), host)
}

fn timed(host: &Host, lua: &str) -> Duration {
    let start = Instant::now();
    host.execute(lua).unwrap();
    start.elapsed()
}

/// A write returns before a slow sync ends, a read right after sees the bytes (read, list), a remove is seen
/// at once, and the flush waits for the sync. (Control, run by hand: flushing inside `atomic_replace_deferred`,
/// the old behaviour, makes the first timing assertion fail.)
#[test]
fn files_write_does_not_wait_for_the_sync_and_reads_see_it_at_once() {
    let (_dir, host) = fixture();
    deferred_sync::set_delay_for_test(DELAY);
    let wrote = timed(&host, "assert(eqoa.files.write('a.txt','one'))");
    assert!(wrote < QUICK, "write waited for the sync: {wrote:?}");
    assert!(deferred_sync::is_busy(), "the sync is still waiting");
    host.execute(
        r#"
        assert(eqoa.files.read('a.txt') == 'one')
        local found = false
        for _, e in ipairs(eqoa.files.list('')) do if e.path == 'a.txt' and e.size == 3 then found = true end end
        assert(found, 'list sees it')
        "#,
    )
    .unwrap();
    // a second write replaces the first before the sync ran: the last bytes are the ones read
    assert!(timed(&host, "assert(eqoa.files.write('a.txt','two!'))") < QUICK);
    host.execute("assert(eqoa.files.read('a.txt') == 'two!')").unwrap();
    // a remove is visible at once, and the file is gone from the list
    assert!(timed(&host, "assert(eqoa.files.remove('a.txt') == true)") < QUICK);
    host.execute(
        r#"
        local ok, err = eqoa.files.read('a.txt')
        assert(ok == nil and err.code == 'not_found', err and err.code)
        for _, e in ipairs(eqoa.files.list('')) do assert(e.path ~= 'a.txt', 'list still shows it') end
        assert(eqoa.files.remove('a.txt') == false)
        "#,
    )
    .unwrap();
    let start = Instant::now();
    deferred_sync::flush();
    assert!(start.elapsed() >= Duration::from_millis(200), "flush waited for the queued syncs: {:?}", start.elapsed());
    assert!(!deferred_sync::is_busy());
    assert!(deferred_sync::take_failures().is_empty());
}

/// `io.open(.., "w")`: `write`, `flush` and `close` of a handle are quick and the bytes are in place at once.
#[test]
fn an_open_w_handle_closes_quickly_and_its_bytes_are_in_place() {
    let (dir, host) = fixture();
    fs::create_dir_all(dir.0.join("scripts/pack/resources")).unwrap();
    let resources = dir.0.join("scripts/pack/resources").canonicalize().unwrap();
    let shown = resources.to_string_lossy().replace("\\\\?\\", "").replace(std::path::MAIN_SEPARATOR, "/");
    host.lua.globals().set("resource", shown).unwrap();
    deferred_sync::set_delay_for_test(DELAY);
    let target = dir.0.join("config/files/pack/resources/h.cfg");
    let spent = timed(&host, r"f = assert(io.open(resource..'/h.cfg','w')); f:write('one;'); assert(f:flush())");
    assert_eq!(fs::read(&target).unwrap(), b"one;", "flush: in place at once");
    let spent2 = timed(&host, r"f:write('two;'); assert(f:close())");
    assert_eq!(fs::read(&target).unwrap(), b"one;two;", "close: in place at once");
    assert!(spent < QUICK && spent2 < QUICK, "open/flush/close waited for the sync: {spent:?} {spent2:?}");
    assert!(deferred_sync::is_busy());
    deferred_sync::flush();
}

/// The refusals still come back at the call: a path out of the sandbox, the imports folder and a file over
/// 1 MiB. Nothing is queued for them.
#[test]
fn sandbox_and_size_errors_are_still_returned_at_the_call() {
    let (_dir, host) = fixture();
    deferred_sync::set_delay_for_test(DELAY);
    host.execute(
        r#"
        local function refused(path, bytes) local ok, r = pcall(eqoa.files.write, path, bytes); return (not ok) or r == nil end
        assert(refused('../outside.txt', 'x'), 'outside the sandbox')
        assert(refused('imports/x.json', 'x'), 'imports are read-only')
        assert(refused('big.bin', string.rep('x', 1024 * 1024 + 1)), 'over 1 MiB')
        "#,
    )
    .unwrap();
    assert!(!deferred_sync::is_busy(), "a refused write queues nothing");
    deferred_sync::flush();
}

/// The option toggle's file write: the file is in place at once (the read-modify-write of the next toggle
/// sees it), the call is quick, and its refusals are unchanged.
#[test]
fn an_option_toggle_write_is_quick_and_the_file_is_in_place_for_the_next_read() {
    let dir = std::env::temp_dir().join(format!("eqoa-deferred-toggle-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let _guard = Dir(dir.clone());
    deferred_sync::set_delay_for_test(DELAY);
    let storage = Storage::new(Some(dir.clone()));
    let key = Storage::key_for_addon_id("toggle_mod");
    let value = serde_json::json!({"table": [["opt", true]]});
    let start = Instant::now();
    storage.save_encoded_deferred_sync(&key, &value).unwrap();
    assert!(start.elapsed() < QUICK, "the toggle write waited: {:?}", start.elapsed());
    let on_disk: serde_json::Value = serde_json::from_slice(&fs::read(storage.profile_path(&key).unwrap()).unwrap()).unwrap();
    assert_eq!(on_disk, value, "in place at once");
    assert!(deferred_sync::is_busy());
    assert!(storage.save_encoded_deferred_sync("../escape", &value).is_err(), "a bad key is refused at the call");
    deferred_sync::flush();
    assert!(!deferred_sync::is_busy());
}
