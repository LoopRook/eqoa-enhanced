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
