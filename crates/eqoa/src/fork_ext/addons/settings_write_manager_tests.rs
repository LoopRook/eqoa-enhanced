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

fn find_file(dir: &Path, name: &str) -> Option<Vec<u8>> {
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_file(&path, name) {
                return Some(found);
            }
        } else if path.file_name().is_some_and(|n| n == name) {
            return fs::read(path).ok();
        }
    }
    None
}

/// Disabling a mod waits for what its `DisableScript` writes and for the files it left open (closed by the
/// manager after the first flush): both are on the disk when the disable returns, with the disk's sync slow.
/// (Control, the old order: the flush ran before them, so the syncs were still pending when disable returned;
/// run by hand by removing the flush at the end of `disable`.)
#[test]
fn disable_waits_for_the_disable_scripts_writes_and_the_files_it_left_open() {
    let Some(mut gpu) = Gpu::new() else {
        return;
    };
    let scripts = Scratch::new();
    let config = Scratch::new();
    scripts.package(
        "a",
        true,
        r#"
        if not hooked then
            hooked = true
            UiForge.RegisterCallback(UiForge.CallbackType.DisableScript, function() assert(eqoa.files.write('d.txt', 'from disable')) end)
        end
        "#,
    );
    let mut manager = gpu.manager(&scripts.0, Some(config.0.clone()));
    gpu.frame(&mut manager);
    gpu.frame(&mut manager);
    super::super::deferred_sync::set_delay_for_test(std::time::Duration::from_millis(700));
    manager.queue(Command::Enable("a".into(), false));
    gpu.frame(&mut manager);
    assert!(
        !super::super::deferred_sync::is_busy(),
        "disable returned with a file's sync still pending"
    );
    assert_eq!(find_file(&config.0, "d.txt").as_deref(), Some(&b"from disable"[..]), "{:?}", manager.diagnostics());
    super::super::deferred_sync::set_delay_for_test(std::time::Duration::ZERO);
}

/// A reload closes the files the mod left open (after the manager's first flush): their bytes are on the disk when
/// the reload returns. (Control, the old order: the sync was still pending then.)
#[test]
fn reload_waits_for_the_files_the_mod_left_open() {
    let Some(mut gpu) = Gpu::new() else {
        return;
    };
    let scripts = Scratch::new();
    let config = Scratch::new();
    scripts.package(
        "a",
        true,
        r#"
        -- the first instance leaves a file open; the reloaded one finds it written and leaves it be
        if not hooked then
            hooked = true
            if eqoa.files.read('data/h.txt') == nil then
                h = assert(io.open('data/h.txt', 'w'))
                h:write('left open')
            end
        end
        "#,
    );
    let mut manager = gpu.manager(&scripts.0, Some(config.0.clone()));
    gpu.frame(&mut manager);
    gpu.frame(&mut manager);
    super::super::deferred_sync::set_delay_for_test(std::time::Duration::from_millis(700));
    manager.queue(Command::Reload("a".into()));
    gpu.frame(&mut manager);
    assert!(!super::super::deferred_sync::is_busy(), "reload returned with a file's sync still pending");
    assert_eq!(find_file(&config.0, "h.txt").as_deref(), Some(&b"left open"[..]), "{:?}", manager.diagnostics());
    super::super::deferred_sync::set_delay_for_test(std::time::Duration::ZERO);
}

/// Regression test (PC interface perf): steady `Manager::frame` ticks in PC mode must not re-run
/// `bindpage::resolve`, while an interface switch (PC <-> Controller) or disable/re-enable runs it once.
#[test]
fn perf_steady_frames_do_not_re_resolve_binds() {
    let Some(mut gpu) = Gpu::new() else {
        eprintln!("SKIP perf_steady_frames_do_not_re_resolve_binds: no GPU adapter");
        return;
    };
    crate::settings::testing::reset(false);
    crate::settings::set_ui_mode(true).unwrap();
    crate::fork_ext::bindpage::reset_in_memory();

    let scripts = Scratch::new();
    let config = Scratch::new();
    scripts.package(
        "mod_bind",
        true,
        r#"
        if not init then
            init = true
            assert(eqoa.input.bind({id = "seven", label = "Seven", default_key = "7"}, function() end))
        end
        "#,
    );
    scripts.package("mod_plain", true, "local x = 1");

    let mut manager = gpu.manager(&scripts.0, Some(config.0.clone()));
    manager.enable_profiles(Some(config.0.clone()));
    // Warmup: frame 1 compiles & initializes; frame 2 settles steady state.
    gpu.frame(&mut manager);
    gpu.frame(&mut manager);

    assert_eq!(
        crate::fork_ext::bindpage::get_key("mod_bind", "seven").as_deref(),
        Some(""),
        "PC mode reserves 7 for Bar 2 slot 2"
    );

    let resolves_before = crate::fork_ext::bindpage::resolve_count_for_test();
    for _ in 0..5 {
        gpu.frame(&mut manager);
    }
    assert_eq!(
        crate::fork_ext::bindpage::resolve_count_for_test(),
        resolves_before,
        "steady PC frames must not re-run bindpage::resolve"
    );

    // Negative control 1: switching PC -> Controller re-resolves once on the next frame.
    crate::settings::set_ui_mode(false).unwrap();
    gpu.frame(&mut manager);
    assert_eq!(
        crate::fork_ext::bindpage::resolve_count_for_test(),
        resolves_before + 1,
        "interface switch must re-resolve mod defaults once"
    );
    assert_eq!(
        crate::fork_ext::bindpage::get_key("mod_bind", "seven").as_deref(),
        Some("7"),
        "7 becomes available in Controller mode"
    );
    for _ in 0..3 {
        gpu.frame(&mut manager);
    }
    assert_eq!(
        crate::fork_ext::bindpage::resolve_count_for_test(),
        resolves_before + 1,
        "steady Controller frames must not re-run bindpage::resolve"
    );

    // Negative control 2: disabling and re-enabling a bound mod runs resolve on each transition.
    manager.queue(Command::Enable("mod_bind".into(), false));
    gpu.frame(&mut manager);
    let after_disable = crate::fork_ext::bindpage::resolve_count_for_test();
    assert!(after_disable > resolves_before + 1, "disable must re-resolve");
    manager.queue(Command::Enable("mod_bind".into(), true));
    gpu.frame(&mut manager);
    let after_enable = crate::fork_ext::bindpage::resolve_count_for_test();
    assert!(after_enable > after_disable, "re-enable must re-resolve");
    for _ in 0..3 {
        gpu.frame(&mut manager);
    }
    assert_eq!(
        crate::fork_ext::bindpage::resolve_count_for_test(),
        after_enable,
        "no further resolves once active again"
    );

    manager.shutdown();
    crate::settings::testing::reset(false);
    crate::fork_ext::bindpage::reset_in_memory();
}
