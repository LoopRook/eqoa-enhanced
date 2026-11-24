// Code in this file is LoopRook's addition for EQOA Enhanced and is under the GNU Affero General Public License v3 or later (see LICENSE).
// Included in manager::tests: the manager's own hand-off of a mod's settings to the background writer (needs a GPU,
// like the other manager tests: without one these return early, as `forge_manager_*` do).

// The mod saves on its second frame (its first frame ends with the manager's load of the saved settings).
const SAVES_THEN_HOOKS: &str = r#"
    frames = (frames or 0) + 1
    if not hooked then hooked = true; eqoa.settings.on_save(function() return {v=2} end) end
    if frames == 2 then assert(eqoa.settings.save({v=1})) end
"#;
const SAVES_V5: &str = "frames = (frames or 0) + 1; if frames == 2 then assert(eqoa.settings.save({v=5})) end";

fn encoded(lua_source: &str) -> serde_json::Value {
    super::super::profile::encode(Lua::new().load(lua_source).eval().unwrap()).unwrap()
}

fn settings_file(config: &Path, addon: &super::super::discovery::DiscoveredAddon) -> Option<serde_json::Value> {
    let storage = Storage::new(Some(config.to_path_buf()));
    let key = Storage::key(addon).unwrap();
    fs::read(storage.profile_path(&key).unwrap())
        .ok()
        .map(|b| serde_json::from_slice(&b).unwrap())
}

/// A mod saves its settings in a frame: the frame does not wait for the disk (the worker sleeps 700 ms like a slow
/// sync), and disabling the mod waits for that write and then writes its final value after it, so the older bytes
/// cannot land on top. Control: before the disable the write is still pending and the file is not there.
#[test]
fn settings_manager_frame_does_not_wait_and_disable_flushes_in_order() {
    let Some(mut gpu) = Gpu::new() else {
        return;
    };
    let scripts = Scratch::new();
    let config = Scratch::new();
    scripts.package("a", true, SAVES_THEN_HOOKS);
    let addon = super::super::discovery::discover(&scripts.0).addons.remove(0);
    let mut manager = gpu.manager(&scripts.0, Some(config.0.clone()));
    gpu.frame(&mut manager);
    manager.writes.set_delay_for_test(std::time::Duration::from_millis(700));
    let start = std::time::Instant::now();
    gpu.frame(&mut manager);
    assert!(
        start.elapsed() < std::time::Duration::from_millis(600),
        "the frame waited for the disk: {:?}",
        start.elapsed()
    );
    assert!(manager.writes.is_busy(), "control: the write is still on its way");
    assert_eq!(settings_file(&config.0, &addon), None, "control: not on disk yet");
    manager.queue(Command::Enable("a".into(), false));
    gpu.frame(&mut manager);
    assert!(!manager.writes.is_busy(), "disable waited for the pending write");
    assert_eq!(
        settings_file(&config.0, &addon),
        Some(encoded("return {v=2}")),
        "the final value written at disable is the one that stays"
    );
    assert!(manager.diagnostics().iter().all(|d| d.phase != "storage"), "{:?}", manager.diagnostics());
}

/// A background write that fails is a manager diagnostic for that mod. Control: with a writable folder, none.
#[test]
fn settings_manager_failed_background_write_is_a_diagnostic_for_the_mod() {
    let Some(mut gpu) = Gpu::new() else {
        return;
    };
    let scripts = Scratch::new();
    scripts.package("a", true, "frames = (frames or 0) + 1; if frames == 2 then assert(eqoa.settings.save({v=1})) end");
    let good = Scratch::new();
    let mut manager = gpu.manager(&scripts.0, Some(good.0.clone()));
    gpu.frame(&mut manager);
    gpu.frame(&mut manager);
    manager.writes.flush();
    gpu.frame(&mut manager);
    assert!(manager.diagnostics().iter().all(|d| d.phase != "storage"), "control: {:?}", manager.diagnostics());
    drop(manager);

    let bad = Scratch::new();
    fs::write(bad.0.join("addons"), b"not a folder").unwrap();
    let mut manager = gpu.manager(&scripts.0, Some(bad.0.clone()));
    gpu.frame(&mut manager);
    gpu.frame(&mut manager);
    manager.writes.flush();
    gpu.frame(&mut manager);
    let found = manager
        .diagnostics()
        .iter()
        .find(|d| d.phase == "storage" && d.message.contains("Settings were not saved to disk"));
    assert_eq!(found.and_then(|d| d.id.as_deref()), Some("a"), "{:?}", manager.diagnostics());
}

/// Quitting waits for a write still pending (the manager's shutdown, and so its drop, flushes).
#[test]
fn settings_manager_shutdown_waits_for_pending_writes() {
    let Some(mut gpu) = Gpu::new() else {
        return;
    };
    let scripts = Scratch::new();
    let config = Scratch::new();
    scripts.package("a", true, SAVES_V5);
    let addon = super::super::discovery::discover(&scripts.0).addons.remove(0);
    let mut manager = gpu.manager(&scripts.0, Some(config.0.clone()));
    gpu.frame(&mut manager);
    manager.writes.set_delay_for_test(std::time::Duration::from_millis(500));
    gpu.frame(&mut manager);
    assert_eq!(settings_file(&config.0, &addon), None, "control: pending");
    manager.shutdown();
    assert!(!manager.writes.is_busy());
    assert_eq!(settings_file(&config.0, &addon), Some(encoded("return {v=5}")));
}
