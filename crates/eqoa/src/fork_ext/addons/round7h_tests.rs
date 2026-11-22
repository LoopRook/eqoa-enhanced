// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round 7H: the last corrections before the `eqoa/1` freeze (the D2 spec pass's list), the parts that need
//! no `Play`: descriptors that were missing, a record without its fields, the names a failure reports, the
//! sequence numbers of a core-side loss, and the low items. The `Play`-driven items are in
//! `fork_ext/round7h_play_tests.rs`.
use super::host::Host;
use super::native_core::{self, CORE, Core};
use super::native_events;

fn status(host: &Host, path: &str) -> String {
    host.lua
        .load(format!("return eqoa.capabilities.get('{path}').status"))
        .eval()
        .unwrap()
}

fn mod_host(id: &str) -> Host {
    use super::discovery::{DiscoveredAddon, EntryMode};
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bundled-mods/eqoa_stats_box");
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

/// Item 8: the two constant tables are described like `eqoa.items.slots`, not only their keys.
#[test]
fn round7h_8_constant_tables_have_their_own_descriptor() {
    let host = Host::new().unwrap();
    for (table, key) in [("eqoa.entities.status", "poisoned"), ("eqoa.abilities.scope", "target")] {
        assert_eq!(status(&host, table), "supported", "{table}");
        assert_eq!(status(&host, &format!("{table}.{key}")), "supported", "{table}.{key}");
    }
    // the control the descriptor is held to: the table the mod reads is there
    host.execute("assert(eqoa.entities.status.poisoned == 4 and eqoa.abilities.scope.target == 1)").unwrap();
}

/// Item 9: each draw-list method is in the registry as `ImDrawList.<Name>`, read from the methods the draw
/// list really has: every described name is callable on a live draw list, and nothing else is described.
#[test]
fn round7h_9_draw_list_methods_are_described_from_their_registration() {
    let mut context = dear_imgui_rs::Context::create();
    context.set_ini_filename::<std::path::PathBuf>(None).unwrap();
    context.io_mut().set_display_size([256., 192.]);
    context.io_mut().set_delta_time(1. / 60.);
    context.font_atlas().try_claim_legacy_renderer().unwrap().build();
    let host = Host::new().unwrap();
    let bridge = super::bridge::Bridge::install(&host).unwrap();
    host.execute(
        r#"
        described = {}
        for _, c in ipairs(eqoa.capabilities.list('ImDrawList')) do
            if c.path ~= 'ImDrawList' then described[#described + 1] = c.path:sub(#'ImDrawList.' + 1) end
            assert(c.status == 'supported' or c.path == 'ImDrawList', c.path .. ' is ' .. c.status)
        end
        table.sort(described)
        local want = {'AddCircle', 'AddCircleFilled', 'AddImage', 'AddImageRounded', 'AddLine', 'AddNgon',
            'AddNgonFilled', 'AddRect', 'AddRectFilled', 'AddText', 'AddTriangle', 'AddTriangleFilled',
            'PopClipRect', 'PushClipRect'}
        assert(#described == #want, 'described: ' .. table.concat(described, ' '))
        for i, name in ipairs(want) do assert(described[i] == name, 'described: ' .. table.concat(described, ' ')) end
        assert(eqoa.capabilities.get('ImDrawList.AddQuad').status == 'unknown')
    "#,
    )
    .unwrap();
    context.frame();
    bridge
        .invoke(|| {
            host.execute(
                r#"
                local dl = ImGui.GetBackgroundDrawList()
                for _, name in ipairs(described) do
                    assert(type(dl[name]) == 'function', name .. ' is described but not a method')
                end
            "#,
            )
        })
        .unwrap();
    drop(context.render_legacy());
}

/// Item 10: `eqoa.options.button` returns an `eqoa.options.Button` whose fields are declared.
#[test]
fn round7h_10_option_button_record_has_declared_fields() {
    let host = mod_host("round7h_button");
    super::native_tests::install_field_check(&host).unwrap();
    host.execute(
        r#"
        local b, err = eqoa.options.button({id = 'export', label = 'Export'}, function() end)
        assert(err == nil and b ~= nil)
        fields('options.Button', b)
        assert(b.id == 'export' and b.label == 'Export' and type(b.item_id) == 'number')
    "#,
    )
    .unwrap();
}

/// Item 12: a load that fails reports, in `error.capability`, a name the registry has.
#[test]
fn round7h_12_a_failed_load_names_a_capability_that_exists() {
    CORE.with(|c| *c.borrow_mut() = Core::default());
    let host = Host::new().unwrap();
    host.execute(
        "named = {}; eqoa.events.on('resource.failed', function(e) named[#named + 1] = e.data.kind .. '=' .. e.data.error.capability end)",
    )
    .unwrap();
    for kind in ["texture", "font", "sound"] {
        native_events::resource(&host.lua, kind, mlua::Value::Nil, Some("could not be loaded")).unwrap();
    }
    native_events::dispatch(&host).unwrap();
    host.execute(
        r#"
        assert(#named == 3, 'failures delivered: ' .. #named)
        for _, entry in ipairs(named) do
            local cap = entry:match('=(.*)$')
            assert(eqoa.capabilities.get(cap).status ~= 'unknown', entry .. ' names a capability that does not exist')
        end
    "#,
    )
    .unwrap();
}

/// Item 13: a loss inside the core (pending deltas discarded before they became events) has no event
/// numbers: `events.dropped` gives the count and the stream, and no made-up `first_seq`/`last_seq`.
#[test]
fn round7h_13_core_side_loss_reports_no_made_up_sequence_numbers() {
    CORE.with(|c| *c.borrow_mut() = Core::default());
    native_core::discard_deferred();
    let host = Host::new().unwrap();
    host.execute("drops = {}; eqoa.events.on('events.dropped', function(e) drops[#drops + 1] = e.data end)").unwrap();
    native_core::observe_text("old-world line", [1.; 4]);
    CORE.with(|c| c.borrow_mut().reset("zone"));
    native_core::flush_deferred();
    native_events::dispatch(&host).unwrap();
    host.execute(
        r#"
        local d
        for _, x in ipairs(drops) do if x.streams[1] == 'source.generation' then d = x end end
        assert(d ~= nil, 'the core-side loss was not reported')
        assert(d.count == 1, 'count ' .. tostring(d.count))
        assert(d.first_seq == nil and d.last_seq == nil,
            'made-up sequence numbers: ' .. tostring(d.first_seq) .. '..' .. tostring(d.last_seq))
    "#,
    )
    .unwrap();
}

/// Low item: `eqoa.input.bind` called again for the same id replaces the callback: one press, one call.
#[test]
fn round7h_low_input_bind_twice_runs_one_callback_per_press() {
    CORE.with(|c| *c.borrow_mut() = Core::default());
    crate::fork_ext::bindpage::reset_in_memory();
    let host = Host::with_test_capabilities(&[]).unwrap();
    host.execute(
        r#"
        first, second = 0, 0
        assert(eqoa.input.bind({id = 'go', label = 'Go', default_key = '9'}, function() first = first + 1 end))
        assert(eqoa.input.bind({id = 'go', label = 'Go', default_key = '9'}, function() second = second + 1 end))
    "#,
    )
    .unwrap();
    CORE.with(|c| {
        c.borrow_mut().emit(
            "input.key",
            serde_json::json!({"key":"9","down":true,"repeat":false,"synthetic":false,"text":""}),
        )
    });
    native_events::dispatch(&host).unwrap();
    host.execute("assert(first == 0 and second == 1, 'first ' .. first .. ', second ' .. second)").unwrap();
    crate::fork_ext::bindpage::reset_in_memory();
}

/// Low item: the functions that take no arguments include `chat.get_channel` and `combat.get_counters` (the
/// list named `chat.channel` and `combat.counters`, which do not exist, so those two ignored arguments).
#[test]
fn round7h_low_no_argument_functions_refuse_an_argument() {
    let host = Host::new().unwrap();
    host.execute(
        r#"
        for _, name in ipairs({'chat.get_channel', 'combat.get_counters', 'group.get'}) do
            local m, f = name:match('^(.-)%.(.*)$')
            local v, err = eqoa[m][f](1)
            assert(v == nil and err and err.code == 'invalid_argument', name .. ' took an argument: ' .. tostring(err and err.code))
        end
    "#,
    )
    .unwrap();
}

/// Low item (the freeze guard's question): the legacy aliases are described because they are bound.
#[test]
fn round7h_low_legacy_alias_status_follows_its_binding() {
    let host = Host::new().unwrap();
    for alias in ["ChatHistory", "WeaponSets"] {
        let bound: bool = host.lua.load(format!("return eqoa.{alias} ~= nil")).eval().unwrap();
        let described = status(&host, &format!("eqoa.{alias}"));
        assert_eq!(
            (bound, described.as_str()),
            (true, "supported"),
            "eqoa.{alias}: bound {bound}, described {described}"
        );
    }
}

/// Fix round 1, item 5: the hidden session's idle timeout the spec quotes, 15 seconds (scope `own`), pinned
/// through a clock seam: still open after 14.9 s without use, ended at 15 s.
#[test]
fn round7h_fix5_hidden_session_idle_timeout_is_15_seconds() {
    use super::window_takeover::{self as wt, TakeoverKind, TakeoverScope};
    use std::time::Duration;
    wt::reset_all_for_test();
    wt::takeover(1, "idle_pin", TakeoverKind::Loot, TakeoverScope::Own).unwrap();
    assert!(wt::activate_hidden_for_test(1, TakeoverKind::Loot, 9001));
    assert!(wt::is_hidden_active(TakeoverKind::Loot));
    wt::age_hidden_session_for_test(TakeoverKind::Loot, Duration::from_millis(14_900));
    wt::sync_sessions_with(|_, _, _| Ok(()), |_, _| true);
    assert!(wt::is_hidden_active(TakeoverKind::Loot), "ended before 15 s without use");
    wt::age_hidden_session_for_test(TakeoverKind::Loot, Duration::from_secs(15));
    wt::sync_sessions_with(|_, _, _| Ok(()), |_, _| true);
    assert!(!wt::is_hidden_active(TakeoverKind::Loot), "still open after 15 s without use");
    wt::reset_all_for_test();
}
