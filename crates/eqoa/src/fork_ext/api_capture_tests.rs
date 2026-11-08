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
