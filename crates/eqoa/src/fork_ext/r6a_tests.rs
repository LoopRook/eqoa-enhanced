// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round 6 Part A integration tests built from Play/protocol-shaped state.
use super::*;
use crate::play::v3;
use crate::fork_ext::addons::{
    host::Host,
    icons::{DecodedIcon, IconProvider, set_custom_icon_provider},
    native_core::{self, CORE, Core},
    native_events,
};
use crate::dialog::Key;
use net::dump::{Item, ItemList, NetItem};
use net::session::Event;
use net::stream::Writer;

fn ready() -> Play {
    CORE.with(|c| *c.borrow_mut() = Core::default());
    native_core::fixture_demand(true);
    let mut p = super::super::tests::play();
    let _ = p.loading.wait.take();
    p.hud = Some((
        crate::ui::Vui::new(Default::default(), Default::default(), vec![]),
        Default::default(),
        crate::hud::Hud::new(),
    ));
    p
}

fn forge_host(tag: &str) -> Host {
    let root = std::env::temp_dir().join(format!(
        "eqoa_r6a_{tag}_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let entry = root.join("main.lua");
    std::fs::write(&entry, "return true").unwrap();
    let addon = crate::fork_ext::addons::discovery::DiscoveredAddon {
        id: format!("r6a_{tag}"),
        name: format!("R6A {tag}"),
        entry,
        script_root: root.clone(),
        package_root: root.clone(),
        module_dirs: vec![root.clone()],
        resource_dirs: vec![],
        api: Some("eqoa/1".to_string()),
        entry_mode: Default::default(),
        requested_capabilities: vec![],
        extensions: Default::default(),
        initially_enabled: true,
        mods_root: Some(root),
    };
    Host::for_addon(&addon).unwrap()
}

fn frame(p: &Play, w: Option<&crate::walker::Walker>, h: &Host) {
    h.snapshot.set(p.forge_snapshot(w));
    native_events::dispatch(h).unwrap();
}

fn apply(p: &mut Play, w: &mut crate::walker::Walker, event: Event) {
    let capture = begin_game_event(p, &event);
    match event {
        Event::Opcode { op: 0x007C, data } => p.quests.recv_add(&data),
        Event::Opcode { op: 0x007D, data } => p.quests.recv_remove(&data),
        Event::Opcode { op: op @ (0x0016 | 0x0018), data } => {
            p.npcw_opcode(op, &data);
        }
        Event::Opcode { op: 0x124D, data } => {
            p.bank_opcode(0x124D, &data);
        }
        Event::Opcode { op: 0x1402, data } => {
            p.cm_msgs.push(data);
        }
        Event::WorldEntry { player, dump, .. } => {
            p.player = player;
            p.dump = dump;
            p.take_abilities();
        }
        Event::Teleport { .. } => {
            let _ = w;
        }
        other => panic!("unexpected event in r6a_tests: {other:?}"),
    }
    end_game_event(p, capture);
}

fn quest_packet(index: u32, title: &str) -> Vec<u8> {
    let mut w = Writer::default();
    w.u32(index);
    w.wide(title);
    w.data
}

#[test]
fn round6a_quests_dump_add_remove_stale_and_zoning() {
    let mut p = ready();
    let mut w = Play::bundled_ui_walker();
    let h = Host::new().unwrap();
    h.execute(
        r#"
        qlog = {}
        eqoa.events.on('quests.changed', function(e)
            qlog[#qlog + 1] = { count = #e.data.after, changed = e.data.changed[1] }
        end)
        "#,
    )
    .unwrap();

    // 1. Zone-in dump delivers initial quest titles
    let mut dump = Box::<net::dump::MemoryDump>::default();
    dump.quests = vec!["Bat Eyes".into(), "The Militia".into()];
    let player = net::session::Player {
        world: 1,
        ..Default::default()
    };
    apply(
        &mut p,
        &mut w,
        Event::WorldEntry {
            self_id: 100,
            player,
            dump,
        },
    );
    frame(&p, Some(&w), &h);
    h.execute(
        r#"
        local list, err = eqoa.quests.list()
        assert(err == nil and #list == 2)
        assert(list[1].index == 1 and list[1].title == 'Bat Eyes')
        assert(list[2].index == 2 and list[2].title == 'The Militia')
        local q1, e1 = eqoa.quests.get(list[1].ref)
        assert(e1 == nil and q1.title == 'Bat Eyes')
        old_ref = list[1].ref
        qlog = {}
        "#,
    )
    .unwrap();

    // 2. 0x007C adds a quest title -> event fires once, old QuestRef is stale
    apply(
        &mut p,
        &mut w,
        Event::Opcode {
            op: 0x007C,
            data: quest_packet(2, "Goblin Ears"),
        },
    );
    frame(&p, Some(&w), &h);
    h.execute(
        r#"
        assert(#qlog == 1 and qlog[1].count == 3 and qlog[1].changed == 'records')
        local stale, serr = eqoa.quests.get(old_ref)
        assert(stale == nil and serr.code == 'stale_handle')
        local list = eqoa.quests.list()
        assert(#list == 3 and list[3].title == 'Goblin Ears')
        old_ref = list[1].ref
        qlog = {}
        "#,
    )
    .unwrap();

    // 3. 0x007D removes quest 0 -> compacts and invalidates refs
    let mut rem = Writer::default();
    rem.u32(1).signed(0);
    apply(
        &mut p,
        &mut w,
        Event::Opcode {
            op: 0x007D,
            data: rem.data,
        },
    );
    frame(&p, Some(&w), &h);
    h.execute(
        r#"
        assert(#qlog == 1 and qlog[1].count == 2)
        local stale, serr = eqoa.quests.get(old_ref)
        assert(stale == nil and serr.code == 'stale_handle')
        local list = eqoa.quests.list()
        assert(list[1].title == 'The Militia' and list[2].title == 'Goblin Ears')
        saved_ref = list[1].ref
        "#,
    )
    .unwrap();

    // 4. Zoning reset -> new generation invalidates previous QuestRef
    let mut next_dump = Box::<net::dump::MemoryDump>::default();
    next_dump.quests = vec!["Fresh Trail".into()];
    let mut next_player = net::session::Player {
        world: 2,
        ..Default::default()
    };
    next_player.world = 2;
    apply(
        &mut p,
        &mut w,
        Event::WorldEntry {
            self_id: 100,
            player: next_player,
            dump: next_dump,
        },
    );
    frame(&p, Some(&w), &h);
    h.execute(
        r#"
        local stale, serr = eqoa.quests.get(saved_ref)
        assert(stale == nil and serr.code == 'stale_handle')
        local list = eqoa.quests.list()
        assert(#list == 1 and list[1].title == 'Fresh Trail')
        "#,
    )
    .unwrap();
}

#[test]
fn round6a_mastery_uninitialized_dump_and_reply_subops() {
    let mut p = ready();
    let mut w = Play::bundled_ui_walker();
    let h = forge_host("mastery");
    let mods = h.registered_modules();
    h.lua
        .globals()
        .set("ff_player", mods.get("frontiers_forge.player").unwrap().clone())
        .unwrap();
    h.execute(
        r#"
        mlog = {}
        eqoa.events.on('mastery.changed', function(e)
            mlog[#mlog + 1] = e.data
        end)
        "#,
    )
    .unwrap();
    frame(&p, Some(&w), &h);

    // 1. Uninitialized: Mastery counters are nil, skills() unavailable, selected() nil, Forge CMs nil
    h.execute(
        r#"
        local m, err = eqoa.mastery.get()
        assert(err == nil and m ~= nil)
        assert(m.available == nil and m.spent == nil and m.experience_share == nil and m.progress_raw == nil)
        assert(m.pending == false and #m.stats == 29 and #m.bonuses == 7)
        local pm = eqoa.player.get().mastery
        assert(pm ~= nil and pm.available == nil and pm.experience_share == nil)
        local sk, sk_err = eqoa.mastery.skills()
        assert(sk == nil and sk_err.code == 'unavailable')
        local node, n_err = eqoa.mastery.selected()
        assert(node == nil and n_err == nil)
        assert(ff_player.GetCMs() == nil and ff_player.GetCMsSpent() == nil and ff_player.GetCMPct() == nil)
        mlog = {}
        "#,
    )
    .unwrap();

    // 2. Sub-op 1: XP share only -> experience_share = 0.4, spent/available/progress still nil
    let vui = crate::pointer::testing::vui();
    let mut null_r = crate::pointer::testing::NullRaster;
    let mut op1 = Writer::default();
    op1.u8(1).u32(7).u32(40);
    apply(
        &mut p,
        &mut w,
        Event::Opcode {
            op: 0x1402,
            data: op1.data,
        },
    );
    p.run_class_mastery(&vui, &mut null_r);
    frame(&p, Some(&w), &h);
    h.execute(
        r#"
        assert(#mlog == 1)
        local m = eqoa.mastery.get()
        assert(math.abs(m.experience_share - 0.4) < 1e-6)
        assert(m.available == nil and m.spent == nil and m.progress_raw == nil)
        assert(ff_player.GetCMPct() == 40 and ff_player.GetCMs() == nil)
        mlog = {}
        "#,
    )
    .unwrap();

    // 3. Sub-op 3: available=5, spent=12 -> spent stays 12 (not 17!)
    let mut op3 = Writer::default();
    op3.u8(3).u32(5).u32(12);
    apply(
        &mut p,
        &mut w,
        Event::Opcode {
            op: 0x1402,
            data: op3.data,
        },
    );
    p.run_class_mastery(&vui, &mut null_r);
    frame(&p, Some(&w), &h);
    h.execute(
        r#"
        assert(#mlog == 1)
        local m = eqoa.mastery.get()
        assert(m.available == 5 and m.spent == 12 and m.progress_raw == nil)
        assert(ff_player.GetCMs() == 5 and ff_player.GetCMsSpent() == 12)
        mlog = {}
        "#,
    )
    .unwrap();

    // 4. Sub-op 4: raw progress
    let mut op4 = Writer::default();
    op4.u8(4).u32(123456);
    apply(
        &mut p,
        &mut w,
        Event::Opcode {
            op: 0x1402,
            data: op4.data,
        },
    );
    p.run_class_mastery(&vui, &mut null_r);
    frame(&p, Some(&w), &h);
    h.execute("assert(#mlog == 1 and eqoa.mastery.get().progress_raw == 123456); mlog = {}")
        .unwrap();

    // 5. Sub-op 8: skill list
    let mut op8 = Writer::default();
    op8.u8(8).u32(7).u32(1);
    op8.signed(100)
        .u8(1)
        .u8(1)
        .signed(10)
        .signed(0)
        .u8(2)
        .signed(3)
        .wide("Bountiful Spirit");
    apply(
        &mut p,
        &mut w,
        Event::Opcode {
            op: 0x1402,
            data: op8.data,
        },
    );
    p.run_class_mastery(&vui, &mut null_r);
    frame(&p, Some(&w), &h);
    h.execute(
        r#"
        local skills, err = eqoa.mastery.skills()
        assert(err == nil and #skills == 1)
        assert(skills[1].id == 10 and skills[1].rank_key == 100 and skills[1].parent_key == 0)
        assert(skills[1].name == 'Bountiful Spirit' and skills[1].category == 2 and skills[1].cost == 3)
        assert(skills[1].known == true and skills[1].owned == true and skills[1].master_class == false)
        "#,
    )
    .unwrap();

    // 6. Sub-op 9: inspected node (CM window open in mode 40 so run_class_mastery retains node)
    p.mode = 40;
    p.cm_wnd.open(&vui, &mut null_r, &p.cm);
    let mut op9 = Writer::default();
    op9.u8(9)
        .u32(7)
        .signed(555)
        .u8(1)
        .u8(1)
        .signed(10)
        .signed(0)
        .u8(2)
        .u8(50)
        .signed(99)
        .signed(6);
    for min in [10, 20, 30, 40, 50, 60, 70] {
        op9.signed(min);
    }
    op9.wide("Bountiful Spirit")
        .signed(7)
        .wide("Bountiful Spirit Title")
        .wide("Raises wisdom.")
        .signed(0)
        .signed(0)
        .signed(0);
    apply(
        &mut p,
        &mut w,
        Event::Opcode {
            op: 0x1402,
            data: op9.data,
        },
    );
    p.run_class_mastery(&vui, &mut null_r);
    frame(&p, Some(&w), &h);
    h.execute(
        r#"
        local n, err = eqoa.mastery.selected()
        assert(err == nil and n ~= nil)
        assert(n.id == 555 and n.skill_id == 10 and n.cost == 3)
        assert(n.name == 'Bountiful Spirit' and n.title == 'Bountiful Spirit Title' and n.text == 'Raises wisdom.')
        assert(n.level_required == 50 and n.points_spent_required == 6 and n.minimum_stats[1] == 10 and n.minimum_stats[7] == 70)
        "#,
    )
    .unwrap();

    // 7. Sub-op 5 & Sub-op 6 & Sub-op 10: stats/bonuses/points update
    let mut op5 = Writer::default();
    op5.u8(5).u32(7).u32(10).u32(2).u32(15).u32(1.2_f32.to_bits());
    for i in 0..29 {
        op5.signed(if i == 4 { 25 } else { 0 });
    }
    for i in 0..7 {
        op5.signed(if i == 2 { 9 } else { 0 });
    }
    op5.u32(0); // 0 flags
    apply(
        &mut p,
        &mut w,
        Event::Opcode {
            op: 0x1402,
            data: op5.data,
        },
    );
    p.run_class_mastery(&vui, &mut null_r);
    frame(&p, Some(&w), &h);
    h.execute(
        r#"
        local m = eqoa.mastery.get()
        assert(m.available == 2 and m.spent == 15 and m.stats[5] == 25 and m.bonuses[3] == 9)
        "#,
    )
    .unwrap();

    // 8. WorldEntry dump initializes all counters even on zero-count char
    let mut dump = Box::<net::dump::MemoryDump>::default();
    dump.mastery.p41c = 99;
    dump.mastery.x0c = 0;
    dump.mastery.x10 = 0;
    dump.mastery.x08 = 0;
    dump.mastery.x14 = 0;
    apply(
        &mut p,
        &mut w,
        Event::WorldEntry {
            self_id: 100,
            player: net::session::Player::default(),
            dump,
        },
    );
    frame(&p, Some(&w), &h);
    h.execute(
        r#"
        local m = eqoa.mastery.get()
        assert(m.available == 0 and m.spent == 0 and m.experience_share == 0.0 and m.progress_raw == 0)
        assert(ff_player.GetCMs() == 0 and ff_player.GetCMsSpent() == 0 and ff_player.GetCMPct() == 0)
        "#,
    )
    .unwrap();
}

#[test]
fn round6a_camera_third_and_first_person_and_tick_coalescing() {
    let p = ready();
    native_core::fixture_demand(false);
    native_core::manager_demand(true);
    let mut w = Play::bundled_ui_walker();
    let h = forge_host("camera");
    let mods = h.registered_modules();
    h.lua
        .globals()
        .set("ff_cam", mods.get("frontiers_forge.camera").unwrap().clone())
        .unwrap();
    h.execute("ff_util = require('frontiers_forge.util')").unwrap();

    // Third person camera state
    w.cam.camera_mode = 3;
    w.cam.cur.orbit.x = vi::fpu::F::new(-0.5); // raw negative preserved in Forge camera
    w.cam.cur.orbit.y = vi::fpu::F::new(0.25);
    w.cam.cur.dist = vi::fpu::F::new(12.0);
    w.eye.position = v3([10.0, 22.0, 30.0]);
    w.eye.direction = v3([0.0, 0.0, -1.0]); // looking north -> compass 0

    frame(&p, Some(&w), &h);
    h.execute(
        r#"
        clog = {}
        eqoa.events.on('camera.changed', function(e) clog[#clog + 1] = e.data end)
        local c, err = eqoa.camera.get()
        assert(err == nil and c.mode == 3 and math.abs(c.distance - 12.0) < 1e-4)
        assert(math.abs(c.position.x - 10.0) < 1e-4 and math.abs(c.position.y - 22.0) < 1e-4 and math.abs(c.position.z - 30.0) < 1e-4)
        local cpos = ff_cam.GetCoordinates()
        assert(math.abs(cpos.x - 10.0) < 1e-4 and math.abs(cpos.y - 22.0) < 1e-4 and math.abs(cpos.z - 30.0) < 1e-4)
        assert(c.heading >= 0 and c.heading < 2 * math.pi)
        assert(math.abs(c.pitch - 0.25) < 1e-4)
        local compass, cerr = eqoa.camera.compass()
        assert(cerr == nil and math.abs(compass) < 1e-5)
        -- Forge retains raw orbit.x while Util.GetCompass uses final eye direction
        assert(math.abs(ff_cam.GetFacingRadians() - (-0.5)) < 1e-4)
        assert(math.abs(ff_util.GetCompassRadians() - compass) < 1e-5)
        assert(math.abs(ff_util.GetCompassDegrees() - 0.0) < 1e-3)
        "#,
    )
    .unwrap();

    // Two render frames in the SAME tick must coalesce to 1 camera.changed on the tick
    native_core::iteration(true);
    w.eye.position = v3([11.0, 22.0, 30.0]);
    frame(&p, Some(&w), &h);
    native_core::iteration(false);
    w.eye.position = v3([12.0, 22.0, 30.0]);
    frame(&p, Some(&w), &h);
    h.execute("assert(#clog == 1, 'same tick emits once immediately; second render buffers')")
        .unwrap();

    // Next 30 Hz game tick flushes the buffered intra-tick change once
    native_core::iteration(true);
    frame(&p, Some(&w), &h);
    h.execute("assert(#clog == 2 and math.abs(clog[2].after.position.x - 12.0) < 1e-4)")
        .unwrap();

    // First person (mode 0, dist 0, eye equals player pos)
    native_core::iteration(true);
    w.cam.camera_mode = 0;
    w.cam.cur.dist = vi::fpu::F::new(0.0);
    w.eye.position = w.cam.walk.phys.position;
    w.eye.direction = v3([-1.0, 0.0, 0.0]); // looking west -> atan2(1, 0) == pi/2
    frame(&p, Some(&w), &h);
    h.execute(
        r#"
        local c = eqoa.camera.get()
        assert(c.mode == 0 and c.distance == 0)
        local comp = eqoa.camera.compass()
        assert(math.abs(comp - math.pi / 2) < 1e-4)
        assert(math.abs(ff_util.GetCompassDegrees() - 90.0) < 1e-2)
        "#,
    )
    .unwrap();
}

#[test]
fn round6a_windows_modes_dialogs_hints_and_outside_world() {
    // Outside world: mode == "none", native_mode == 0, busy == false
    let mut p = super::super::tests::play();
    p.session.self_id = None;
    CORE.with(|c| *c.borrow_mut() = Core::default());
    let h = forge_host("windows");
    let mods = h.registered_modules();
    h.lua
        .globals()
        .set("ff_wnd", mods.get("frontiers_forge.window").unwrap().clone())
        .unwrap();
    h.execute(
        r#"
        local w, err = eqoa.windows.get()
        assert(err == nil and w.mode == 'none' and w.native_mode == 0 and w.busy == false)
        assert(w.start_menu_open == false and w.any_window_open == false and w.active == nil and #w.windows == 0)
        assert(eqoa.windows.find('start_menu') == nil)
        assert(eqoa.windows.format_hint('triangle', 'Exit') == '|tExit')
        local parsed = eqoa.windows.parse_hint('|-|x Accept  |t Exit  Plain')
        assert(#parsed == 2 or #parsed == 3)
        assert(parsed[1].button == 'cross' and parsed[1].label == 'Accept')
        assert(parsed[2].button == 'triangle' and parsed[2].label == 'Exit  Plain' or parsed[2].button == 'triangle')
        local only_lit = eqoa.windows.parse_hint('Press button')
        assert(#only_lit == 1 and only_lit[1].button == 'text' and only_lit[1].label == 'Press button')
        local bad_fmt, bad_err = eqoa.windows.format_hint('bogus', 'X')
        assert(bad_fmt == nil and bad_err.code == 'invalid_argument')
        assert(ff_wnd.FormatButtonHint('triangle', 'Exit') == '|tExit')
        assert(ff_wnd.FormatButtonHint('bogus', 'Fallback') == 'Fallback')
        local pb, pl = ff_wnd.ParseButtonHint('|tExit')
        assert(pb == 'triangle' and pl == 'Exit')
        "#,
    )
    .unwrap();

    // Enter world
    let mut p = ready();
    let w = Play::bundled_ui_walker();
    let mut lines = vec![String::new(); 250];
    lines[160] = "Exit".into();
    lines[211] = "Select".into();
    p.words = vi::strtab::StringTable::parse(lines.join("\n").as_bytes());
    frame(&p, Some(&w), &h);
    h.execute(
        r#"
        wlog = {}
        eqoa.events.on('windows.changed', function(e) wlog[#wlog + 1] = e.data end)
        local g = eqoa.windows.get()
        assert(g.mode == 'normal' and g.native_mode == 0 and g.busy == false and not g.start_menu_open and not g.any_window_open)
        "#,
    )
    .unwrap();

    // Start menu & Merchant/Bank/Loot modes + stale WindowRef after close
    p.mode = 13;
    frame(&p, Some(&w), &h);
    h.execute(
        r#"
        assert(#wlog == 1)
        local g = eqoa.windows.get()
        assert(g.mode == 'start_menu' and g.native_mode == 13 and g.busy == true and g.start_menu_open == true and g.any_window_open == true)
        local sm = eqoa.windows.find('start_menu')
        assert(sm ~= nil and sm.focused == true and #sm.hints == 2)
        saved_wref = sm.ref
        local resolved, rerr = eqoa.windows.get_window(saved_wref)
        assert(rerr == nil and resolved.kind == 'start_menu')
        wlog = {}
        "#,
    )
    .unwrap();

    for (mode, name) in [(10, "merchant"), (11, "bank"), (1, "loot"), (99, "other")] {
        p.mode = mode;
        frame(&p, Some(&w), &h);
        h.execute(&format!(
            "local g = eqoa.windows.get(); assert(g.mode == '{name}' and g.native_mode == {mode} and g.busy == true and g.windows[1].kind == '{name}')"
        ))
        .unwrap();
    }
    h.execute(
        r#"
        local stale, serr = eqoa.windows.get_window(saved_wref)
        assert(stale == nil and serr.code == 'stale_handle')
        "#,
    )
    .unwrap();

    // Text dialog (mode 9) -> dialog.kind='text', Forge reports IsTextDialogOpen
    p.mode = 9;
    frame(&p, Some(&w), &h);
    h.execute(
        "assert(eqoa.windows.get().mode == 'text_dialog' and eqoa.windows.find('text_dialog').dialog.kind == 'text' and ff_wnd.IsTextDialogOpen() == true)",
    )
    .unwrap();

    // Confirm box (mode 14) -> kind='confirm', text/style/style_id readable, yes_no defaults to No (selection=2)
    p.mbox.prompt(&vi::ui::wide("Delete item?"), 1, 0, 0, 100);
    p.mode = 14;
    frame(&p, Some(&w), &h);
    h.execute(
        r#"
        local c = eqoa.windows.find('confirm')
        assert(c.dialog.kind == 'confirm' and c.dialog.text == 'Delete item?' and c.dialog.style == 'yes_no' and c.dialog.style_id == 1 and c.dialog.selection == 2)
        "#,
    )
    .unwrap();

    // Input box (mode 2): plain text + password=true sets text=nil
    p.mode = 2;
    p.edit.purpose = 0xB0F_0000;
    p.edit.title = vi::ui::wide("Chat");
    p.edit.text = vi::ui::wide("Hello world");
    p.edit.password = false;
    frame(&p, Some(&w), &h);
    h.execute(
        r#"
        local inp = eqoa.windows.find('input')
        assert(inp.dialog.kind == 'input' and inp.dialog.text == 'Hello world' and inp.dialog.chat == true and inp.dialog.password == false)
        "#,
    )
    .unwrap();
    p.edit.password = true;
    p.edit.purpose = 0x1234;
    frame(&p, Some(&w), &h);
    h.execute(
        r#"
        local inp = eqoa.windows.find('input')
        assert(inp.dialog.text == nil and inp.dialog.password == true and inp.dialog.chat == false and inp.dialog.tag == 0x1234)
        "#,
    )
    .unwrap();

    // Map overlay open over normal play
    p.mode = 0;
    p.test_map_search_surface(true);
    frame(&p, Some(&w), &h);
    h.execute(
        r#"
        local g = eqoa.windows.get()
        assert(g.mode == 'normal' and g.native_mode == 0 and g.busy == true and g.any_window_open == true)
        local mw = eqoa.windows.find('map')
        assert(mw ~= nil and mw.focused == true and g.active == mw.ref)
        wlog = {}
        "#,
    )
    .unwrap();

    // Quiet frame emits no extra windows.changed
    frame(&p, Some(&w), &h);
    h.execute("assert(#wlog == 0)").unwrap();
}

struct TestUiProvider;
impl IconProvider for TestUiProvider {
    fn decode_icon(&self, _: u32) -> Option<DecodedIcon> {
        None
    }
    fn decode_ui_texture(&self, tex_id: u32) -> Option<DecodedIcon> {
        if matches!(tex_id, 0x17 | 0xFB | 0x73 | 0x74 | 0x75) {
            Some(DecodedIcon {
                width: 32,
                height: 44,
                rgba: vec![255; 32 * 44 * 4],
            })
        } else {
            None
        }
    }
}

#[test]
fn round6a_forge_icons_merchant_hint_hotbar_items_and_zero_addon() {
    let mut p = ready();
    let w = Play::bundled_ui_walker();
    let mut lines = vec![String::new(); 250];
    lines[160] = "Exit".into();
    p.words = vi::strtab::StringTable::parse(lines.join("\n").as_bytes());
    CORE.with(|c| c.borrow_mut().words = p.words.clone());
    p.npcw.shop.id = Some(7001);
    p.mode = 10;

    // Bind inventory potion to quick slot 1
    let mut item = Item::default();
    item.id = 555;
    item.name = "Healing Vial".into();
    item.p4c = 2;
    item.max_stack = 20;
    p.dump.inventory = ItemList {
        items: vec![NetItem {
            inv_index: 99,
            quantity: 3,
            charges: 0,
            cur_hp: 10,
            equip_slot: -1,
            quick_slot: 0,
            item,
        }],
        ..Default::default()
    };
    p.hud_menus.build(&p.abilities, &p.dump.inventory, &p.gear, p.hud_bars());

    set_custom_icon_provider(Some(Box::new(TestUiProvider)));
    let h = forge_host("forge_icons");
    let mods = h.registered_modules();
    h.lua
        .globals()
        .set("ff_ui", mods.get("frontiers_forge.ui").unwrap().clone())
        .unwrap();
    h.lua
        .globals()
        .set("ff_merc", mods.get("frontiers_forge.merchant").unwrap().clone())
        .unwrap();

    // Attach GPU texture cache when a wgpu adapter is available (headless CI may skip GPU upload)
    let _gpu_bridge = crate::fork_ext::addons::icons::install_test_gpu_for_host(&h);

    frame(&p, Some(&w), &h);
    if _gpu_bridge.is_some() {
        h.execute(
            r#"
            -- eqoa.icons.class and disposition
            local ci, cerr = eqoa.icons.class(0)
            assert(cerr == nil and ci.width == 32 and ci.height == 44 and ci.texture ~= nil)
            local di, derr = eqoa.icons.disposition(0)
            assert(derr == nil and di.texture ~= nil)
            local bad_c, bad_cerr = eqoa.icons.class(99)
            assert(bad_c == nil and bad_cerr.code == 'invalid_argument')

            -- Forge UI class & disposition textures + Draw*Icon
            local t1, w1, h1 = ff_ui.GetClassIconTexture(14)
            assert(t1 ~= nil and w1 == 32 and h1 == 44)
            local t2, w2, h2 = ff_ui.GetDispositionIconTexture(5)
            assert(t2 ~= nil and w2 == 32 and h2 == 44)
            assert(ff_ui.GetDispositionIconTexture(nil) == nil)
            drawn = {}
            ImGui = { Image = function(tex, w, h) drawn[#drawn + 1] = { tex = tex, w = w, h = h } end }
            assert(ff_ui.DrawClassIcon(0, 2) == true and drawn[1].w == 64 and drawn[1].h == 88)
            assert(ff_ui.DrawDispositionIcon(0, 0.5) == true and drawn[2].w == 16 and drawn[2].h == 22)
            "#,
        )
        .unwrap();
    } else {
        eprintln!("SKIP r6a GPU icon upload assertions: no GPU adapter");
    }

    h.execute(
        r#"
        -- Forge merchant GetButtonHint & eqoa MerchantState.button_hint
        assert(ff_merc.GetButtonHint() == '|tExit')
        assert(eqoa.merchant.get().merchant.button_hint == '|tExit')

        -- HotbarSlot.item linked to ItemRef
        local inv = eqoa.inventory.get()
        local bars = eqoa.hotbar.list()
        local item_bar
        for _, b in ipairs(bars) do if b.kind == 'items' then item_bar = b end end
        assert(item_bar ~= nil and item_bar.slots[1].kind == 'item')
        assert(item_bar.slots[1].item == inv.items[1].ref)
        saved_hotbar_item_ref = item_bar.slots[1].item
        local resolved = eqoa.items.get(saved_hotbar_item_ref)
        assert(resolved ~= nil and resolved.name == 'Healing Vial')

        -- Capabilities check (native + Forge additions + known seams/deferrals)
        assert(eqoa.capabilities.get('eqoa.quests.list').status == 'supported')
        assert(eqoa.capabilities.get('eqoa.mastery.get').status == 'supported')
        assert(eqoa.capabilities.get('eqoa.camera.get').status == 'supported')
        assert(eqoa.capabilities.get('eqoa.windows.get').status == 'supported')
        assert(eqoa.capabilities.get('eqoa.hotbar.HotbarSlot.item').status == 'supported')
        assert(eqoa.capabilities.get('eqoa.icons.class').status == 'supported')
        assert(eqoa.capabilities.get('eqoa.icons.disposition').status == 'supported')
        assert(eqoa.capabilities.get('frontiers_forge.player.GetCMs').status == 'supported')
        assert(eqoa.capabilities.get('frontiers_forge.ui.GetClassIconTexture').status == 'supported')
        assert(eqoa.capabilities.get('frontiers_forge.merchant.GetButtonHint').status == 'supported')
        assert(eqoa.capabilities.get('frontiers_forge.camera.GetCoordinates').status == 'supported')
        assert(eqoa.capabilities.get('frontiers_forge.window.FormatButtonHint').status == 'supported')
        assert(eqoa.capabilities.get('eqoa.windows.Window.rows').status == 'supported')
        assert(eqoa.capabilities.get('eqoa.windows.Window.selected_row').status == 'supported')
        assert(eqoa.capabilities.get('eqoa.windows.Window.scroll_top').status == 'supported')
        assert(eqoa.capabilities.get('eqoa.windows.MenuRow.command').status == 'supported')
        assert(eqoa.capabilities.get('eqoa.windows.Dialog.options').status == 'supported')
        assert(eqoa.capabilities.get('eqoa.windows.Dialog.selection').status == 'supported')
        assert(eqoa.capabilities.get('eqoa.textures.formats.jpeg').status == 'not_yet')
        "#,
    )
    .unwrap();

    // Removing inventory item unbinds / invalidates the cached hotbar ItemRef
    p.dump.inventory.items.clear();
    p.hud_menus.build(&p.abilities, &p.dump.inventory, &p.gear, p.hud_bars());
    frame(&p, Some(&w), &h);
    h.execute(
        r#"
        local gone, gerr = eqoa.items.get(saved_hotbar_item_ref)
        assert(gone == nil and gerr.code == 'stale_handle')
        "#,
    )
    .unwrap();
    set_custom_icon_provider(None);

    // Zero-addon path builds zero views and captures zero events
    drop(h);
    native_core::fixture_demand(false);
    native_core::manager_demand(false);
    assert!(!native_core::capture_requested());
    super::take_view_builds();
    let _ = p.forge_snapshot(Some(&w));
    assert_eq!(super::take_view_builds(), 0);
}

#[test]
fn round6a_seam_dialogs_privchat_response_pages_and_confirm_yes_no() {
    let mut p = ready();
    let w = Play::bundled_ui_walker();
    let h = forge_host("seam_dialogs");
    let mut lines = vec![String::new(); 300];
    lines[160] = "Close".into();
    lines[216] = "Back".into();
    lines[244] = "More".into();
    let vui = crate::ui::Vui::new(
        vi::strtab::StringTable::parse(lines.join("\n").as_bytes()).unwrap(),
        Default::default(),
        vec![],
    );

    // 1. PrivChat (mode 9, text_dialog)
    p.mode = 9;
    p.priv_chat.set_text(&vi::ui::wide("Hail, adventurer!"));
    frame(&p, Some(&w), &h);
    h.execute(
        r#"
        local g = eqoa.windows.get()
        assert(g.mode == 'text_dialog')
        local rw = eqoa.windows.find('text_dialog')
        assert(rw.dialog.kind == 'text' and rw.dialog.text == 'Hail, adventurer!')
        assert(#rw.dialog.options == 0 and rw.rows == nil and rw.selected_row == nil)
        "#,
    )
    .unwrap();

    // 2. Response dialog (mode 7, response_dialog) with 14 options -> 2 pages (PAGE=12)
    p.mode = 7;
    p.response.set_dialog(
        &vui,
        crate::dialog::DialogBox {
            id: 42,
            text: vi::ui::wide("Choose your path:"),
            options: (1..=14)
                .map(|i| vi::ui::wide(&format!("Option {i}")))
                .collect(),
        },
    );
    frame(&p, Some(&w), &h);
    h.execute(
        r#"
        wlog = {}
        eqoa.events.on('windows.changed', function(e) wlog[#wlog + 1] = e.data end)
        local rw = eqoa.windows.find('response_dialog')
        assert(rw.dialog.kind == 'text' and rw.dialog.text == 'Choose your path:')
        -- Page 1: Option 1..12, More, Close
        assert(#rw.dialog.options == 14 and rw.dialog.options[13] == 'More' and rw.dialog.options[14] == 'Close',
            (function()
                local p = {}
                for k = 1, 14 do p[#p+1] = string.format('[%d]=%q', k, tostring(rw.dialog.options[k])) end
                return 'opts=' .. #rw.dialog.options .. ' ' .. table.concat(p, ' ')
            end)())
        assert(#rw.rows == 14 and rw.rows[1].text == 'Option 1' and rw.selected_row == 1)
        "#,
    )
    .unwrap();

    // Advance down to "More" (row 13, native index 12) and activate it -> page 2 (options 13, 14, Back, Close)
    for _ in 0..12 {
        p.response.key(&vui, Key::Down, 0);
    }
    p.response.key(&vui, Key::Enter, 0); // opens page 2 and steps sel from 12 to 13 -> visible row 2 on page 2!
    frame(&p, Some(&w), &h);
    h.execute(
        r#"
        assert(#wlog == 1)
        local rw = eqoa.windows.find('response_dialog')
        assert(#rw.dialog.options == 4)
        assert(rw.dialog.options[1] == 'Option 13' and rw.dialog.options[2] == 'Option 14')
        assert(rw.dialog.options[3] == 'Back' and rw.dialog.options[4] == 'Close')
        -- selected_row must be visible 1-based position 2, NOT native sel+1 (14)
        assert(rw.selected_row == 2 and rw.rows[2].text == 'Option 14')
        wlog = {}
        "#,
    )
    .unwrap();

    // 3. MBox yes_no (mode 14, confirm) starts on No (2) -> Left -> Yes (1)
    p.mode = 14;
    p.mbox.prompt(&vi::ui::wide("Destroy item?"), 1, 0, 0, 0);
    frame(&p, Some(&w), &h);
    h.execute(
        r#"
        local mc = eqoa.windows.find('confirm')
        assert(mc.dialog.kind == 'confirm' and mc.dialog.style == 'yes_no' and mc.dialog.selection == 2)
        wlog = {}
        "#,
    )
    .unwrap();
    let mut cues = Vec::new();
    p.mbox.key(Key::Left, &mut cues);
    frame(&p, Some(&w), &h);
    h.execute(
        r#"
        assert(#wlog == 1)
        local mc = eqoa.windows.find('confirm')
        assert(mc.dialog.selection == 1)
        "#,
    )
    .unwrap();
}

#[test]
fn round6a_seam_loot_bank_questlog_and_start_menu_rows() {
    let mut p = ready();
    let w = Play::bundled_ui_walker();
    let h = forge_host("seam_windows");
    let mut lines = vec![String::new(); 500];
    lines[117] = "TAKE ALL".into();
    lines[118] = "LEAVE ALL".into();
    lines[160] = "Exit".into();
    lines[436] = "Usable".into();
    lines[437] = "Equipment".into();
    lines[438] = "Misc".into();
    let vui = crate::ui::Vui::new(
        vi::strtab::StringTable::parse(lines.join("\n").as_bytes()).unwrap(),
        Default::default(),
        vec![],
    );
    let mut null_r = crate::pointer::testing::NullRaster;
    p.words = Some(vui.words.clone());

    // 1. Loot window (mode 1) via real 0x0018 corpse + NpcWindows::loot_lists()
    let encode_net_item = |id: i64, inv_index: u32, name: &str| {
        let mut iw = Writer::default();
        iw.signed(1).signed(10).signed(0).signed(-1).signed(-1).u32(inv_index);
        for v in [
            id, 10, 0, 4001, 0, -1, 0, 0, 1, 0, 0, 0, 0, 1, 1, 0, 0, -1, -1, 0, 0, 0, 0,
        ] {
            iw.signed(v);
        }
        iw.wide(name).wide("").signed(0);
        iw.data
    };
    let mut lw = Writer::default();
    lw.signed(2).u32(0);
    lw.bytes(&encode_net_item(101, 10, "Gnoll Fang"));
    lw.bytes(&encode_net_item(102, 11, "Rusty Dirk"));
    let mut mut_w = Play::bundled_ui_walker();
    apply(&mut p, &mut mut_w, Event::Opcode { op: 0x0018, data: lw.data });
    p.run_npcw(&vui, &mut null_r);
    // Arrow down twice in loot window -> sel = 2 ("Rusty Dirk" row 3)
    for _ in 0..2 {
        p.npcw_key(139, true);
        p.run_npcw(&vui, &mut null_r);
        p.npcw_key(139, false);
        p.run_npcw(&vui, &mut null_r);
    }
    frame(&p, Some(&w), &h);
    h.execute(
        r#"
        local lw = eqoa.windows.find('loot')
        assert(lw ~= nil and #lw.rows == 4)
        assert(lw.rows[1].text == 'LEAVE ALL' and lw.rows[1].command == 'loot.leave_all' and lw.rows[1].item == nil)
        assert(lw.rows[2].text == 'Gnoll Fang' and lw.rows[2].item ~= nil)
        assert(eqoa.items.get(lw.rows[2].item).name == 'Gnoll Fang')
        assert(lw.rows[3].text == 'Rusty Dirk' and lw.rows[4].text == 'TAKE ALL' and lw.rows[4].command == 'loot.take_all')
        assert(lw.selected_row == 3 and lw.scroll_top == 1,
            string.format('loot sel=%s top=%s', tostring(lw.selected_row), tostring(lw.scroll_top)))
        "#,
    )
    .unwrap();
    apply(&mut p, &mut mut_w, Event::Opcode { op: 0x0016, data: vec![] });
    p.mode = 0;

    // 2. Bank window (mode 11) via BankPlay::read_lists()
    let mut b_item = Item::default();
    b_item.id = 201;
    b_item.name = "Stored Sapphire".into();
    let mut dump = Box::<net::dump::MemoryDump>::default();
    dump.bank.items = ItemList {
        items: vec![NetItem { inv_index: 5, quantity: 1, item: b_item, ..Default::default() }],
        ..Default::default()
    };
    apply(&mut p, &mut mut_w, Event::WorldEntry { self_id: 100, player: net::session::Player::default(), dump });
    apply(&mut p, &mut mut_w, Event::Opcode { op: 0x124D, data: vec![] });
    p.run_bank(&vui, &mut null_r);
    // Move to "Items" option (down, enter), then Right arrow to focus bank list ("Stored Sapphire")
    p.bank_key(139, true);
    p.bank_key(134, true);
    p.bank_key(137, true);
    p.run_bank(&vui, &mut null_r);
    frame(&p, Some(&w), &h);
    h.execute(
        r#"
        local bw = eqoa.windows.find('bank')
        assert(bw ~= nil and #bw.rows == 1)
        assert(bw.rows[1].text == 'Stored Sapphire' and bw.rows[1].item ~= nil)
        assert(eqoa.items.get(bw.rows[1].item).name == 'Stored Sapphire')
        assert(bw.selected_row == 1 and bw.scroll_top == 1)
        "#,
    )
    .unwrap();

    // 3. Quest Log (mode 22) via QuestLog::read_state()
    p.quests.titles = vec![
        vi::ui::wide("First Errand"),
        vi::ui::wide("Goblin Threat"),
        vi::ui::wide("Lost Relic"),
    ];
    p.mode = 22;
    p.quest_open = true;
    p.run_quests(&vui, &mut null_r);
    p.quest_keys.push((139, true)); // Down -> sel = 1 ("Goblin Threat")
    p.run_quests(&vui, &mut null_r);
    frame(&p, Some(&w), &h);
    h.execute(
        r#"
        local qw = eqoa.windows.find('quest_log')
        assert(qw ~= nil and #qw.rows == 3)
        assert(qw.rows[1].text == 'First Errand' and qw.rows[2].text == 'Goblin Threat')
        assert(qw.selected_row == 2 and qw.scroll_top == 1)
        "#,
    )
    .unwrap();

    // 4. Start menu (mode 13) via MainMenu::read_state()
    p.bags_start_built();
    {
        let start = p.start.as_mut().unwrap();
        let entry1 = crate::client_image::MenuEntry { msg: 0x0800_0001, id: 1, ..Default::default() };
        let entry2 = crate::client_image::MenuEntry { msg: 0x0800_0002, id: 2, ..Default::default() };
        start.menus.list[0].entries = vec![entry1, entry2];
        start.menus.labels.insert(
            (0x0800_0001, 1),
            crate::client_image::Label { text: vi::ui::wide("Character"), tail: vec![], dim: false },
        );
        start.menus.labels.insert(
            (0x0800_0002, 2),
            crate::client_image::Label { text: vi::ui::wide("Raid (Locked)"), tail: vec![], dim: true },
        );
        let loc = crate::startmenu::Location {
            world: 0,
            pos: [0.0; 3],
            cols: 1,
            rows: 1,
            cell: 1.0,
            origin: [0.0; 3],
            bounds: None,
        };
        start.menu.visible = true;
        start.menu.show_main(&vui, &null_r, true, 0, &loc);
        let mut cues = Vec::new();
        start.menu.key(&vui, &null_r, &start.menus, 139, true, 1, &loc, &mut cues);
        let sm_stats = crate::startmenu::Stats::default();
        start.menu.draw(&vui, &mut null_r, &start.menus, 2, &sm_stats, &mut cues); // pulse -> sel = 1
        start.menu.key(&vui, &null_r, &start.menus, 139, false, 3, &loc, &mut cues);
    }
    p.mode = 13;
    frame(&p, Some(&w), &h);
    h.execute(
        r#"
        local sw = eqoa.windows.find('start_menu')
        assert(sw ~= nil and #sw.rows == 2)
        assert(sw.rows[1].text == 'Character' and sw.rows[1].enabled == true and sw.rows[1].command == '0x8000001:1')
        assert(sw.rows[2].text == 'Raid (Locked)' and sw.rows[2].enabled == false)
        assert(sw.selected_row == 2 and sw.scroll_top == 1)
        "#,
    )
    .unwrap();
}

#[test]
fn round6a_seam_trainer_mastery_and_spellbook_rows() {
    let mut p = ready();
    let w = Play::bundled_ui_walker();
    let h = forge_host("seam_skills");
    let mut lines = vec![String::new(); 1800];
    for (idx, word) in [
        (76, "Details"),
        (247, "Strength"),
        (248, "Stamina"),
        (249, "Agility"),
        (250, "Dexterity"),
        (251, "Wisdom"),
        (252, "Intelligence"),
        (253, "Charisma"),
        (481, "Train"),
        (510, "Tradeskill"),
        (1652, "General"),
        (1653, "Archetype"),
        (1654, "Class"),
        (1655, "Master Class"),
    ] {
        lines[idx] = word.into();
    }
    let vui = crate::ui::Vui::new(
        vi::strtab::StringTable::parse(lines.join("\n").as_bytes()).unwrap(),
        Default::default(),
        vec![],
    );
    let mut null_r = crate::pointer::testing::NullRaster;
    p.words = Some(vui.words.clone());

    // 1. Trainer (mode 12) via Trainer::read_state()
    p.mode = 12;
    p.trainer_open = true;
    p.run_trainer(&vui, &mut null_r);
    p.trainer_keys.push((139, true)); // Down -> Stamina (row 2)
    p.run_trainer(&vui, &mut null_r);
    frame(&p, Some(&w), &h);
    h.execute(
        r#"
        local tw = eqoa.windows.find('trainer')
        assert(tw ~= nil and #tw.rows == 9)
        assert(tw.rows[1].text == 'Strength' and tw.rows[2].text == 'Stamina')
        assert(tw.rows[8].text == 'Train' and tw.rows[8].command == 'trainer.train')
        assert(tw.rows[9].text == 'Details' and tw.rows[9].command == 'trainer.details')
        assert(tw.selected_row == 2)
        "#,
    )
    .unwrap();
    // Move down to Details bar (mode == 2 -> selected_row 9)
    for _ in 0..7 {
        p.trainer_keys.push((139, true));
    }
    p.run_trainer(&vui, &mut null_r);
    frame(&p, Some(&w), &h);
    h.execute("assert(eqoa.windows.find('trainer').selected_row == 9)").unwrap();

    // 2. Class Mastery (mode 40) via ClassMastery::read_state()
    p.cm.skills = vec![
        crate::classmastery::Skill {
            x00: 100,
            on: true,
            known: 1,
            id: 10,
            parent: 0,
            category: 1, // Archetype (tab 2)
            cost: 3,
            name: vi::ui::wide("Iron Will"),
        },
        crate::classmastery::Skill {
            x00: 101,
            on: false,
            known: 0,
            id: 11,
            parent: 0,
            category: 1,
            cost: 4,
            name: vi::ui::wide("Secret Art"),
        },
    ];
    p.mode = 40;
    p.cm_wnd.open(&vui, &null_r, &p.cm);
    let mut cues = Vec::new();
    // Switch category (+1 via key 136) -> category 1 ("Archetype", tab 2), switch column -> available (col 1)
    p.cm_wnd.key(&vui, &mut null_r, &mut p.cm, 136, true, 0, &mut cues);
    p.cm_wnd.key(&vui, &mut null_r, &mut p.cm, 9, true, 0, &mut cues);
    frame(&p, Some(&w), &h);
    h.execute(
        r#"
        local mw = eqoa.windows.find('mastery')
        assert(mw ~= nil and mw.tab == 2 and mw.tabs[2] == 'Archetype' and #mw.tabs == 5,
            (function()
                local p = {}
                for k = 1, 5 do p[#p+1] = string.format('[%d]=%q', k, tostring(mw and mw.tabs and mw.tabs[k])) end
                return 'tab=' .. tostring(mw and mw.tab) .. ' ntabs=' .. tostring(mw and mw.tabs and #mw.tabs) .. ' ' .. table.concat(p, ' ')
            end)())
        assert(#mw.rows == 1 and mw.rows[1].text == '???' and mw.rows[1].enabled == false)
        assert(mw.selected_row == 1 and mw.scroll_top == 1)
        "#,
    )
    .unwrap();

    // Switch back to owned (col 0) -> Iron Will
    p.cm_wnd.key(&vui, &mut null_r, &mut p.cm, 9, true, 0, &mut cues);
    frame(&p, Some(&w), &h);
    h.execute(
        r#"
        local mw = eqoa.windows.find('mastery')
        assert(#mw.rows == 1 and mw.rows[1].text == 'Iron Will' and mw.rows[1].enabled == true)
        "#,
    )
    .unwrap();

    // 3. SpellBook (mode 5) via SpellBook::top() + public fields
    p.abilities.insert(
        501,
        net::dump::Ability {
            id: 501,
            name: "Greater Heal".into(),
            level: 12,
            ..Default::default()
        },
    );
    p.abilities.insert(
        502,
        net::dump::Ability {
            id: 502,
            name: "Flash of Light".into(),
            level: 14,
            ..Default::default()
        },
    );
    p.book.open(&vui, &null_r, 0, 0);
    p.book.rows = vec![
        crate::spellbook::Row { id: 501, valid: true, index: 1, ..Default::default() },
        crate::spellbook::Row { id: 502, valid: true, index: 2, ..Default::default() },
    ];
    p.book.tab = 1; // 11-20 -> tab 2
    p.book.sel = 1; // Flash of Light -> selected_row 2
    p.mode = 5;
    frame(&p, Some(&w), &h);
    h.execute(
        r#"
        local sb = eqoa.windows.find('spellbook')
        assert(sb ~= nil and sb.tab == 2 and #sb.tabs == 5)
        assert(#sb.rows == 2 and sb.rows[1].text == 'Greater Heal' and sb.rows[2].text == 'Flash of Light')
        assert(sb.selected_row == 2 and sb.scroll_top == 1)
        "#,
    )
    .unwrap();
}
