use super::host::Host;

#[test]
fn native_capabilities_are_probeable_and_stubs_diagnose_even_when_caught() {
    let h = Host::new().unwrap();
    h.execute(
        r#"
        assert(eqoa.api == 'eqoa/1')
        assert(eqoa.capabilities.has('eqoa.capabilities.get'))
        assert(eqoa.capabilities.get('eqoa.camera.get').status == 'supported')
        assert(eqoa.capabilities.get('eqoa.actions.move').status == 'not_yet')
        assert(eqoa.capabilities.get('eqoa.no_such_symbol').status == 'unknown')
        local v,e = eqoa.actions.move()
        assert(v == nil and e.code == 'not_yet')
        local ok = pcall(function() return eqoa.player.misspelled end)
        assert(ok)
    "#,
    )
    .unwrap();
    let logs = h.drain_logs().unwrap();
    assert!(logs.iter().any(|s| s.contains("eqoa.actions.move")));
    assert!(logs.iter().any(|s| s.contains("eqoa.player.misspelled")));
}

#[test]
fn native_pregame_getters_do_not_fabricate_world_or_hud_claims() {
    let h = Host::new().unwrap();
    h.execute(
        r#"
        assert(eqoa.client.get().in_world == false)
        local p,e = eqoa.player.get()
        assert(p == nil and e.code == 'not_in_world')
        local claim,e = eqoa.hud.hide('health')
        assert(claim == nil and e.code == 'not_in_world')
        local owners,e = eqoa.hud.hidden_by('all')
        assert(owners == nil and e.code == 'unavailable')
        assert(eqoa.input.get().pad == nil)
        local ok = pcall(function() eqoa.api = 'changed' end)
        assert(not ok)
    "#,
    )
    .unwrap();
}

#[test]
fn native_events_allow_removal_during_dispatch_and_report_real_overflow() {
    use super::{
        native_core::{CORE, Core},
        native_events,
    };
    CORE.with(|c| *c.borrow_mut() = Core::default());
    let h = Host::new().unwrap();
    h.execute("calls={};eqoa.events.on('chat.message',function(e) calls[#calls+1]='first';eqoa.events.off(second) end);second=eqoa.events.on('chat.message',function() calls[#calls+1]='second' end);eqoa.events.once('chat.message',function() calls[#calls+1]='once' end);eqoa.events.on('events.dropped',function(e) dropped=e.data.count end);eqoa.events.after(0,function() timer_called=true end)").unwrap();
    super::native_core::observe_text("one", [1.0; 4]);
    native_events::dispatch(&h).unwrap();
    h.execute("assert(table.concat(calls,',')=='first,once');assert(timer_called)")
        .unwrap();
    for _ in 0..300 {
        super::native_core::observe_text("burst", [1.0; 4]);
    }
    native_events::dispatch(&h).unwrap();
    h.execute("assert(dropped==nil);assert(#calls==258)")
        .unwrap();
    native_events::dispatch(&h).unwrap();
    h.execute("assert(#calls==302 and dropped==nil)").unwrap();
    for _ in 0..1300 {
        super::native_core::observe_text("overflow", [1.0; 4]);
    }
    native_events::dispatch(&h).unwrap();
    h.execute("assert(dropped==276 and #calls==556)").unwrap();
}

#[test]
fn native_input_claims_conflict_across_vms_and_release_on_vm_exit() {
    super::input::reset_input_state();
    let a = Host::new().unwrap();
    let b = Host::new().unwrap();
    a.execute("claim=assert(eqoa.input.claim('Enter',true));assert(eqoa.input.capture_text(true))")
        .unwrap();
    b.execute("local v,e=eqoa.input.claim('Enter');assert(v==nil and e.code=='busy')")
        .unwrap();
    drop(a);
    b.execute("claim=assert(eqoa.input.claim('Enter'));assert(eqoa.input.release(claim)==true);assert(eqoa.input.release(claim)==false)").unwrap();
}

#[test]
fn native_settings_reject_nonplain_data_and_wait_for_a_real_writer() {
    let h = Host::new().unwrap();
    h.execute("for _,v in ipairs({{[1]=true,[3]=true},{[1]=true,a=true},setmetatable({},{}),{fn=function()end}}) do local ok,e=eqoa.settings.save(v);assert(ok==nil and e.code=='invalid_argument') end;local ok,e=eqoa.settings.save({size=20});assert(ok==nil and e.code=='unavailable')").unwrap();
}

#[test]
fn native_legacy_chat_queue_is_owned_bounded_and_cancelled_on_disable() {
    use super::{chat, native_events};
    chat::take_outgoing_chat();
    let a = Host::new().unwrap();
    let b = Host::new().unwrap();
    chat::install_chat_adapter(&a).unwrap();
    chat::install_chat_adapter(&b).unwrap();
    a.snapshot.set(super::host::Snapshot {
        in_game: true,
        ..Default::default()
    });
    b.snapshot.set(super::host::Snapshot {
        in_game: true,
        ..Default::default()
    });
    a.lua
        .globals()
        .set(
            "chat",
            a.registered_modules()["frontiers_forge.chat"].clone(),
        )
        .unwrap();
    b.lua
        .globals()
        .set(
            "chat",
            b.registered_modules()["frontiers_forge.chat"].clone(),
        )
        .unwrap();
    a.execute("assert(chat.SendChatText('a'));assert(chat.IsSendPending())")
        .unwrap();
    b.execute("assert(not chat.IsSendPending());assert(chat.SendChatText('b'))")
        .unwrap();
    native_events::clear(&a);
    assert_eq!(chat::take_outgoing_chat(), vec!["b"]);
    b.execute("for i=1,32 do assert(chat.SendChatText('queued')) end;assert(not chat.SendChatText('overflow'));assert(chat.UninstallSendHook());assert(not chat.IsSendPending());assert(not chat.SendChatText('after uninstall'))").unwrap();
    assert!(chat::take_outgoing_chat().is_empty());
}
#[test]
fn native_iteration_shares_instruction_budget_across_entry_and_callbacks() {
    let h = Host::new().unwrap();
    h.execute("UiForge.RegisterCallback(UiForge.CallbackType.Save,function() for i=1,2500000 do end;return 1 end)").unwrap();
    let entry = h.compile("for i=1,2500000 do end").unwrap();
    h.frame_entry(&entry).unwrap();
    assert!(
        h.callback(2, mlua::Value::Nil).is_err(),
        "entry and Save must share one 4M instruction budget"
    );
    h.end_iteration();
    assert!(
        h.callback(2, mlua::Value::Nil).is_ok(),
        "new top-level boundary replenishes the budget"
    );
}
#[test]
fn native_subscriber_heap_exhaustion_is_terminal() {
    let h = Host::new().unwrap();
    h.execute("eqoa.events.on('chat.message',function() local x=string.rep('x',32*1024*1024) end)")
        .unwrap();
    super::native_core::observe_text("heap test", [1.0; 4]);
    assert!(super::native_events::dispatch(&h).is_err());
}

#[test]
fn native_storage_quota_combines_private_files_default_settings_and_named_profiles() {
    use super::{
        named_profiles::{Document, Store},
        storage::Storage,
    };
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let config = std::env::temp_dir().join(format!(
        "eqoa-native-quota-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let key = "a".repeat(64);
    let storage = Storage::new(Some(config.clone()));
    storage
        .save_encoded(&key, &serde_json::json!("old"))
        .unwrap();
    let data = storage.files_dir(&key).unwrap();
    std::fs::create_dir_all(data.join("files")).unwrap();
    let file = std::fs::File::create(data.join("files/large.bin")).unwrap();
    file.set_len(16 * 1024 * 1024 - 8).unwrap();
    drop(file);
    assert!(
        storage
            .save_encoded(&key, &serde_json::json!("too large"))
            .is_err(),
        "settings must share the file quota"
    );
    assert_eq!(
        std::fs::read_to_string(storage.profile_path(&key).unwrap()).unwrap(),
        "\"old\""
    );
    let store = Store::new(Some(config.clone()));
    let mut doc = Document::empty();
    doc.addons.insert(key.clone(), serde_json::json!("x"));
    store.write("First", &doc).unwrap();
    assert!(
        store.write("Second", &doc).is_err(),
        "saved profiles must share the addon aggregate quota"
    );
    assert!(
        super::sandbox::check_quota(&data, &data.join("new.bin"), 1).is_err(),
        "file writes must include existing settings"
    );
    std::fs::remove_dir_all(config).unwrap();
}

#[test]
fn native_events_share_one_retention_limit_for_sources_and_resource_completions() {
    use super::{
        native_core::{CORE, Core},
        native_events,
    };
    CORE.with(|c| *c.borrow_mut() = Core::default());
    let h = Host::new().unwrap();
    h.execute("seen=0;resources=0;dropped=0;eqoa.events.on('chat.message',function() seen=seen+1 end);eqoa.events.on('resource.ready',function() seen=seen+1;resources=resources+1 end);eqoa.events.on('events.dropped',function(e) dropped=dropped+e.data.count end)").unwrap();
    for _ in 0..200 {
        super::native_core::observe_text("queued", [1.0; 4]);
    }
    for _ in 0..100 {
        native_events::resource(&h.lua, "texture", mlua::Value::Nil, None).unwrap();
    }
    native_events::dispatch(&h).unwrap();
    h.execute("assert(seen==256 and resources==56 and dropped==0)")
        .unwrap();
    native_events::dispatch(&h).unwrap();
    h.execute("assert(seen==300 and resources==100 and dropped==0)")
        .unwrap();
    h.execute("seen=0;resources=0;dropped=0").unwrap();
    for _ in 0..1300 {
        native_events::resource(&h.lua, "texture", mlua::Value::Nil, None).unwrap();
    }
    native_events::dispatch(&h).unwrap();
    h.execute("assert(seen==254 and resources==254 and dropped==276)")
        .unwrap();
    for _ in 0..4 {
        native_events::dispatch(&h).unwrap();
    }
    h.execute("assert(seen==1024 and resources==1024 and dropped==276)")
        .unwrap();
}

#[test]
fn native_memory_exhaustion_cannot_be_swallowed_by_nested_pcall() {
    let h = Host::new().unwrap();
    let result = h.execute(
        "pcall(function() pcall(function() local x=string.rep('x',32*1024*1024) end) end)",
    );
    assert!(
        result.is_err(),
        "heap exhaustion must escape every protected call and fail this VM boundary"
    );
}

#[test]
fn native_events_enforce_two_mebibyte_payload_retention_without_losing_dispatch_backlog() {
    use super::{
        native_core::{CORE, Core},
        native_events,
    };
    CORE.with(|c| *c.borrow_mut() = Core::default());
    let h = Host::new().unwrap();
    h.execute("count=0;lost=0;eqoa.events.on('chat.message',function() count=count+1 end);eqoa.events.on('events.dropped',function(e) lost=e.data.count end)").unwrap();
    for _ in 0..1000 {
        super::native_core::observe_text(&"x".repeat(8192), [1.0; 4]);
    }
    native_events::dispatch(&h).unwrap();
    h.execute("assert(count>200 and count<256);assert(lost+count==1000)")
        .unwrap();
}

#[test]
fn native_forge_input_defaults_use_raw_neutral_byte_128() {
    let snapshot = super::input::InputSnapshot::default();
    assert_eq!(snapshot.right_analog, (128, 128));
    assert_eq!(snapshot.left_analog, (128, 128));
    assert_eq!(snapshot.button_bitfield, 0xffff);
}

#[test]
fn review_fix_m4_native_binding_preserves_runtime_io_and_argument_error_taxonomy() {
    let h = Host::new().unwrap();
    super::native::bind(
        &h.lua,
        "text",
        "localized",
        h.lua
            .create_function(|_, (): ()| -> mlua::Result<()> {
                Err(mlua::Error::runtime("backend upload failed"))
            })
            .unwrap(),
    )
    .unwrap();
    h.execute("local v,e=eqoa.text.localized();assert(v==nil and e.code=='failed')")
        .unwrap();
    for (kind, expected) in [
        (std::io::ErrorKind::PermissionDenied, "permission_denied"),
        (std::io::ErrorKind::NotFound, "not_found"),
        (std::io::ErrorKind::Other, "io_error"),
    ] {
        super::native::bind(
            &h.lua,
            "text",
            "localized",
            h.lua
                .create_function(move |_, (): ()| -> mlua::Result<()> {
                    Err(mlua::Error::external(std::io::Error::new(
                        kind,
                        "backend IO failure",
                    )))
                })
                .unwrap(),
        )
        .unwrap();
        h.execute(&format!(
            "local v,e=eqoa.text.localized();assert(v==nil and e.code=='{expected}')"
        ))
        .unwrap();
    }
    super::native::bind(
        &h.lua,
        "text",
        "localized",
        h.lua.create_function(|_, _: mlua::Table| Ok(())).unwrap(),
    )
    .unwrap();
    h.execute("local v,e=eqoa.text.localized(false);assert(v==nil and e.code=='invalid_argument')")
        .unwrap();
    assert!(
        h.drain_logs()
            .unwrap()
            .iter()
            .any(|line| line.contains("backend upload failed") && line.contains("failed"))
    );
}

#[test]
fn review_fix_l2_experience_threshold_is_for_the_requested_level() {
    let h = Host::new().unwrap();
    for level in [1, 2, 50, 51, 59] {
        h.execute(&format!(
            "assert(eqoa.player.experience_for_level({level})=={})",
            crate::hud::exp_for_level(level)
        ))
        .unwrap();
    }
    h.execute("assert(eqoa.player.experience_for_level(0)==nil);assert(eqoa.player.experience_for_level(60)==nil);local v,e=eqoa.player.experience_for_level('2');assert(v==nil and e.code=='invalid_argument')").unwrap();
}

#[test]
fn review_fix_l3_capability_prefixes_match_complete_dotted_segments() {
    let h = Host::new().unwrap();
    h.execute(r#"
        assert(#eqoa.capabilities.list('eqoa.play')==0)
        local subtree=eqoa.capabilities.list('eqoa.player')
        assert(#subtree>=3)
        for _,descriptor in ipairs(subtree) do assert(descriptor.path=='eqoa.player' or descriptor.path:sub(1,12)=='eqoa.player.') end
        assert(#eqoa.capabilities.list('eqoa.player.get')==1)
        assert(#eqoa.capabilities.list()==#eqoa.capabilities.list(''))
    "#).unwrap();
}

#[test]
fn perf_review_m4_overflow_diagnostic_survives_discard() {
    use super::{native_core, native_events};
    native_core::CORE.with(|c| *c.borrow_mut() = native_core::Core::default());
    native_core::discard_deferred();
    let h = Host::new().unwrap();
    for _ in 0..8193 {
        native_core::defer(1, |_| {});
    }
    native_events::clear(&h);
    native_core::discard_deferred();
    let logs = h.drain_logs().unwrap();
    assert!(
        logs.iter()
            .any(|l| l.contains("source.capture") && l.contains("overflow")),
        "overflow loss vanished: {logs:?}"
    );
}
#[test]
fn perf_review_m5_generation_drop_is_diagnosed_and_marked() {
    use super::native_core;
    native_core::CORE.with(|c| *c.borrow_mut() = native_core::Core::default());
    native_core::discard_deferred();
    let h = Host::new().unwrap();
    native_core::observe_text("old-world secret", [1.; 4]);
    native_core::CORE.with(|c| c.borrow_mut().reset("zone"));
    native_core::flush_deferred();
    let logs = h.drain_logs().unwrap();
    assert!(
        logs.iter().any(|l| l.contains("source.generation")),
        "generation loss vanished: {logs:?}"
    );
    native_core::CORE.with(|c| {
        let c = c.borrow();
        assert!(c.events.iter().any(|e| e.name == "events.dropped"
            && e.data["streams"][0] == "source.generation"
            && e.data["count"] == 1));
        assert!(!c.events.iter().any(|e| e.name == "chat.message"));
    });
    assert!(!logs.iter().any(|l| l.contains("old-world secret")));
}
#[test]
fn perf_review_l1_chat_scroll_preserves_source_order() {
    use super::native_core;
    native_core::CORE.with(|c| *c.borrow_mut() = native_core::Core::default());
    native_core::discard_deferred();
    let _h = Host::new().unwrap();
    native_core::observe_text("preceding line", [1.; 4]);
    crate::fork_ext::chat_history::key(19, true);
    native_core::flush_deferred();
    native_core::CORE.with(|c| {
        let c = c.borrow();
        let names = c.events.iter().map(|e| e.name.as_str()).collect::<Vec<_>>();
        assert_eq!(names, ["chat.message", "chat.history_scroll"]);
    });
    crate::fork_ext::chat_history::key(19, false);
}

#[test]
fn perf_review_m5_generation_drop_is_visible_when_discarded_without_flush() {
    use super::{native_core, native_events};
    native_core::CORE.with(|c| *c.borrow_mut() = native_core::Core::default());
    native_core::discard_deferred();
    let h = Host::new().unwrap();
    native_core::observe_text("old-world secret", [1.; 4]);
    native_core::CORE.with(|c| c.borrow_mut().reset("zone"));
    native_events::clear(&h);
    let logs = h.drain_logs().unwrap();
    assert!(logs.iter().any(|l| l.contains("source.generation")));
    assert!(!logs.iter().any(|l| l.contains("old-world secret")));
}

#[test]
fn round_3_stage_a_player_stats_api() {
    use super::native_core::{CORE, Core};
    use serde_json::json;

    // Negative control: pregame / not in world
    CORE.with(|c| *c.borrow_mut() = Core::default());
    let h = Host::new().unwrap();
    h.execute(
        r#"
        local s, err = eqoa.player.stats()
        assert(s == nil and err.code == 'not_in_world', "must fail when not in world")
    "#,
    )
    .unwrap();

    // In world with populated player_stats
    CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.view.in_world = true;
        c.view.phase = "world";
        c.view.player_stats = Some(json!({
            "rows": [
                { "name": "strength", "word": 226, "stat_id": 0, "current": 120, "base": 100, "min": 100, "max": 100, "change": 20, "shift": "raised" },
                { "name": "armor", "word": 242, "stat_id": 14, "current": 45, "base": 45, "min": 45, "max": 45, "change": 0, "shift": "same" },
            ],
            "strength": { "name": "strength", "word": 226, "stat_id": 0, "current": 120, "base": 100, "min": 100, "max": 100, "change": 20, "shift": "raised" },
            "armor": { "name": "armor", "word": 242, "stat_id": 14, "current": 45, "base": 45, "min": 45, "max": 45, "change": 0, "shift": "same" },
        }));
    });

    h.execute(
        r#"
        local s, err = eqoa.player.stats()
        assert(err == nil, "must succeed in world")
        assert(s ~= nil, "stats table must exist")
        assert(s.strength.current == 120, "current strength")
        assert(s.strength.base == 100, "base strength")
        assert(s.strength.shift == "raised", "shift raised")
        assert(s.strength.change == 20, "change 20")
        assert(s.armor.current == 45, "current armor")
        assert(s.armor.shift == "same", "shift same")
        assert(#s.rows == 2, "rows count")
        assert(s.rows[1].name == "strength", "first row name")
    "#,
    )
    .unwrap();
}

#[test]
fn round_3_stage_a_actions_assist_and_summary_api() {
    use super::native_core::{CORE, Core};

    super::action_broker::reset_all_for_test();
    crate::fork_ext::bindpage::reset_in_memory();
    CORE.with(|c| *c.borrow_mut() = Core::default());
    let h = Host::for_addon(&super::discovery::DiscoveredAddon {
        id: "test_r3_actions".into(),
        name: "test_r3_actions".into(),
        entry: std::env::temp_dir().join("main.lua"),
        script_root: std::env::temp_dir(),
        package_root: std::env::temp_dir(),
        module_dirs: vec![std::env::temp_dir()],
        resource_dirs: vec![std::env::temp_dir()],
        api: Some("eqoa/1".into()),
        entry_mode: super::discovery::EntryMode::Once,
        capabilities_declared: true,
        requested_capabilities: vec!["eqoa.actions.assist".into()],
        extensions: Default::default(),
        initially_enabled: true,
        mods_root: None,
    })
    .unwrap();
    // Without eqoa.actions.assist capability: refused with permission_denied.
    let denied = Host::for_addon(&super::discovery::DiscoveredAddon {
        id: "test_r3_no_cap".into(),
        name: "test_r3_no_cap".into(),
        entry: std::env::temp_dir().join("main.lua"),
        script_root: std::env::temp_dir(),
        package_root: std::env::temp_dir(),
        module_dirs: vec![std::env::temp_dir()],
        resource_dirs: vec![std::env::temp_dir()],
        api: Some("eqoa/1".into()),
        entry_mode: super::discovery::EntryMode::Once,
        capabilities_declared: true,
        requested_capabilities: Vec::new(),
        extensions: Default::default(),
        initially_enabled: true,
        mods_root: None,
    })
    .unwrap();
    denied
        .execute(
            r#"
        local ok, err = eqoa.actions.assist()
        assert(ok == nil and err.code == 'permission_denied', "assist requires capability")
    "#,
        )
        .unwrap();

    // Outside a gesture (with Host::new() capability bypass / declared cap): reaches in-world gate directly.
    h.execute(
        r#"
        local ok, err = eqoa.actions.assist()
        assert(ok == nil and err.code == 'not_in_world', "assist not in world without gesture")
        local ok2, err2 = eqoa.windows.open("summary")
        assert(ok2 == nil and err2.code == 'not_in_world', "summary not in world")
        local ok3, err3 = eqoa.windows.open("unsupported_window")
        assert(ok3 == nil and err3.code == 'invalid_argument', "unsupported window")
        eqoa.input.bind({ id = "assist_go", label = "Assist Go", default_key = "k" }, function()
            last_req, last_err = eqoa.actions.assist()
        end)
    "#,
    )
    .unwrap();
    let press_k = || {
        CORE.with(|c| {
            c.borrow_mut().emit(
                "input.key",
                serde_json::json!({"key":"k","down":true,"repeat":false,"synthetic":false,"text":""}),
            )
        });
        super::native_events::dispatch(&h).unwrap();
    };
    press_k();
    h.execute("assert(last_req == nil and last_err.code == 'not_in_world', 'assist not in world')")
        .unwrap();

    // In world: queue assist inside bound key gesture
    CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.view.in_world = true;
    });

    super::actions::take_action_requests(); // drain any leftover
    press_k();
    h.execute("assert(last_req ~= nil and last_err == nil, 'assist queued with RequestId')")
        .unwrap();
    assert_eq!(super::action_broker::queued_kinds_for_test(), vec!["assist"]);
    assert_eq!(
        super::action_broker::dispatch_all_for_test(),
        vec!["assist"],
        "accepted assist must reach broker dispatch"
    );
    super::action_broker::reset_all_for_test();

    h.execute(
        r#"
        local ok, err = eqoa.windows.open("summary")
        assert(ok == true and err == nil, "summary succeeded")
    "#,
    )
    .unwrap();
    let reqs2 = super::actions::take_action_requests();
    assert!(!reqs2.assist, "assist must not be requested");
    assert!(reqs2.summary, "summary must be requested");
    crate::fork_ext::bindpage::reset_in_memory();
}

#[test]
fn round_3_stage_a_hud_frame_and_layout_and_visible() {
    use super::native_core::{CORE, Core};

    // Negative control: not in world -> visible is false
    CORE.with(|c| *c.borrow_mut() = Core::default());
    let h = Host::new().unwrap();
    h.execute(
        r#"
        assert(eqoa.hud.visible("health") == false, "not in world visible is false")
        assert(eqoa.hud.visible() == false, "not in world visible is false")
    "#,
    )
    .unwrap();

    // In world
    CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.view.in_world = true;
    });

    h.execute(
        r#"
        assert(eqoa.hud.visible("health") == true, "health visible in world")
        assert(eqoa.hud.visible("target") == true, "target visible in world")
        local f = eqoa.hud.frame()
        assert(f.width == 640 and f.height == 448, "default 640x448 frame")
        local custom_f = eqoa.hud.frame({ width = 1280, height = 720 })
        assert(custom_f.window_width == 1280 and custom_f.window_height == 720, "custom resolution")

        local l = eqoa.hud.layout()
        assert(l.player.health ~= nil, "player health rect")
        assert(l.target.health ~= nil, "target health rect")
        assert(l.target_of_target == nil, "tot removed from layout API")
        assert(#l.group_members == 4, "4 group members")
        assert(l.group_members[1].health ~= nil, "member 1 health")
        assert(l.pet.panel ~= nil, "pet panel")
        assert(l.stats_box == nil, "stats box removed from layout API")
    "#,
    )
    .unwrap();
}

#[test]
fn round_3_stage_a_options_toggle_and_get() {
    let h = option_test_host("my_custom_mod");

    // Negative control: invalid argument
    h.execute(
        r#"
        local ok, err = eqoa.options.toggle("invalid")
        assert(ok == nil and err.code == 'invalid_argument', "toggle invalid arg")
        local ok2, err2 = eqoa.options.toggle({})
        assert(ok2 == nil and err2.code == 'invalid_argument', "toggle missing id")
    "#,
    )
    .unwrap();

    // Valid registration under the caller's real addon ID
    h.execute(
        r#"
        local t, err = eqoa.options.toggle({ id = "test_opt", label = "Test Option", default = true })
        assert(err == nil, "toggle registered")
        assert(t.id == "test_opt")
        assert(t.value == true)
        local val = eqoa.options.get("test_opt")
        assert(val == true, "get toggle matches")
    "#,
    )
    .unwrap();

    // Change setting via real addon ID
    let item_val = super::options::get_toggle("my_custom_mod", "test_opt");
    assert_eq!(item_val, Some(true));
    super::options::set_toggle("my_custom_mod", "test_opt", false);

    h.execute(
        r#"
        local val = eqoa.options.get("test_opt")
        assert(val == false, "updated toggle value")
    "#,
    )
    .unwrap();

    // Second mod with the same toggle id ("test_opt") does not collide with my_custom_mod
    let h2 = option_test_host("other_custom_mod");
    h2.execute(
        r#"
        local t, err = eqoa.options.toggle({ id = "test_opt", label = "Other Option", default = true })
        assert(err == nil and t.value == true, "second mod gets its own toggle state")
        assert(eqoa.options.get("test_opt") == true)
    "#,
    )
    .unwrap();
    assert_eq!(
        super::options::get_toggle("my_custom_mod", "test_opt"),
        Some(false),
        "first mod toggle state preserved"
    );

    // Disabling a mod hides its option rows and key rows
    crate::fork_ext::bindpage::register("my_custom_mod", "act", "k", "Custom Act");
    assert!(
        super::options::active_mod_items()
            .iter()
            .any(|(_, label)| label == "Test Option")
    );
    let rows_before = crate::fork_ext::bindpage::extra_rows();
    super::options::set_addon_enabled("my_custom_mod", false);
    crate::fork_ext::bindpage::set_addon_enabled("my_custom_mod", false);
    assert!(
        !super::options::active_mod_items()
            .iter()
            .any(|(_, label)| label == "Test Option"),
        "disabled mod option row disappears"
    );
    assert_eq!(
        crate::fork_ext::bindpage::extra_rows(),
        rows_before - 1,
        "disabled mod bind row disappears"
    );
    assert_eq!(
        crate::fork_ext::bindpage::dispatch_keypress("k"),
        None,
        "disabled mod keybind does not fire"
    );
    super::options::reset_in_memory();
    crate::fork_ext::bindpage::reset_in_memory();
}

#[test]
fn round_3_stage_a_input_bind_and_fonts_game() {
    let h = Host::new().unwrap();

    // Negative control: invalid argument
    h.execute(
        r#"
        local ok, err = eqoa.input.bind("not_table")
        assert(ok == nil and err.code == 'invalid_argument', "bind invalid arg")
        local ok2, err2 = eqoa.input.bind({})
        assert(ok2 == nil and err2.code == 'invalid_argument', "bind missing id")
    "#,
    )
    .unwrap();

    // Valid bind registration and HUD layer negative controls outside world
    h.execute(
        r#"
        local ok, err = eqoa.binds.register("toggle_stats", "b", "Stats Box")
        assert(ok == true and err == nil, "bind registered via eqoa.binds.register")

        -- Negative controls: outside world returns not_in_world
        local res, err_text = eqoa.hud.text({ x = 10, y = 20, text = "Test" })
        assert(res == nil and err_text.code == "not_in_world", "hud.text outside world returns not_in_world")

        local res_g, err_g = eqoa.hud.gauge({ x = 10, y = 20, art = 9, fill = 0.5 })
        assert(res_g == nil and err_g.code == "not_in_world", "hud.gauge outside world returns not_in_world")

        local res_i, err_i = eqoa.hud.image({ ui_id = 25, x = 10, y = 20, w = 100 })
        assert(res_i == nil and err_i.code == "not_in_world", "hud.image outside world returns not_in_world")
    "#,
    )
    .unwrap();
}

#[test]
fn round_3_stage_a_controls_page_rebind_and_negative_control() {
    let entries_before = crate::bindpage::defaults();
    let _ = crate::fork_ext::bindpage::register("mod_test", "custom_key", "p", "Custom Action");
    let entries_after = crate::bindpage::defaults();
    assert_eq!(
        entries_after,
        entries_before + 1,
        "ControlsPage defaults entry shifted down by 1 extra row"
    );

    // Negative control: old key does not fire after rebind
    assert!(crate::fork_ext::bindpage::set_key(
        "mod_test",
        "custom_key",
        "z"
    ));
    assert_eq!(
        crate::fork_ext::bindpage::get_key("mod_test", "custom_key"),
        Some("z".into())
    );
    assert_eq!(
        crate::fork_ext::bindpage::dispatch_keypress("p"),
        None,
        "old key does not fire"
    );
    assert_eq!(
        crate::fork_ext::bindpage::dispatch_keypress("z"),
        Some(("mod_test".into(), "custom_key".into())),
        "new key fires"
    );
}

#[test]
fn round_3_stage_a_entity_and_group_power_fractions() {
    use super::native_core::CORE;
    use serde_json::json;

    CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.view.in_world = true;
        c.view.phase = "world";
        c.view.player = json!({ "id": 1, "target_id": 2 });
        c.view.entities = vec![
            json!({
                "id": 1,
                "name": "Self",
                "hp_fraction": 1.0,
                "power": 0.85,
                "kind": "player",
                "class_code": 2,
            }),
            json!({
                "id": 2,
                "name": "Target",
                "hp_fraction": 0.5,
                "power": null, // negative control: server sent no power
                "kind": "npc",
                "class_code": 0,
            }),
        ];
        c.view.group = json!({
            "in_group": true,
            "self_leader": true,
            "members": [
                { "id": 1, "name": "Self", "active": true, "hp_fraction": 1.0, "power": 0.85 },
                { "id": 3, "name": "Cleric", "active": true, "hp_fraction": 0.9, "power": 0.4 },
            ],
            "raid": [],
            "invites": [],
        });
    });

    let h = Host::new().unwrap();
    h.execute(
        r#"
        local entities = eqoa.entities.list()
        assert(#entities == 2, "2 entities")
        assert(entities[1].power == 0.85, "self power fraction 0.85")
        assert(entities[2].power == nil, "negative control: target power nil when not sent")

        local grp = eqoa.group.get()
        assert(grp.in_group == true, "in group")
        assert(#grp.members == 2, "2 members")
        assert(grp.members[1].power == 0.85, "member 1 power")
        assert(grp.members[2].power == 0.4, "member 2 power")
    "#,
    )
    .unwrap();
}

#[test]
fn round_3_hud_layer_draw_calls_and_queue() {
    use super::hud_layer;
    use super::native::session;
    use super::native_core::CORE;

    CORE.with(|c| {
        c.borrow_mut().view.in_world = true;
    });

    let h = Host::new().unwrap();
    session(&h.lua).borrow_mut().addon = "hud_layer_test".to_string();
    hud_layer::clear_all();

    h.execute(
        r#"
        local ok1, err1 = eqoa.hud.text({ x = 99, y = 35, text = "100%", scale = 0.6, align = "center", outline = true })
        assert(ok1 == true and err1 == nil, "hud.text succeeds in world")

        local ok2, err2 = eqoa.hud.gauge({ x = 30, y = 335, art = 9, fill = 0.75, lit = true })
        assert(ok2 == true and err2 == nil, "hud.gauge succeeds in world")

        local ok3, err3 = eqoa.hud.image({ ui_id = 25, x = 20, y = 300, w = 174 })
        assert(ok3 == true and err3 == nil, "hud.image succeeds in world")

        local ok4, err4 = eqoa.hud.panel({ x = 200, y = 54, w = 148, h = 132 })
        assert(ok4 == true and err4 == nil, "hud.panel succeeds in world")
    "#,
    )
    .unwrap();

    let mut count = 0;
    hud_layer::MOD_QUEUES.with(|q| {
        let map = q.borrow();
        if let Some(queue) = map.get("hud_layer_test") {
            count = queue.len();
        }
    });
    assert_eq!(count, 4, "all 4 commands queued");

    hud_layer::clear_all();
}

// Package metadata supplies identity exactly as discovery does in production.
fn option_test_host(id: &str) -> Host {
    use super::discovery::{DiscoveredAddon, EntryMode};
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../bundled-mods/eqoa_stats_box");
    Host::for_addon(&DiscoveredAddon {
        id: id.into(),
        name: id.into(),
        entry: root.join("main.lua"),
        script_root: root.clone(),
        package_root: root.clone(),
        module_dirs: vec![root.clone()],
        resource_dirs: vec![root],
        api: Some("eqoa/1".into()),
        entry_mode: EntryMode::Once,
        capabilities_declared: true,
        requested_capabilities: Vec::new(),
        extensions: Default::default(),
        initially_enabled: true,
        mods_root: None,
    })
    .unwrap()
}

#[test]
fn failed_option_saves_leave_lua_and_controls_values_unchanged_and_diagnose() {
    super::options::set_storage_root(None);
    crate::settings::testing::reset(true);
    super::options::reset_in_memory();
    let denied = option_test_host("eqoa_stats_box");
    denied
        .execute(
            r#"
        local direct,direct_err = eqoa.actions.toggle_stats_box()
        assert(direct == nil and direct_err.code == 'permission_denied')
    "#,
        )
        .unwrap();
    let mut cap_addon = super::discovery::DiscoveredAddon {
        id: "eqoa_stats_box".into(),
        name: "eqoa_stats_box".into(),
        entry: std::env::temp_dir().join("eqoa-option-test-eqoa_stats_box").join("main.lua"),
        script_root: std::env::temp_dir().join("eqoa-option-test-eqoa_stats_box"),
        package_root: std::env::temp_dir().join("eqoa-option-test-eqoa_stats_box"),
        module_dirs: vec![std::env::temp_dir().join("eqoa-option-test-eqoa_stats_box")],
        resource_dirs: vec![std::env::temp_dir().join("eqoa-option-test-eqoa_stats_box")],
        api: Some("eqoa/1".into()),
        entry_mode: super::discovery::EntryMode::Once,
        capabilities_declared: true,
        requested_capabilities: vec!["eqoa.actions.toggle_stats_box".into()],
        extensions: Default::default(),
        initially_enabled: true,
        mods_root: None,
    };
    let host = Host::for_addon(&cap_addon).unwrap();
    cap_addon.requested_capabilities.clear();
    host.execute(
        r#"
        eqoa.options.toggle({id='stats_box',default=false})
        local ok,err = eqoa.options.set('stats_box',true)
        assert(ok == nil and err.code == 'io_error')
        assert(eqoa.options.get('stats_box') == false)
        local direct,direct_err = eqoa.actions.toggle_stats_box()
        assert(direct == nil and direct_err.code == 'io_error')
        eqoa.input.bind({id='toggle',label='Toggle Stats Box',default_key='b'}, function()
            last_state, last_action_err = eqoa.actions.toggle_stats_box()
        end)
    "#,
    )
    .unwrap();
    super::native_core::CORE.with(|c| {
        c.borrow_mut().emit(
            "input.key",
            serde_json::json!({"key":"b","down":true,"repeat":false,"synthetic":false,"text":""}),
        )
    });
    super::native_events::dispatch(&host).unwrap();
    host.execute(
        "assert(last_state == nil and last_action_err.code == 'io_error');assert(eqoa.options.get('stats_box') == false)",
    )
    .unwrap();
    assert!(!super::options::toggle_by_item(2000));
    assert_eq!(super::options::value_by_item(2000), Some(false));
    assert!(!super::options::set_toggle(
        "eqoa_stats_box",
        "stats_box",
        true
    ));
    assert_eq!(
        super::options::get_toggle("eqoa_stats_box", "stats_box"),
        Some(false)
    );
    let logs = host.drain_logs().unwrap();
    assert!(
        logs.iter()
            .any(|line| line.contains("eqoa.options.set") && line.contains("io_error"))
    );
    assert!(logs.iter().any(|line| line.contains("eqoa.actions.toggle_stats_box") && line.contains("io_error")));

    // Positive control: the same API and Controls row work once storage is writable.
    crate::settings::testing::reset(false);
    // testing::reset clears bindpage in memory; re-register the Controls bind as the client session would.
    crate::fork_ext::bindpage::register("eqoa_stats_box", "toggle", "b", "Toggle Stats Box");
    for expected in [true, false] {
        super::native_core::CORE.with(|c| {
            c.borrow_mut().emit(
                "input.key",
                serde_json::json!({"key":"b","down":true,"repeat":false,"synthetic":false,"text":""}),
            )
        });
        super::native_events::dispatch(&host).unwrap();
        host.execute(&format!(
            "assert(last_state == {expected} and last_action_err == nil)"
        ))
        .unwrap();
    }
    host.execute(
        r#"
        local ok,save_err = eqoa.options.set('stats_box',true)
        assert(ok == true and save_err == nil)
    "#,
    )
    .unwrap();
    assert!(super::options::toggle_by_item(2000));
    assert_eq!(super::options::value_by_item(2000), Some(false));
    super::options::reset_in_memory();
    crate::settings::testing::reset(false);
}

#[test]
fn option_and_bind_names_never_redirect_caller_identity() {
    crate::settings::testing::reset(false);
    super::options::set_storage_root(None);
    super::options::reset_in_memory();
    crate::fork_ext::bindpage::reset_in_memory();
    for id in ["test", "mod", "third_party"] {
        let host = option_test_host(id);
        host.execute(
            r#"
            assert(eqoa.options.get('stats_box') == nil)
            local toggle = eqoa.options.toggle({id='stats_box',default=true})
            assert(toggle.value == true)
            assert(eqoa.options.get('stats_box') == true)
            assert(eqoa.options.set('stats_box',false))
            assert(eqoa.options.get('stats_box') == false)
            assert(eqoa.input.bind({id='toggle',label='Toggle Stats Box',default_key='b'}, function()
                other_state, other_err = eqoa.actions.toggle_stats_box()
            end))
            assert(eqoa.binds.get('toggle') == 'b')
            assert(eqoa.binds.register('toggle','b','Toggle Stats Box'))
        "#,
        )
        .unwrap();
        super::native_core::CORE.with(|c| {
            c.borrow_mut().emit(
                "input.key",
                serde_json::json!({"key":"b","down":true,"repeat":false,"synthetic":false,"text":""}),
            )
        });
        super::native_events::dispatch(&host).unwrap();
        host.execute("assert(other_state == nil and other_err.code == 'permission_denied')")
            .unwrap();
        assert_eq!(super::options::get_toggle(id, "stats_box"), Some(false));
        assert!(crate::fork_ext::bindpage::get_key(id, "toggle").is_some());
        assert_eq!(
            super::options::get_toggle("eqoa_stats_box", "stats_box"),
            Some(false)
        );
        assert_eq!(
            crate::fork_ext::bindpage::get_key("eqoa_stats_box", "toggle"),
            None
        );
    }
    assert_eq!(
        super::options::get_toggle("unregistered", "stats_box"),
        None
    );
    super::options::reset_in_memory();
    crate::fork_ext::bindpage::reset_in_memory();
    crate::settings::testing::reset(false);
}

#[test]
fn explicit_foreign_option_reads_respect_enabled_state_without_fallback() {
    crate::settings::testing::reset(false);
    super::options::set_storage_root(None);
    super::options::reset_in_memory();
    let host = option_test_host("eqoa_target_of_target");
    host.execute(
        r#"
        assert(eqoa.options.get('mana_bars') == nil)
        assert(eqoa.options.get('mana_bars','eqoa_mana_bars') == true)
        assert(eqoa.options.get('missing','eqoa_mana_bars') == nil)
    "#,
    )
    .unwrap();
    assert!(super::options::effective_toggle(
        "eqoa_mana_bars",
        "mana_bars"
    ));
    super::options::set_addon_enabled("eqoa_mana_bars", false);
    assert!(!super::options::effective_toggle(
        "eqoa_mana_bars",
        "mana_bars"
    ));
    assert_eq!(
        super::options::get_toggle("eqoa_mana_bars", "mana_bars"),
        Some(true)
    );
    host.execute("assert(eqoa.options.get('mana_bars','eqoa_mana_bars') == false)")
        .unwrap();
    super::options::set_addon_enabled("eqoa_mana_bars", true);
    assert!(super::options::set_toggle(
        "eqoa_mana_bars",
        "mana_bars",
        false
    ));
    assert!(!super::options::effective_toggle(
        "eqoa_mana_bars",
        "mana_bars"
    ));
    super::options::reset_in_memory();
    crate::settings::testing::reset(false);
}

#[test]
fn stats_box_status_message_matches_the_saved_state() {
    assert_eq!(
        super::native_state::stats_box_message(true),
        "stats box shown"
    );
    assert_eq!(
        super::native_state::stats_box_message(false),
        "stats box hidden"
    );
}

#[test]
fn guarded_records_keep_cached_reads_missing_diagnostics_and_read_only_semantics() {
    let host = Host::new().unwrap();
    let record = super::native::record(
        &host.lua,
        "eqoa.guard_fixture",
        &serde_json::json!({
            "name":"cached", "optional":null, "nested":{"value":7}
        }),
    )
    .unwrap();
    host.lua.globals().set("cached_record", record).unwrap();
    host.execute(
        r#"
        for i=1,1000 do
            assert(cached_record.name=='cached')
            assert(cached_record.nested.value==7)
            assert(cached_record.optional==nil)
        end
        assert(not pcall(function() cached_record.name='changed' end))
        assert(not pcall(function() cached_record.new_field=true end))
        assert(not pcall(function() cached_record.nested.value=8 end))
    "#,
    )
    .unwrap();
    assert!(
        host.drain_logs().unwrap().is_empty(),
        "known nullable fields stay silent"
    );
    host.execute("assert(cached_record.missing_field==nil)")
        .unwrap();
    let logs = host.drain_logs().unwrap();
    assert!(
        logs.iter()
            .any(|line| line.contains("eqoa.guard_fixture.missing_field")
                && line.contains("unknown_symbol"))
    );
    host.lua.gc_collect().unwrap();
    host.execute("assert(cached_record.name=='cached' and cached_record.nested.value==7)")
        .unwrap();
}

#[test]
fn guarded_record_lua_cycles_are_collectible_without_rust_registry_roots() {
    let lua = mlua::Lua::new();
    let cycle = || {
        let backing = lua.create_table().unwrap();
        backing.raw_set("value", 7).unwrap();
        let proxy = super::native::guard(
            &lua,
            backing.clone(),
            "eqoa.gc_fixture",
            Some(vec!["value".into(), "self".into()]),
        )
        .unwrap();
        backing.raw_set("self", proxy.clone()).unwrap();
        assert_eq!(proxy.get::<u32>("value").unwrap(), 7);
    };
    // Warm callback/table allocator caches before measuring retained memory.
    for _ in 0..128 {
        cycle();
    }
    lua.gc_collect().unwrap();
    lua.gc_collect().unwrap();
    let baseline = lua.used_memory();
    for _ in 0..1024 {
        cycle();
    }
    lua.gc_collect().unwrap();
    lua.gc_collect().unwrap();
    assert!(
        lua.used_memory() <= baseline + 32 * 1024,
        "unreachable guarded Lua cycles retained memory: baseline={baseline}, after={}",
        lua.used_memory()
    );
}

#[test]
fn record_reuse_returns_fresh_values_after_a_change_and_the_same_table_when_unchanged() {
    use super::native_core::CORE;
    let h = Host::new().unwrap();
    CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.view.in_world = true;
        c.view.player = serde_json::json!({"id": 1, "level": 10});
    });
    h.execute(
        r#"
        local a = eqoa.player.get()
        local b = eqoa.player.get()
        assert(rawequal(a, b), 'unchanged value is reused')
        assert(a.level == 10)
        assert(not pcall(function() a.level = 11 end), 'reused record stays read-only')
        f1 = eqoa.hud.frame()
        assert(rawequal(f1, eqoa.hud.frame()))
    "#,
    )
    .unwrap();
    CORE.with(|c| c.borrow_mut().view.player = serde_json::json!({"id": 1, "level": 11}));
    h.execute(
        r#"
        local c = eqoa.player.get()
        assert(c.level == 11, 'a changed value is never served stale')
        local d = eqoa.hud.frame({width = 320, height = 224})
        assert(not rawequal(d, f1), 'a different frame request is not served from the cache')
    "#,
    )
    .unwrap();
}

#[test]
fn record_reuse_caches_lists_with_fresh_tables_and_this_frames_records() {
    use super::native_core::CORE;
    let h = Host::new().unwrap();
    CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.view.in_world = true;
        c.view.entities = serde_json::json!([
            {"id": 7, "name": "Rat", "kind": "npc", "position": {"x": 1.0, "y": 0.0, "z": 2.0}},
            {"id": 8, "name": "Bat", "kind": "npc", "position": {"x": 3.0, "y": 0.0, "z": 4.0}}
        ])
        .as_array()
        .unwrap()
        .clone();
    });
    h.execute(
        r#"
        local a = eqoa.entities.list()
        assert(#a == 2 and a[1].name == 'Rat' and a[2].name == 'Bat')
        local b = eqoa.entities.list()
        -- a list is the caller's own table (as before), its records are this frame's read-only ones
        assert(not rawequal(a, b), 'a list is a fresh table each call')
        assert(rawequal(a[1], b[1]), 'an unchanged record is reused instead of rebuilt')
        assert(not pcall(function() b[1].name = 'changed' end), 'reused records stay read-only')
        a[1] = false
        table.insert(a, 'extra')
        local c = eqoa.entities.list()
        assert(#c == 2 and c[1].name == 'Rat' and c[2].name == 'Bat', 'a caller edits only its own list')
    "#,
    )
    .unwrap();
    // One creature moves and the other despawns: the very next read has the new place, not a cache.
    CORE.with(|c| {
        c.borrow_mut().view.entities = serde_json::json!([
            {"id": 7, "name": "Rat", "kind": "npc", "position": {"x": 9.0, "y": 0.0, "z": 2.0}}
        ])
        .as_array()
        .unwrap()
        .clone();
    });
    h.execute(
        r#"
        local a = eqoa.entities.list()
        assert(#a == 1, 'a despawned creature is gone')
        assert(a[1].position.x == 9.0, 'a moved creature is read where it is now, not cached')
    "#,
    )
    .unwrap();
}

#[test]
fn hud_specs_with_metatable_defaults_and_plain_specs_both_read_their_fields() {
    use super::{hud_layer, native_core::CORE};
    let h = Host::new().unwrap();
    CORE.with(|c| c.borrow_mut().view.in_world = true);
    hud_layer::clear_all();
    h.execute(
        r#"
        local defaults = setmetatable({}, {__index = {x = 7, y = 9, text = 'from defaults'}})
        assert(eqoa.hud.text(defaults))
        assert(eqoa.hud.text({x = 1, y = 2, text = 'plain', scale = 2}))
        local ok, e = eqoa.hud.text({y = 2, text = 'no x'})
        assert(ok == nil and e.code == 'invalid_argument')
    "#,
    )
    .unwrap();
    let queued =
        hud_layer::MOD_QUEUES.with(|q| q.borrow().values().flatten().cloned().collect::<Vec<_>>());
    let texts = queued
        .iter()
        .filter_map(|c| match c {
            hud_layer::HudCommand::Text {
                x, y, text, scale, ..
            } => Some((*x, *y, text.clone(), *scale)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        texts.contains(&(7.0, 9.0, "from defaults".to_string(), 1.0)),
        "{texts:?}"
    );
    assert!(
        texts.contains(&(1.0, 2.0, "plain".to_string(), 2.0)),
        "{texts:?}"
    );
    hud_layer::clear_all();
}

/// What a Lua value reads back as, driven by the JSON it was built from (a record's own fields are
/// hidden behind its proxy, so it is read key by key, as an addon reads it).
fn read_back(value: &mlua::Value, shape: &serde_json::Value) -> serde_json::Value {
    use serde_json::Value as J;
    match (value, shape) {
        (mlua::Value::Table(t), J::Object(fields)) => {
            let mut out = serde_json::Map::new();
            for (k, v) in fields {
                let got: mlua::Value = t.get(k.as_str()).unwrap();
                if !matches!(got, mlua::Value::Nil) {
                    out.insert(k.clone(), read_back(&got, v));
                }
            }
            J::Object(out)
        }
        (mlua::Value::Table(t), J::Array(items)) => J::Array(
            items
                .iter()
                .enumerate()
                .map(|(i, v)| match t.raw_get::<mlua::Value>(i + 1).unwrap() {
                    mlua::Value::Nil => J::Null,
                    got => read_back(&got, v),
                })
                .collect(),
        ),
        (mlua::Value::Boolean(b), _) => J::Bool(*b),
        (mlua::Value::Number(n), _) => serde_json::json!(*n),
        (mlua::Value::Integer(n), _) => serde_json::json!(*n as f64),
        (mlua::Value::String(s), _) => J::String(s.to_str().unwrap().to_string()),
        (mlua::Value::UserData(_), _) => J::String("<userdata>".into()),
        other => panic!("unexpected {other:?}"),
    }
}

/// The one-call record build gives what the call-by-call one gives: the same fields, nested records,
/// arrays with a hole, `$ref` userdata and empty objects; read-only all through; a missing field
/// diagnosed. A record deeper than the build takes is left to the slow path (the control), and the
/// build is the one that ran for the rest (a counter, not an assumption).
#[test]
fn raw_record_build_matches_the_call_by_call_build() {
    use super::{native, record_raw};
    use serde_json::json;
    let h = Host::new().unwrap();
    let lua = &h.lua;
    let sample = json!({
        "id": 4294967295u64, "name": "Zu\u{f1}iga \u{1F409} Dragon", "level": 51, "hp_fraction": 0.5, "dead": false, "gone": null,
        "position": {"x": 1.5, "y": -2.0, "z": 1e9}, "empty": {}, "tags": ["a", null, "c"],
        "ref": {"$ref": "entity", "key": "7", "generation": 3, "incarnation": 9},
        "nested": [{"deep": {"x": 1}}, {"deep": {"x": 2}}],
    });
    let before = record_raw::builds();
    let fast = native::record(lua, "eqoa.entities.Entity", &sample).unwrap();
    assert_eq!(record_raw::builds(), before + 1, "the one-call build made it");
    let slow = native::record_slow(lua, "eqoa.entities.Entity", &sample).unwrap();
    let (a, b) = (read_back(&fast, &sample), read_back(&slow, &sample));
    assert_eq!(a, b);
    assert_eq!(a["name"], "Zu\u{f1}iga \u{1F409} Dragon");
    assert_eq!(a["position"]["z"], 1e9);
    assert_eq!(a["tags"], json!(["a", null, "c"]));
    assert_eq!(a["nested"][1]["deep"]["x"], 2.0);
    assert!(a.get("gone").is_none(), "a null field is absent");
    lua.globals().set("rec", fast.clone()).unwrap();
    h.execute(
        r#"
        assert(getmetatable(rec) == false, 'the metatable is hidden')
        assert(type(rec.ref) == 'userdata')
        assert(not pcall(function() rec.id = 1 end), 'a record is read-only')
        assert(not pcall(function() rec.position.x = 1 end), 'so is a nested one')
        assert(not pcall(function() rec.nested[1].deep.x = 5 end))
        assert(rec.tags[3] == 'c' and #rec.tags >= 1)
        local ok = pcall(function() return rec.misspelled end)
        assert(ok and rec.misspelled == nil)
        assert(rec.position.misspelled == nil)
    "#,
    )
    .unwrap();
    let logs = h.drain_logs().unwrap();
    assert!(logs.iter().any(|s| s.contains("eqoa.entities.Entity.misspelled")), "a missing field is diagnosed: {logs:?}");
    // control: past the depth the build takes, the call-by-call build makes it and the counter stays
    let mut deep = json!({"x": 1.0});
    for _ in 0..16 {
        deep = json!({ "in": deep });
    }
    let before = record_raw::builds();
    let value = native::record(lua, "eqoa.Deep", &deep).unwrap();
    assert_eq!(record_raw::builds(), before, "too deep for the one-call build");
    assert_eq!(read_back(&value, &deep), deep, "and still built");
    // arrays of plain values
    let numbers = json!([1, 2.5, "x", true]);
    let before = record_raw::builds();
    let value = native::record(lua, "eqoa.List", &numbers).unwrap();
    assert_eq!(record_raw::builds(), before + 1);
    assert_eq!(read_back(&value, &numbers), json!([1.0, 2.5, "x", true]));
}

/// A change event's `before` and `after` records are built when a handler reads them, not on delivery:
/// a handler reading only `after` leaves `before` unbuilt (the build counter, the control), and a
/// later read gives the same values as an eager build, the same table each time, read-only, an
/// absent record (`entity.added`) nil, a missing field still diagnosed.
#[test]
fn change_event_before_and_after_are_built_only_when_read() {
    use super::{
        native_core::{CORE, Core},
        native_events, record_raw,
    };
    use serde_json::json;
    CORE.with(|c| *c.borrow_mut() = Core::default());
    let h = Host::new().unwrap();
    h.execute("seen = {}; eqoa.events.on('entity.changed', function(e) seen[#seen+1] = e; read_id = e.data.after.id end); eqoa.events.on('entity.added', function(e) added = e end)").unwrap();
    let rec = |name: &str, x: f64| json!({"id": 7, "name": name, "position": {"x": x, "y": 0.0, "z": 2.0}, "ref": {"$ref": "entity", "key": "7", "generation": 1, "incarnation": 1}});
    let emit = |name: &str, before: serde_json::Value, after: serde_json::Value| {
        CORE.with(|c| c.borrow_mut().emit(name, json!({"ref": after["ref"], "before": before, "after": after, "changed": ["position"]})))
    };
    emit("entity.changed", rec("Rat", 1.0), rec("Rat", 5.0));
    let before = record_raw::builds();
    native_events::dispatch(&h).unwrap();
    let delivered = record_raw::builds() - before;
    h.execute("assert(read_id == 7)").unwrap();
    // read-only handler of `after`: the data's other fields and `after` are built, `before` is not
    let read_before = record_raw::builds();
    h.execute("assert(seen[1].data.before.name == 'Rat' and seen[1].data.before.position.x == 1)").unwrap();
    assert_eq!(record_raw::builds() - read_before, 1, "reading `before` built it then (control: it was {delivered} builds to deliver)");
    // `ref`, `changed` and the nested records of `after` are built on first read too (a field at a
    // time): read them once before counting
    h.execute("local d = seen[1].data; assert(type(d.ref) == 'userdata' and d.changed[1] == 'position' and d.after.position.x == 5)").unwrap();
    let again = record_raw::builds();
    h.execute(
        r#"
        local d = seen[1].data
        assert(rawequal(d.before, d.before) and rawequal(d.after, d.after), 'the same table each time')
        assert(d.after.position.x == 5 and d.changed[1] == 'position' and type(d.ref) == 'userdata')
        assert(not pcall(function() d.before = 1 end), 'read-only')
        assert(not pcall(function() d.before.name = 'x' end), 'and its record too')
        assert(d.nope == nil)
    "#,
    )
    .unwrap();
    assert_eq!(record_raw::builds(), again, "no further builds for what was built");
    assert!(h.drain_logs().unwrap().iter().any(|s| s.contains("data.nope")), "a missing field is still diagnosed");
    // an entity.added has no `before`: nil, and no diagnostic for it
    emit("entity.added", json!(null), rec("Fox", 3.0));
    native_events::dispatch(&h).unwrap();
    h.execute("assert(added.data.before == nil and added.data.after.name == 'Fox')").unwrap();
    assert!(!h.drain_logs().unwrap().iter().any(|s| s.contains("data.before")), "an absent `before` is not a missing field");
}
