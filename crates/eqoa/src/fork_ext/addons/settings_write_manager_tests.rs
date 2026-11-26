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

/// Lead review #1: switching profiles (or legacy <-> named) must never reuse an old profile's
/// value or writer, even if two sessions share the same numeric generation (negative control).
#[test]
fn settings_gen_profile_switch_reads_and_writes_target_profile_with_session_identity_control() {
    use super::super::named_profiles::{Character, CharacterSnapshot, Document, Store};
    let Some(mut gpu) = Gpu::new() else {
        eprintln!("SKIP settings_gen_profile_switch: no GPU adapter");
        return;
    };
    let scripts = Scratch::new();
    let config = Scratch::new();
    scripts.package(
        "m",
        true,
        r#"
        local cur = eqoa.settings.load()
        seen = cur and cur.v or "none"
        if save_next ~= nil then
            assert(eqoa.settings.save({v = save_next}))
            save_next = nil
        end
        "#,
    );
    let addon = super::super::discovery::discover(&scripts.0).addons.remove(0);
    let key = Storage::key(&addon).unwrap();
    let store = Store::new(Some(config.0.clone()));
    let mut doc_a = Document::empty();
    doc_a.enabled.insert(key.clone(), true);
    store.create("ProfA", &doc_a).unwrap();
    let mut doc_b = Document::empty();
    doc_b.enabled.insert(key.clone(), true);
    doc_b.addons.insert(key.clone(), encoded("return {v='Y'}"));
    store.create("ProfB", &doc_b).unwrap();

    let hero = CharacterSnapshot::new(true, "Hero", 1).unwrap();
    store
        .bind(&Character::new("srv", hero).unwrap(), "ProfA", false)
        .unwrap();

    let mut manager = gpu.manager(&scripts.0, Some(config.0.clone()));
    // Exercise legacy -> named transition on a live manager instance too.
    gpu.frame(&mut manager);
    assert!(matches!(
        manager.entries[0].instance.as_ref().unwrap().settings_config,
        Some(SettingsConfig::Legacy(..))
    ));

    manager.enable_profiles(Some(config.0.clone()));
    // Negative control: before profile_boundary, even if the instance was retained across
    // legacy -> named (both starting near 0/1), the enum variant differs and forces reconfigure.
    let old_cfg = manager.entries[0].instance.as_ref().unwrap().settings_config.unwrap();
    let new_cfg = manager.current_settings_config();
    assert_ne!(old_cfg, new_cfg, "legacy and named tokens never collide");

    // Enter world on ProfA, save X.
    manager.profile_boundary(&mut gpu.context, Some(hero), "srv");
    assert_eq!(manager.profile_name(), Some("ProfA"));
    gpu.frame(&mut manager);
    manager.entries[0]
        .instance
        .as_ref()
        .unwrap()
        .host
        .lua
        .globals()
        .set("save_next", "X")
        .unwrap();
    gpu.frame(&mut manager);
    manager.writes.flush();
    assert_eq!(
        store.read("ProfA").unwrap().addons.get(&key),
        Some(&encoded("return {v='X'}"))
    );

    // Switch to ProfB (which has Y): mod reads Y at once and saves Z into ProfB only.
    manager.queue_profile(ProfileAction::Load("ProfB".into()));
    manager.profile_boundary(&mut gpu.context, Some(hero), "srv");
    assert_eq!(manager.profile_name(), Some("ProfB"));
    gpu.frame(&mut manager);
    let seen_b: String = manager.entries[0]
        .instance
        .as_ref()
        .unwrap()
        .host
        .lua
        .globals()
        .get("seen")
        .unwrap();
    assert_eq!(seen_b, "Y", "ProfB value Y must be readable at once after switch");

    manager.entries[0]
        .instance
        .as_ref()
        .unwrap()
        .host
        .lua
        .globals()
        .set("save_next", "Z")
        .unwrap();
    gpu.frame(&mut manager);
    manager.writes.flush();
    assert_eq!(
        store.read("ProfB").unwrap().addons.get(&key),
        Some(&encoded("return {v='Z'}")),
        "save under ProfB must land in ProfB"
    );
    assert_eq!(
        store.read("ProfA").unwrap().addons.get(&key),
        Some(&encoded("return {v='X'}")),
        "ProfA must remain untouched with X"
    );

    // Switch back to ProfA: mod reads X at once.
    manager.queue_profile(ProfileAction::Load("ProfA".into()));
    manager.profile_boundary(&mut gpu.context, Some(hero), "srv");
    assert_eq!(manager.profile_name(), Some("ProfA"));
    gpu.frame(&mut manager);
    let seen_a: String = manager.entries[0]
        .instance
        .as_ref()
        .unwrap()
        .host
        .lua
        .globals()
        .get("seen")
        .unwrap();
    assert_eq!(seen_a, "X", "switching back to ProfA must read X at once");

    // Negative control for naive generation-only comparison:
    // Simulate a retained instance whose previous session was at generation=1, and a new
    // NamedSession (pointing at ProfB) also starts at generation=1.
    let gen_a = match manager.entries[0].instance.as_ref().unwrap().settings_config {
        Some(SettingsConfig::Named { session_id, generation }) => (session_id, generation),
        other => panic!("expected Named config, got {other:?}"),
    };
    let mut replacement = store.read("ProfB").unwrap();
    replacement.addons.insert(key.clone(), encoded("return {v='FROM_NEW_SESSION'}"));
    {
        let s = manager.named.as_mut().unwrap();
        s.active = "ProfB".into();
        s.data = replacement;
        s.session_id = gen_a.0 + 1;
        s.generation = gen_a.1; // Deliberately IDENTICAL numeric generation!
    }
    let naive_same_gen = gen_a.1 == manager.named.as_ref().unwrap().generation;
    assert!(naive_same_gen, "negative control: naive generation compare would falsely match");
    assert_ne!(
        manager.entries[0].instance.as_ref().unwrap().settings_config,
        Some(manager.current_settings_config()),
        "SettingsConfig with session_id must NOT match across replaced sessions"
    );
    gpu.frame(&mut manager);
    let seen_replaced: String = manager.entries[0]
        .instance
        .as_ref()
        .unwrap()
        .host
        .lua
        .globals()
        .get("seen")
        .unwrap();
    assert_eq!(
        seen_replaced, "FROM_NEW_SESSION",
        "retained instance must immediately pick up the replaced session's value"
    );

    manager.shutdown();
}

/// Lead review #2: read-your-writes in the same handler run and on the next frame (both named-profile
/// and legacy mode, even while background writes are delayed), plus Command::Save refresh.
#[test]
fn settings_gen_read_your_writes_same_handler_next_frame_named_and_legacy() {
    use super::super::named_profiles::Store;
    let Some(mut gpu) = Gpu::new() else {
        eprintln!("SKIP settings_gen_read_your_writes: no GPU adapter");
        return;
    };
    for named in [true, false] {
        let scripts = Scratch::new();
        let config = Scratch::new();
        scripts.package(
            "m",
            true,
            r#"
            frames = (frames or 0) + 1
            if not hooked then
                hooked = true
                eqoa.settings.on_save(function() return {v = 22} end)
            end
            if frames == 2 then
                assert(eqoa.settings.save({v = 11}))
                local immediate = eqoa.settings.load()
                assert(immediate and immediate.v == 11, "same-handler read-your-writes failed")
                same_handler = immediate.v
            elseif frames == 3 then
                local next_val = eqoa.settings.load()
                assert(next_val and next_val.v == 11, "next-frame read-your-writes failed")
                next_frame = next_val.v
            elseif frames == 4 then
                local after_cmd = eqoa.settings.load()
                after_save_cmd = after_cmd and after_cmd.v or -1
            end
            "#,
        );
        let addon = super::super::discovery::discover(&scripts.0).addons.remove(0);
        let key = Storage::key(&addon).unwrap();
        let mut manager = gpu.manager(&scripts.0, Some(config.0.clone()));
        if named {
            manager.enable_profiles(Some(config.0.clone()));
        }
        gpu.frame(&mut manager); // frame 1: init
        manager
            .writes
            .set_delay_for_test(std::time::Duration::from_millis(350));
        gpu.frame(&mut manager); // frame 2: save {v=11} + same-handler load
        gpu.frame(&mut manager); // frame 3: next-frame load while disk write is still delayed
        {
            let lua = &manager.entries[0].instance.as_ref().unwrap().host.lua;
            assert_eq!(lua.globals().get::<i32>("same_handler").unwrap(), 11, "named={named}");
            assert_eq!(lua.globals().get::<i32>("next_frame").unwrap(), 11, "named={named}");
        }
        manager.writes.flush();
        if named {
            let doc = Store::new(Some(config.0.clone())).read("Default").unwrap();
            assert_eq!(doc.addons.get(&key), Some(&encoded("return {v=11}")));
        } else {
            assert_eq!(settings_file(&config.0, &addon), Some(encoded("return {v=11}")));
        }

        // Command::Save invokes on_save ({v=22}); next frame must see {v=22} (fails if save bump missing).
        manager.queue(Command::Save("m".into()));
        gpu.frame(&mut manager); // frame 4
        let after_cmd: i32 = manager.entries[0]
            .instance
            .as_ref()
            .unwrap()
            .host
            .lua
            .globals()
            .get("after_save_cmd")
            .unwrap();
        assert_eq!(
            after_cmd, 22,
            "named={named}: Command::Save must refresh slot value for next frame"
        );
        assert!(manager.diagnostics().is_empty(), "named={named}: {:?}", manager.diagnostics());
        manager.shutdown();
    }
}

/// Lead review #2: two mods saving in the same frame must both keep their saved entries in the
/// named profile (fails red if the generation bump in `collect_native_settings` is removed).
#[test]
fn settings_gen_two_mods_save_same_frame_keep_both() {
    use super::super::named_profiles::Store;
    let Some(mut gpu) = Gpu::new() else {
        eprintln!("SKIP settings_gen_two_mods_save_same_frame: no GPU adapter");
        return;
    };
    let scripts = Scratch::new();
    let config = Scratch::new();
    scripts.package(
        "mod_a",
        true,
        r#"
        frames = (frames or 0) + 1
        if frames == 3 then assert(eqoa.settings.save({from = "a"})) end
        if frames == 4 then local s = eqoa.settings.load(); got = s and s.from or "nil" end
        "#,
    );
    scripts.package(
        "mod_b",
        true,
        r#"
        frames = (frames or 0) + 1
        if frames == 3 then assert(eqoa.settings.save({from = "b"})) end
        if frames == 4 then local s = eqoa.settings.load(); got = s and s.from or "nil" end
        "#,
    );
    let discovered = super::super::discovery::discover(&scripts.0).addons;
    let key_a = Storage::key(discovered.iter().find(|a| a.id == "mod_a").unwrap()).unwrap();
    let key_b = Storage::key(discovered.iter().find(|a| a.id == "mod_b").unwrap()).unwrap();

    let mut manager = gpu.manager(&scripts.0, Some(config.0.clone()));
    manager.enable_profiles(Some(config.0.clone()));
    gpu.frame(&mut manager); // frame 1: init
    gpu.frame(&mut manager); // frame 2: steady (both mods configured at same generation)
    gpu.frame(&mut manager); // frame 3: BOTH mod_a and mod_b call eqoa.settings.save in the same frame!

    // Wait for the background writes submitted by mod_a and mod_b WITHOUT calling flush_profile.
    // If collect_native_settings did not bump generation after mod_a, mod_b's stale writer would
    // overwrite Default.json without mod_a's entry.
    manager.writes.flush();
    let doc = Store::new(Some(config.0.clone())).read("Default").unwrap();
    assert_eq!(
        doc.addons.get(&key_a),
        Some(&encoded("return {from='a'}")),
        "mod_a's save was clobbered by mod_b's same-frame save"
    );
    assert_eq!(
        doc.addons.get(&key_b),
        Some(&encoded("return {from='b'}")),
        "mod_b's save missing"
    );

    gpu.frame(&mut manager); // frame 4: both mods read back their own values
    for (id, expected) in [("mod_a", "a"), ("mod_b", "b")] {
        let entry = manager.entries.iter().find(|e| e.metadata.id == id).unwrap();
        let got: String = entry
            .instance
            .as_ref()
            .unwrap()
            .host
            .lua
            .globals()
            .get("got")
            .unwrap();
        assert_eq!(got, expected, "{id} lost its setting on next frame");
    }
    manager.shutdown();
}

/// Lead review #2: a save refused by quota must leave the previous saved value readable
/// immediately and on subsequent frames, in both named-profile and legacy mode.
#[test]
fn settings_gen_quota_refusal_keeps_old_value_readable() {
    let Some(mut gpu) = Gpu::new() else {
        eprintln!("SKIP settings_gen_quota_refusal: no GPU adapter");
        return;
    };
    for named in [true, false] {
        let scripts = Scratch::new();
        let config = Scratch::new();
        scripts.package(
            "m",
            true,
            r#"
            frames = (frames or 0) + 1
            if frames == 2 then
                assert(eqoa.settings.save({v = "good"}))
            elseif frames == 3 then
                local ok, err = eqoa.settings.save({big = string.rep("x", 1100000)})
                assert(ok == nil and err and err.code == "quota", "expected quota error")
                local same = eqoa.settings.load()
                same_after_refusal = same and same.v or "lost"
            elseif frames == 4 then
                local next_val = eqoa.settings.load()
                next_after_refusal = next_val and next_val.v or "lost"
            end
            "#,
        );
        let mut manager = gpu.manager(&scripts.0, Some(config.0.clone()));
        if named {
            manager.enable_profiles(Some(config.0.clone()));
        }
        for _ in 0..4 {
            gpu.frame(&mut manager);
        }
        let lua = &manager.entries[0].instance.as_ref().unwrap().host.lua;
        assert_eq!(
            lua.globals().get::<String>("same_after_refusal").unwrap(),
            "good",
            "named={named}: same-frame value lost after quota refusal"
        );
        assert_eq!(
            lua.globals().get::<String>("next_after_refusal").unwrap(),
            "good",
            "named={named}: next-frame value lost after quota refusal"
        );
        manager.shutdown();
    }
}

/// Fix (b) performance regression test: steady frames perform 0 `configure_native_settings` calls,
/// while an actual `eqoa.settings.save` refreshes once and then returns to 0.
#[test]
fn settings_gen_steady_frames_skip_reconfigure() {
    let Some(mut gpu) = Gpu::new() else {
        eprintln!("SKIP settings_gen_steady_frames_skip_reconfigure: no GPU adapter");
        return;
    };
    let scripts = Scratch::new();
    let config = Scratch::new();
    scripts.package(
        "saver",
        true,
        r#"
        frames = (frames or 0) + 1
        if frames == 8 then assert(eqoa.settings.save({v = 99})) end
        "#,
    );
    scripts.package("plain", true, "local x = 1");

    let mut manager = gpu.manager(&scripts.0, Some(config.0.clone()));
    manager.enable_profiles(Some(config.0.clone()));
    gpu.frame(&mut manager); // frame 1: init
    gpu.frame(&mut manager); // frame 2: settled

    let before = manager.settings_configures.get();
    for _ in 0..5 {
        gpu.frame(&mut manager); // frames 3..7: steady
    }
    assert_eq!(
        manager.settings_configures.get(),
        before,
        "steady frames must not re-clone profile data to reconfigure native settings"
    );

    gpu.frame(&mut manager); // frame 8: saver calls eqoa.settings.save -> bumps generation
    gpu.frame(&mut manager); // frame 9: both mods reconfigure to fresh generation
    let after_save = manager.settings_configures.get();
    assert!(
        after_save > before,
        "negative control: an actual save must refresh the profile writer"
    );
    gpu.frame(&mut manager); // frame 10: steady again
    assert_eq!(
        manager.settings_configures.get(),
        after_save,
        "no further reconfigures once caught up"
    );
    manager.shutdown();
}

/// A mod option flipped on the Options page (`options::set_toggle_persisted`, which writes `addons/<key>.json`) reaches
/// a mod's `eqoa.settings.load()` in legacy storage, though the manager no longer re-reads that file every frame. In
/// named-profile mode the mod's settings are the profile's, which that write does not touch: they stay as they were.
/// Negative control: without the generation the option write bumps, the legacy mod keeps the old `nil` (checked by
/// leaving the write out in the same test: no change, no reload).
#[test]
fn option_write_reaches_settings_load_in_legacy_storage_and_leaves_named_profiles_alone() {
    let Some(mut gpu) = Gpu::new() else {
        eprintln!("SKIP option_write_reaches_settings_load: no GPU adapter");
        return;
    };
    for named in [false, true] {
        let scripts = Scratch::new();
        let config = Scratch::new();
        scripts.package("t", true, "local c = eqoa.settings.load(); seen = c and c.opt or 'none'");
        let mut manager = gpu.manager(&scripts.0, Some(config.0.clone()));
        if named {
            manager.enable_profiles(Some(config.0.clone()));
        }
        let seen = |m: &Manager| m.entries[0].instance.as_ref().unwrap().host.lua.globals().get::<mlua::Value>("seen").unwrap();
        for _ in 0..4 {
            gpu.frame(&mut manager);
        }
        let id = manager.entries[0].metadata.id.clone();
        assert!(matches!(seen(&manager), mlua::Value::String(ref s) if &*s.to_str().unwrap() == "none"), "named={named}: nothing saved yet");
        // control: frames with no option write change nothing
        for _ in 0..3 {
            gpu.frame(&mut manager);
        }
        assert!(matches!(seen(&manager), mlua::Value::String(ref s) if &*s.to_str().unwrap() == "none"), "named={named}: control");
        super::super::options::set_toggle_persisted(&id, "opt", true).unwrap();
        for _ in 0..3 {
            gpu.frame(&mut manager);
        }
        if named {
            assert!(matches!(seen(&manager), mlua::Value::String(ref s) if &*s.to_str().unwrap() == "none"), "named: the profile's settings are untouched");
        } else {
            assert!(matches!(seen(&manager), mlua::Value::Boolean(true)), "legacy: the option written is what load() gives, got {:?}", seen(&manager));
        }
        manager.shutdown();
    }
    super::super::options::reset_in_memory();
}
