use super::*;
use crate::fork_ext::addons::{
    host::Host,
    native_core::{CORE, Core},
    native_events,
};
fn ready() -> Play {
    CORE.with(|c| *c.borrow_mut() = Core::default());
    let mut p = super::tests::play();
    let _ = p.loading.wait.take();
    p.hud = Some((
        crate::ui::Vui::new(Default::default(), Default::default(), vec![]),
        Default::default(),
        crate::hud::Hud::new(),
    ));
    p
}
#[test]
fn native_state_preserves_signed_stats_full_strings_ids_and_unbounded_lists() {
    let mut p = ready();
    p.player.name = "A full native character name beyond twenty four bytes".into();
    p.target = 0xfeed_beef;
    p.entities.slots = vec![None; 40];
    for i in 0..40 {
        p.entities.slots[i] = Some(net::entity::Entity {
            id: if i == 39 { p.target } else { 0x100 + i as u32 },
            p19: if i == 0 { 0 } else { 255 },
            creature: Some(net::entity::Creature {
                name: format!("Creature {i}"),
                level: 4,
                target: p.target,
                ..Default::default()
            }),
            ..Default::default()
        });
    }
    p.stats.stat[7] = -8;
    p.stats.stat[8] = 200;
    CORE.with(|c| c.borrow_mut().stats_received = true);
    p.forge_snapshot(None);
    let host = Host::new().unwrap();
    host.execute(r#"local p=assert(eqoa.player.get());assert(p.name=='A full native character name beyond twenty four bytes');assert(p.hp.current==-8);assert(p.target_id==0xfeedbeef);assert(#eqoa.entities.list()==40);assert(eqoa.entities.list()[1].hp_fraction==nil);assert(eqoa.target.get().id==0xfeedbeef);old=eqoa.target.get();assert(eqoa.entities.get(old.ref).ref==old.ref)"#).unwrap();
    CORE.with(|c| c.borrow_mut().reset("zone"));
    p.forge_snapshot(None);
    host.execute("local v,e=eqoa.entities.get(old.ref);assert(v==nil and e.code=='stale_handle');assert(old.name=='Creature 39')").unwrap();
}
#[test]
fn native_effects_and_group_use_actual_channels_and_current_roster() {
    let mut p = ready();
    let host = Host::new().unwrap();
    p.forge_snapshot(None);
    host.execute("local v,e=eqoa.effects.list();assert(v==nil and e.code=='unavailable')")
        .unwrap();
    let event = net::session::Event::State {
        channel: 0x43,
        data: 0u32.to_le_bytes().to_vec(),
    };
    let capture = begin_game_event(&p, &event);
    net::dump::read_effects(&mut p.effects, &0u32.to_le_bytes());
    end_game_event(&p, capture);
    host.execute("assert(#eqoa.effects.list()==0)").unwrap();
    p.effects.count = 1;
    p.effects.icon[0] = 123;
    p.effects.name[0] = "An actual native effect".encode_utf16().collect();
    p.group.in_group = true;
    p.group.leader = true;
    p.group.members = vec![
        crate::group::Member {
            id: 0x200,
            name: "New leader".into(),
            ..Default::default()
        },
        crate::group::Member {
            id: 0x100,
            name: "Self".into(),
            ..Default::default()
        },
    ];
    let snapshot = p.forge_snapshot(None);
    assert_eq!(snapshot.effects.count, 1);
    assert_eq!(snapshot.effects.effects[0].icon_ref, 123);
    assert!(!snapshot.group.is_leader);
    host.execute("local e=eqoa.effects.list()[1];assert(e.index==1 and e.icon==123 and e.name=='An actual native effect');local g=eqoa.group.get();assert(g.self_leader==false and g.members[1].hp_fraction==nil and g.members[1].position==nil)").unwrap();
}
#[test]
fn native_logical_chat_burst_keeps_source_order_and_full_text() {
    let mut p = ready();
    p.forge_snapshot(None);
    let host = Host::new().unwrap();
    host.execute("messages={};eqoa.events.on('chat.message',function(e) messages[#messages+1]=e.data.text end)").unwrap();
    for i in 0..25 {
        p.add_chat(format!("logical line {i}"), [1.0; 4]);
    }
    native_events::dispatch(&host).unwrap();
    host.execute("assert(#messages==25);assert(messages[1]=='logical line 0' and messages[25]=='logical line 24');local history=eqoa.chat.history();assert(#history==25 and history[1].text==messages[1]);local e=eqoa.chat.history(2);assert(e[1].seq<e[2].seq)").unwrap();
    assert_eq!(
        p.chat.len(),
        8,
        "negative control: the old presentation ring loses 17 lines"
    );
}
#[test]
fn native_controller_snapshot_uses_live_report_bytes_and_rotation_is_consumed() {
    crate::fork_ext::addons::input::reset_input_state();
    let mut report = crate::input::Report {
        buttons: 1 << 4,
        sticks: [0.25, -0.5, 0.75, 0.0],
    };
    crate::fork_ext::prepare_controls(&mut report, true, true);
    let snapshot = crate::fork_ext::addons::input::live_snapshot();
    assert_eq!(snapshot.button_bitfield, !(1 << 4));
    assert_eq!(snapshot.right_analog, (report.data()[4], report.data()[5]));
    let host = Host::new().unwrap();
    host.execute("local p=eqoa.input.get().pad;assert(p.buttons[1]=='up' and p.right.x==0.25 and p.right.y==-0.5 and p.left.x==0.75 and p.raw_left.y==128)").unwrap();
    crate::fork_ext::prepare_controls(&mut report, false, false);
    host.execute("assert(eqoa.input.get().pad==nil)").unwrap();
}

#[test]
fn native_legacy_chat_dispatch_waits_for_game_tick_and_world_readiness() {
    let mut p = ready();
    p.forge_snapshot(None);
    crate::fork_ext::addons::chat::take_outgoing_chat();
    crate::fork_ext::addons::chat::queue_outgoing_chat("queued".into());
    begin_iteration(&mut p, false);
    assert_eq!(
        crate::fork_ext::addons::chat::take_outgoing_chat(),
        vec!["queued"]
    );
    crate::fork_ext::addons::chat::queue_outgoing_chat("discard on world gap".into());
    p.session.self_id = None;
    begin_iteration(&mut p, true);
    assert!(crate::fork_ext::addons::chat::take_outgoing_chat().is_empty());
}

#[test]
fn native_cooldowns_capture_whole_record_and_lockout_edges_with_stable_ids() {
    let mut p = ready();
    p.forge_snapshot(None);
    let h = Host::new().unwrap();
    crate::fork_ext::addons::ability::install_ability_list_adapter(&h).unwrap();
    h.lua
        .globals()
        .set(
            "forge_abilities",
            h.registered_modules()["frontiers_forge.ability_list"].clone(),
        )
        .unwrap();
    h.snapshot.set(p.forge_snapshot(None));
    h.execute("forge_edges={};forge_abilities.Acquire('native-regression');for _,name in ipairs({'OnLockoutStart','OnLockoutEnd'}) do forge_abilities.On('native-regression',name,function(e) forge_edges[#forge_edges+1]={name=name,key=e.display_index,lockout=e.lockout_ms} end) end").unwrap();
    h.execute("edges={};for _,name in ipairs({'cooldown.started','cooldown.updated','cooldown.ended'}) do eqoa.events.on(name,function(e) edges[#edges+1]={name=e.name,id=e.data.ability_id,key=e.data.display_key,lockout=e.data.lockout,ref=e.data.ability} end) end").unwrap();
    let a = net::dump::Ability {
        id: i64::MAX,
        display_index: 17,
        valid: 1,
        lockout: 5000,
        ..Default::default()
    };
    let event = net::session::Event::AbilityAdd {
        token: 1,
        flag: 0,
        ability: Box::new(a.clone()),
    };
    let capture = begin_game_event(&p, &event);
    p.abilities.insert(a.id, a);
    end_game_event(&p, capture);
    for raw in [2500, 0] {
        let e = net::session::Event::AbilityLockout {
            index: 17,
            token: 1,
            lockout: raw,
        };
        let capture = begin_game_event(&p, &e);
        p.abilities.get_mut(&i64::MAX).unwrap().lockout = raw as i64;
        end_game_event(&p, capture);
    }
    // Reindex after receipt must not change the captured edge identity/key.
    p.abilities.get_mut(&i64::MAX).unwrap().display_index = 22;
    p.forge_snapshot(None);
    h.snapshot.set(p.forge_snapshot(None));
    h.execute("forge_abilities.Update();assert(#forge_edges==2);assert(forge_edges[1].name=='OnLockoutStart' and forge_edges[1].key==17 and forge_edges[1].lockout==5000);assert(forge_edges[2].name=='OnLockoutEnd' and forge_edges[2].key==17 and forge_edges[2].lockout==0)").unwrap();
    native_events::dispatch(&h).unwrap();
    h.execute("assert(#edges==3);assert(edges[1].name=='cooldown.started' and edges[1].id=='9223372036854775807' and edges[1].key==17 and edges[1].lockout==5);assert(edges[2].name=='cooldown.updated' and edges[2].lockout==2.5);assert(edges[3].name=='cooldown.ended' and edges[3].lockout==0);assert(eqoa.abilities.get(edges[1].ref).display_key==22)").unwrap();
}

#[test]
fn native_combat_and_forge_share_signed_amount_rgb_snapshot_and_consumer_lifecycle() {
    let mut p = ready();
    p.forge_snapshot(None);
    let h = Host::new().unwrap();
    crate::fork_ext::addons::combat::install_combat_adapter(&h).unwrap();
    h.lua
        .globals()
        .set(
            "forge_combat",
            h.registered_modules()["frontiers_forge.combat"].clone(),
        )
        .unwrap();
    h.snapshot.set(p.forge_snapshot(None));
    h.execute("hits={};forge_combat.Acquire('source');forge_combat.On('source','OnDamageDealt',function(e) hits[#hits+1]=e end)").unwrap();
    p.fork_combat_event(0x100, i32::MIN, 0x200, Some([0, 0, 0]));
    let snapshot = p.forge_snapshot(None);
    assert_eq!(snapshot.combat.count, 1);
    assert_eq!(snapshot.combat.events[0].color, 0x01000000);
    assert_eq!(snapshot.combat.events[0].amount, i32::MIN);
    h.snapshot.set(snapshot);
    h.execute("forge_combat.Update();assert(#hits==1 and hits[1].amount==-2147483648 and hits[1].color==0x01000000 and not hits[1].is_heal);assert(forge_combat.UninstallHook());assert(not forge_combat.IsHookInstalled());assert(not forge_combat.GetHookInfo().installed)").unwrap();
    p.fork_combat_event(0x100, -1, 0x200, None);
    h.snapshot.set(p.forge_snapshot(None));
    h.execute("forge_combat.Update();assert(#hits==1);assert(forge_combat.InstallHook());assert(forge_combat.IsHookInstalled());forge_combat.Update();assert(#hits==1)").unwrap();
    native_events::clear(&h);
    p.fork_combat_event(0x100, -2, 0x200, None);
    h.snapshot.set(p.forge_snapshot(None));
    h.execute("forge_combat.Update();assert(#hits==1);assert(not forge_combat.IsHookInstalled());assert(not forge_combat.GetHookInfo().installed)").unwrap();
}

#[test]
fn native_chat_context_uses_group_opcode_and_restores_after_capture_is_revoked() {
    let mut p = ready();
    p.forge_snapshot(None);
    let h = Host::new().unwrap();
    let event = net::session::Event::GroupChat {
        from: 0x200,
        text: "group line".into(),
    };
    let capture = begin_game_event(&p, &event);
    p.add_chat("group line".into(), [1.0; 4]);
    drop(capture); // Models an early native exit before end_game_event.
    p.add_chat("local line".into(), [1.0; 4]);
    h.execute("local history=eqoa.chat.history();assert(history[1].channel=='group');assert(history[2].channel=='system');assert(history[1].sender==nil and history[1].outgoing==nil)").unwrap();
}

#[test]
fn native_presentation_counter_includes_pregame_without_counting_extra_game_steps() {
    let mut p = ready();
    let mut report = crate::input::Report::default();
    crate::fork_ext::prepare_controls(&mut report, false, false);
    CORE.with(|c| {
        let c = c.borrow();
        assert_eq!(c.frame, 1);
        assert_eq!(c.tick, 0);
    });
    begin_iteration(&mut p, false);
    begin_iteration(&mut p, true);
    CORE.with(|c| {
        let c = c.borrow();
        assert_eq!(c.frame, 1);
        assert_eq!(c.tick, 1);
    });
    crate::fork_ext::prepare_controls(&mut report, true, true);
    CORE.with(|c| {
        let c = c.borrow();
        assert_eq!(c.frame, 2);
        assert_eq!(c.tick, 1);
    });
}

#[test]
fn review_fix_h1_forge_projection_requires_a_positive_creature_level() {
    let mut p = ready();
    p.entities.slots = vec![None; 4];
    for (slot, creature) in [
        (1, None),
        (
            2,
            Some(net::entity::Creature {
                level: 0,
                ..Default::default()
            }),
        ),
        (
            3,
            Some(net::entity::Creature {
                level: 7,
                ..Default::default()
            }),
        ),
    ] {
        p.entities.slots[slot] = Some(net::entity::Entity {
            id: 0x100 + slot as u32,
            creature,
            ..Default::default()
        });
    }
    let snapshot = p.forge_snapshot(None);
    assert!(
        !snapshot.entities.entities[1].is_populated,
        "ID alone is not a populated Forge record"
    );
    assert!(!snapshot.entities.entities[2].is_populated);
    assert!(snapshot.entities.entities[3].is_populated);
}

#[test]
fn review_fix_h2_forge_target_is_low_byte_while_native_target_is_full_u32() {
    let mut p = ready();
    p.entities.slots = vec![
        None,
        Some(net::entity::Entity {
            id: 0x1234,
            creature: Some(net::entity::Creature {
                level: 8,
                target: 0xfeed_beef,
                ..Default::default()
            }),
            ..Default::default()
        }),
    ];
    let snapshot = p.forge_snapshot(None);
    assert_eq!(snapshot.entities.entities[1].target_id, 0xef);
    let host = Host::new().unwrap();
    host.execute("assert(eqoa.entities.find(0x1234).target_id==0xfeedbeef)")
        .unwrap();
}

#[test]
fn review_fix_h3_entity_names_match_case_insensitive_literal_utf8_and_optional_substrings() {
    let mut p = ready();
    p.entities.slots = ["Assistant Recruiter", "?LITE Wolf", "Wolf.*"]
        .into_iter()
        .enumerate()
        .map(|(i, name)| {
            Some(net::entity::Entity {
                id: 0x100 + i as u32,
                creature: Some(net::entity::Creature {
                    name: name.into(),
                    level: 1,
                    ..Default::default()
                }),
                ..Default::default()
            })
        })
        .collect();
    p.forge_snapshot(None);
    let h = Host::new().unwrap();
    h.execute(
        r#"
        assert(#eqoa.entities.named('assistant recruiter')==1)
        assert(#eqoa.entities.named('?Lite wolf',true)==1)
        assert(#eqoa.entities.named('wolf')==0)
        assert(#eqoa.entities.named('wOlF',false)==2)
        assert(#eqoa.entities.named('.*',false)==1)
        local value,err=eqoa.entities.named('wolf',1)
        assert(value==nil and err.code=='invalid_argument')
    "#,
    )
    .unwrap();
}

#[test]
fn review_fix_m1_accepted_chat_retries_modal_and_diagnoses_world_loss() {
    use crate::fork_ext::addons::chat;
    let mut p = ready();
    chat::take_outgoing_chat();
    let host = Host::new().unwrap();
    chat::install_chat_adapter(&host).unwrap();
    host.snapshot.set(p.forge_snapshot(None));
    host.lua
        .globals()
        .set(
            "chat",
            host.registered_modules()["frontiers_forge.chat"].clone(),
        )
        .unwrap();
    host.execute("assert(chat.SendChatText('hello'));assert(chat.IsSendPending())")
        .unwrap();
    p.mode = 60;
    begin_iteration(&mut p, true);
    host.execute("assert(chat.IsSendPending(),'accepted modal send must remain queued')")
        .unwrap();
    p.mode = 0;
    begin_iteration(&mut p, true);
    host.execute("assert(not chat.IsSendPending());assert(chat.SendChatText('world gap'))")
        .unwrap();
    p.session.self_id = None;
    begin_iteration(&mut p, true);
    host.execute("assert(not chat.IsSendPending())").unwrap();
    let logs = host.drain_logs().unwrap();
    assert!(
        logs.iter()
            .any(|line| line.contains("frontiers_forge.chat.SendChatText")
                && line.contains("cancelled")),
        "world-loss cancellation must be visible: {logs:?}"
    );
}

#[test]
fn review_fix_m6_iteration_readiness_does_not_build_a_native_view() {
    let mut p = ready();
    super::api_capture::take_view_builds();
    begin_iteration(&mut p, true);
    assert_eq!(
        super::api_capture::take_view_builds(),
        0,
        "readiness must not serialize state"
    );
    p.session.self_id = None;
    begin_iteration(&mut p, true);
    assert_eq!(super::api_capture::take_view_builds(), 0);
}

#[test]
fn performance_busy_world_zero_addons_frame_and_packet_allocation_bound() {
    use crate::fork_ext::addons::{input, perf_alloc::measure};
    let mut p = ready();
    crate::fork_ext::addons::native_core::fixture_demand(false);
    p.entities.slots = (0..200)
        .map(|i| {
            Some(net::entity::Entity {
                id: 0x100 + i,
                pos: [i as f32, 0., 1.],
                p19: 255,
                creature: Some(net::entity::Creature {
                    name: format!("Busy creature {i}"),
                    level: 20,
                    target: 0x1234,
                    ..Default::default()
                }),
                ..Default::default()
            })
        })
        .collect();
    p.group.in_group = true;
    p.group.members = (0..6)
        .map(|i| crate::group::Member {
            id: 0x100 + i,
            name: format!("Group {i}"),
            active: true,
            ..Default::default()
        })
        .collect();
    p.abilities = (0..200)
        .map(|i| {
            (
                i,
                net::dump::Ability {
                    id: i,
                    name: format!("Ability {i}"),
                    desc: "Long description. ".repeat(100),
                    display_index: i,
                    slot: -1,
                    ..Default::default()
                },
            )
        })
        .collect();
    p.forge_snapshot(None);
    let mut report = crate::input::Report::default();
    let (_, frame_time, frame_allocs, frame_bytes) = measure(|| {
        for _ in 0..300 {
            begin_iteration(&mut p, true);
            health_redraw(&mut p, true, false);
            std::hint::black_box(health_hidden(&p));
            std::hint::black_box(p.forge_snapshot(None));
            input::observe_report(&mut report, true, true);
            std::hint::black_box(input::live_snapshot());
        }
    });
    p.mode = 13;
    let (_, menu_time, menu_allocs, menu_bytes) = measure(|| {
        for _ in 0..300 {
            begin_iteration(&mut p, true);
            health_redraw(&mut p, true, false);
            std::hint::black_box(health_hidden(&p));
            std::hint::black_box(p.forge_snapshot(None));
            input::observe_report(&mut report, true, true);
        }
    });
    eprintln!(
        "PERF zero-addons menu_frame_us={:.3} allocs/menu_frame={:.1} bytes/menu_frame={:.1}",
        menu_time.as_secs_f64() * 1e6 / 300.,
        menu_allocs as f64 / 300.,
        menu_bytes as f64 / 300.
    );
    let event = net::session::Event::State {
        channel: 1,
        data: vec![],
    };
    let (_, event_time, event_allocs, event_bytes) = measure(|| {
        for i in 0..600 {
            let change = begin_game_event(&p, &event);
            p.entities.slots[1].as_mut().unwrap().pos[0] = i as f32;
            end_game_event(&p, change);
        }
    });
    let (_, chat_time, chat_allocs, chat_bytes) = measure(|| {
        for _ in 0..600 {
            observe_text("busy server line", [1.; 4]);
        }
    });
    eprintln!(
        "PERF zero-addons frame_us={:.3} allocs/frame={:.1} bytes/frame={:.1}; event_us={:.3} allocs/event={:.1} bytes/event={:.1}; chat_us={:.3} allocs/chat={:.1} bytes/chat={:.1}",
        frame_time.as_secs_f64() * 1e6 / 300.,
        frame_allocs as f64 / 300.,
        frame_bytes as f64 / 300.,
        event_time.as_secs_f64() * 1e6 / 600.,
        event_allocs as f64 / 600.,
        event_bytes as f64 / 600.,
        chat_time.as_secs_f64() * 1e6 / 600.,
        chat_allocs as f64 / 600.,
        chat_bytes as f64 / 600.
    );
    assert!(
        menu_allocs <= 300,
        "zero-addon menus must not allocate snapshots: {menu_allocs}"
    );
    assert!(
        frame_allocs <= 300,
        "zero enabled addons must not allocate per-frame state: {frame_allocs}"
    );
    assert!(
        event_allocs <= 600,
        "zero enabled addons must not allocate packet state: {event_allocs}"
    );
    assert_eq!(
        chat_allocs, 0,
        "zero enabled addons must not allocate a chat journal"
    );
}

#[test]
fn performance_busy_world_enabled_uses_incremental_typed_packet_updates() {
    use crate::fork_ext::addons::{native_core, perf_alloc::measure};
    let mut p = ready();
    native_core::fixture_demand(false);
    p.entities.slots = (0..200)
        .map(|i| {
            Some(net::entity::Entity {
                id: 0x100 + i,
                pos: [i as f32, 0., 1.],
                p19: 255,
                creature: Some(net::entity::Creature {
                    name: format!("Busy creature {i}"),
                    level: 20,
                    ..Default::default()
                }),
                ..Default::default()
            })
        })
        .collect();
    p.abilities = (0..200)
        .map(|i| {
            (
                i,
                net::dump::Ability {
                    id: i,
                    name: format!("Ability {i}"),
                    desc: "Long description. ".repeat(100),
                    display_index: i,
                    slot: -1,
                    ..Default::default()
                },
            )
        })
        .collect();
    p.group.in_group = true;
    p.group.members = (0..6)
        .map(|i| crate::group::Member {
            id: 0x100 + i,
            name: format!("Group {i}"),
            active: true,
            ..Default::default()
        })
        .collect();
    let h = Host::new().unwrap();
    p.forge_snapshot(None);
    h.execute("seen=0;last=-1;eqoa.events.on('entity.changed',function(e) seen=seen+1;last=e.data.after.position.x end)").unwrap();
    super::api_capture::take_view_builds();
    let (_, frames, allocs, bytes) = measure(|| {
        for _ in 0..300 {
            std::hint::black_box(p.forge_snapshot(None));
        }
    });
    let event = net::session::Event::State {
        channel: 1,
        data: vec![],
    };
    let (_, events, event_allocs, event_bytes) = measure(|| {
        for i in 0..600 {
            let change = begin_game_event(&p, &event);
            p.entities.slots[1].as_mut().unwrap().pos[0] = i as f32 + 1000.;
            end_game_event(&p, change);
        }
    });
    assert_eq!(
        super::api_capture::take_view_builds(),
        0,
        "unchanged frames and packet updates must never rebuild a complete view"
    );
    assert!(allocs <= 300, "unchanged enabled frames allocated {allocs}");
    assert!(
        event_allocs <= 600 * 10,
        "typed source deltas must stay bounded: {event_allocs}"
    );
    for _ in 0..3 {
        native_events::dispatch(&h).unwrap();
    }
    h.execute(
        "assert(seen==600 and last==1599);assert(eqoa.entities.find(0x101).position.x==1599)",
    )
    .unwrap();
    let (_, duplicates, duplicate_allocs, _) = measure(|| {
        for _ in 0..600 {
            let change = begin_game_event(&p, &event);
            end_game_event(&p, change);
        }
    });
    assert_eq!(
        duplicate_allocs, 0,
        "unchanged packet data must not be copied"
    );
    eprintln!(
        "PERF enabled frame_us={:.3} allocations/frame={:.3} bytes/frame={:.1}; changed_event_us={:.3} allocations/event={:.3} bytes/event={:.1}; duplicate_event_us={:.3} duplicate_allocations={}",
        frames.as_secs_f64() * 1e6 / 300.,
        allocs as f64 / 300.,
        bytes as f64 / 300.,
        events.as_secs_f64() * 1e6 / 600.,
        event_allocs as f64 / 600.,
        event_bytes as f64 / 600.,
        duplicates.as_secs_f64() * 1e6 / 600.,
        duplicate_allocs
    );
}

#[test]
fn performance_busy_world_disabled_vms_leave_packet_and_frame_capture_quiescent() {
    use crate::fork_ext::addons::{native_core, perf_alloc::measure};
    let mut p = ready();
    native_core::fixture_demand(false);
    native_core::manager_demand(false);
    p.entities.slots = (0..200)
        .map(|i| {
            Some(net::entity::Entity {
                id: 0x100 + i,
                ..Default::default()
            })
        })
        .collect();
    let hosts = (0..16).map(|_| Host::new().unwrap()).collect::<Vec<_>>();
    for host in &hosts {
        native_events::clear(host);
    }
    assert!(!native_core::capture_requested());
    p.forge_snapshot(None);
    let event = net::session::Event::State {
        channel: 1,
        data: vec![],
    };
    let mut report = crate::input::Report::default();
    crate::fork_ext::prepare_controls(&mut report, true, true);
    let (_, elapsed, allocs, bytes) = measure(|| {
        for frame in 0..300 {
            p.mode = if frame % 2 == 0 { 0 } else { 13 };
            begin_iteration(&mut p, true);
            health_redraw(&mut p, true, false);
            std::hint::black_box(p.forge_snapshot(None));
            report.buttons = frame as u16;
            crate::fork_ext::prepare_controls(&mut report, true, true);
            for _ in 0..2 {
                let capture = begin_game_event(&p, &event);
                end_game_event(&p, capture);
            }
            observe_text("ignored while disabled", [1.; 4]);
        }
    });
    assert_eq!(
        allocs, 0,
        "retained disabled VMs must not allocate producer work"
    );
    eprintln!(
        "PERF 16-disabled-VMs combined_frame_us={:.3} allocations={} bytes={}",
        elapsed.as_secs_f64() * 1e6 / 300.,
        allocs,
        bytes
    );
}

#[test]
fn native_incremental_packets_preserve_group_target_and_hotbar_records() {
    use crate::fork_ext::addons::native_core;
    let mut p = ready();
    native_core::fixture_demand(false);
    p.entities.slots = vec![Some(net::entity::Entity {
        id: 0x101,
        pos: [10., 20., 30.],
        creature: Some(net::entity::Creature {
            name: "Recruiter".into(),
            level: 20,
            ..Default::default()
        }),
        ..Default::default()
    })];
    let h = Host::new().unwrap();
    p.forge_snapshot(None);
    h.execute("groups=0;eqoa.events.on('group.changed',function() groups=groups+1 end)")
        .unwrap();
    let event = net::session::Event::GroupRoster {
        ids: vec![0x101],
        names: vec!["Recruiter".into()],
    };
    let capture = begin_game_event(&p, &event);
    p.group.in_group = true;
    p.group
        .roster(&[0x101], &["Recruiter".into()], p.self_id32());
    end_game_event(&p, capture);
    h.execute(
        "local g=eqoa.group.get();assert(g.members[1].position.x==10 and g.members[1].world_id==0)",
    )
    .unwrap();
    let event = net::session::Event::State {
        channel: 0,
        data: vec![],
    };
    let capture = begin_game_event(&p, &event);
    p.entities.slots[0].as_mut().unwrap().pos[0] = 55.;
    end_game_event(&p, capture);
    h.execute("assert(eqoa.group.get().members[1].position.x==55)")
        .unwrap();
    p.target = 0x101;
    let event = net::session::Event::TargetInfo {
        disposition: 4,
        b: 0,
        c: 0,
        title: String::new(),
        guild: String::new(),
    };
    let capture = begin_game_event(&p, &event);
    p.target_info = Some(4);
    end_game_event(&p, capture);
    p.forge_snapshot(None);
    h.execute("assert(eqoa.target.get().disposition==4)")
        .unwrap();
    let a = net::dump::Ability {
        id: 0x1234,
        display_index: 12,
        slot: 0,
        valid: 1,
        name: "A real ability".into(),
        ..Default::default()
    };
    let event = net::session::Event::AbilityAdd {
        token: 0,
        flag: 0,
        ability: Box::new(a.clone()),
    };
    let capture = begin_game_event(&p, &event);
    p.abilities.insert(a.id, a);
    p.hud_menus
        .build(&p.abilities, &p.dump.inventory, &p.gear, p.hud_bars());
    end_game_event(&p, capture);
    h.execute("ability=eqoa.abilities.find('4660');assert(eqoa.hotbar.list()[1].slots[1].ability==ability.ref)").unwrap();
    let event = net::session::Event::AbilitySlot {
        token: 0,
        index: 12,
        slot: 1,
    };
    let capture = begin_game_event(&p, &event);
    p.abilities.get_mut(&0x1234).unwrap().slot = 1;
    p.hud_menus
        .build(&p.abilities, &p.dump.inventory, &p.gear, p.hud_bars());
    end_game_event(&p, capture);
    h.execute("assert(eqoa.abilities.find('4660').ref==ability.ref);assert(eqoa.hotbar.list()[1].slots[2].ability==ability.ref)").unwrap();
    native_core::fixture_demand(true);
    p.forge_snapshot(None);
    native_core::fixture_demand(false);
    h.execute("assert(eqoa.hotbar.list()[1].slots[2].ability==ability.ref)")
        .unwrap();
    native_events::dispatch(&h).unwrap();
    h.execute("assert(groups==2)").unwrap();
    p.entities.slots.push(Some(net::entity::Entity {
        id: p.self_id32().unwrap(),
        creature: Some(net::entity::Creature::default()),
        ..Default::default()
    }));
    p.forge_snapshot(None);
    let event = net::session::Event::State {
        channel: 1,
        data: vec![],
    };
    let capture = begin_game_event(&p, &event);
    p.entities.slots[1]
        .as_mut()
        .unwrap()
        .creature
        .as_mut()
        .unwrap()
        .p263 = 1;
    end_game_event(&p, capture);
    h.execute("assert(eqoa.player.get().fighting==true)")
        .unwrap();
    let event = net::session::Event::State {
        channel: 0,
        data: vec![],
    };
    let capture = begin_game_event(&p, &event);
    p.entities.slots[0] = None;
    end_game_event(&p, capture);
    h.execute("local m=eqoa.group.get().members[1];assert(m.entity==nil and m.position==nil and m.world_id==nil)")
        .unwrap();
    let event = net::session::Event::State {
        channel: 1,
        data: vec![],
    };
    let capture = begin_game_event(&p, &event);
    p.entities.slots[1] = None;
    end_game_event(&p, capture);
    h.execute("assert(eqoa.player.get().fighting==nil)")
        .unwrap();
}

#[test]
fn native_quiescent_metadata_matches_requested_snapshots_in_all_menu_modes() {
    use crate::fork_ext::addons::native_core;
    let mut p = ready();
    native_core::manager_demand(false);
    for mode in [0, 1, 7, 13, 30, 33, 34, 36, 60] {
        p.mode = mode;
        native_core::fixture_demand(true);
        let full = p.forge_snapshot(None);
        native_core::fixture_demand(false);
        let minimal = p.forge_snapshot(None);
        assert_eq!(
            (
                minimal.in_game,
                minimal.start_menu,
                minimal.ui_mode,
                minimal.ui_busy
            ),
            (full.in_game, full.start_menu, full.ui_mode, full.ui_busy),
            "mode {mode}"
        );
    }
}

fn perf_review_walker() -> crate::walker::Walker {
    fn record(kind: u16, children: u32, body: Vec<u8>) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend(kind.to_le_bytes());
        out.extend(1i16.to_le_bytes());
        out.extend((body.len() as i32).to_le_bytes());
        out.extend(children.to_le_bytes());
        out.extend(body);
        out
    }
    let tree = record(0x8230, 0, vec![0; 8]);
    let base = record(0x8200, 1, tree);
    let root = record(0x8000, 1, base);
    let mut lib = esf::Library::new();
    let file = lib.add("synthetic-perf-world", esf::Container::parse_records(root));
    let mut world = vi::world::World::load(&lib, file).unwrap();
    world.leaves.push(0);
    world.zones.push(Default::default());
    crate::walker::Walker::arrive(
        world,
        crate::walker::Spawn {
            pos: [0.; 3],
            heading: 0.,
            placing: crate::walker::Placing::AsGiven,
        },
    )
}

#[test]
fn perf_review_m1_motion_tick_and_non_motion_delivery() {
    use crate::fork_ext::addons::native_core;
    let mut p = ready();
    native_core::fixture_demand(false);
    let h = Host::new().unwrap();
    let mut w = perf_review_walker();
    p.forge_snapshot(Some(&w));
    h.execute("moves=0;eqoa.events.on('player.changed',function() moves=moves+1 end)")
        .unwrap();
    CORE.with(|c| c.borrow_mut().events.clear());
    for i in 1..=120 {
        w.cam.walk.phys.position.x = vi::fpu::F::new(i as f32);
        p.forge_snapshot(Some(&w));
    }
    let count = || {
        CORE.with(|c| {
            c.borrow()
                .events
                .iter()
                .filter(|e| e.name == "player.changed")
                .count()
        })
    };
    assert_eq!(count(), 1, "motion must not emit at rendered-frame rate");
    h.execute("assert(eqoa.player.get().position.x==120)")
        .unwrap();
    p.auto_attack = true;
    p.forge_snapshot(Some(&w));
    assert_eq!(
        count(),
        2,
        "non-motion changes must not wait for the next tick"
    );
    native_core::iteration(true);
    w.cam.walk.phys.position.x = vi::fpu::F::new(121.);
    p.forge_snapshot(Some(&w));
    assert_eq!(count(), 3);
    w.cam.walk.phys.position.x = vi::fpu::F::new(122.);
    p.forge_snapshot(Some(&w));
    native_core::iteration(true);
    p.forge_snapshot(Some(&w));
    assert_eq!(
        count(),
        4,
        "a stopped player's coalesced final position must flush"
    );
}

#[test]
fn perf_review_m1_other_vm_events_do_not_evict_forge_edges_or_hits() {
    use crate::fork_ext::addons::{ability, combat, native_core};
    let mut p = ready();
    native_core::fixture_demand(false);
    let subscriber = Host::new().unwrap();
    p.forge_snapshot(None);
    subscriber
        .execute("eqoa.events.on('player.changed',function() end)")
        .unwrap();
    let forge = Host::new().unwrap();
    ability::install_ability_list_adapter(&forge).unwrap();
    combat::install_combat_adapter(&forge).unwrap();
    forge.snapshot.set(p.forge_snapshot(None));
    forge
        .lua
        .globals()
        .set(
            "abilities",
            forge.registered_modules()["frontiers_forge.ability_list"].clone(),
        )
        .unwrap();
    forge
        .lua
        .globals()
        .set(
            "combat",
            forge.registered_modules()["frontiers_forge.combat"].clone(),
        )
        .unwrap();
    forge.execute("edges=0;hits=0;abilities.Acquire('consumer');abilities.On('consumer','OnLockoutStart',function() edges=edges+1 end);combat.Acquire('consumer');combat.On('consumer','OnDamageDealt',function() hits=hits+1 end)").unwrap();
    native_core::emit(
        "cooldown.started",
        serde_json::json!({"display_key":17,"lockout":5}),
    );
    native_core::observe_hit(0x100, 0x200, -17, None);
    native_core::flush_deferred();
    for _ in 0..3000 {
        native_core::emit(
            "player.changed",
            serde_json::json!({"before":{"name":"x".repeat(2048)},"after":{"name":"y".repeat(2048)}}),
        );
    }
    forge.execute("abilities.Update();combat.Update();assert(edges==1 and hits==1);assert(abilities.GetDroppedCount()==0)").unwrap();
}

#[test]
fn performance_busy_world_perf_review_m3_moving_player_bound() {
    use crate::fork_ext::addons::{native_core, perf_alloc::measure};
    let mut p = ready();
    native_core::fixture_demand(false);
    p.entities.slots = (0..200)
        .map(|i| {
            Some(net::entity::Entity {
                id: 0x100 + i,
                creature: Some(net::entity::Creature {
                    name: format!("Creature {i}"),
                    level: 20,
                    ..Default::default()
                }),
                ..Default::default()
            })
        })
        .collect();
    p.group.in_group = true;
    p.group.members = (0..6)
        .map(|i| crate::group::Member {
            id: 0x100 + i,
            name: format!("Group {i}"),
            active: true,
            ..Default::default()
        })
        .collect();
    p.abilities = (0..200)
        .map(|i| {
            (
                i,
                net::dump::Ability {
                    id: i,
                    desc: "Long description. ".repeat(100),
                    ..Default::default()
                },
            )
        })
        .collect();
    let h = Host::new().unwrap();
    let mut w = perf_review_walker();
    p.forge_snapshot(Some(&w));
    h.execute("eqoa.events.on('player.changed',function() end)")
        .unwrap();
    super::api_capture::take_view_builds();
    let (_, elapsed, allocs, bytes) = measure(|| {
        for frame in 1..=300 {
            native_core::iteration(frame % 4 == 0);
            w.cam.walk.phys.position.x = vi::fpu::F::new(ee::mul(frame as f32, 0.5));
            w.cam.walk.committed.position = w.cam.walk.phys.position;
            let snap = p.forge_snapshot(Some(&w));
            assert_eq!(snap.player.position[0], ee::mul(frame as f32, 0.5));
            h.snapshot.set(snap);
        }
    });
    eprintln!(
        "PERF moving subscribed frame_us={:.3} allocations/frame={:.3} bytes/frame={:.1}",
        elapsed.as_secs_f64() * 1e6 / 300.,
        allocs as f64 / 300.,
        bytes as f64 / 300.
    );
    assert_eq!(super::api_capture::take_view_builds(), 0);
    assert!(allocs <= 300 * 1000, "moving path allocated {allocs}");
    h.execute("assert(eqoa.player.get().position.x==150)")
        .unwrap();
}
