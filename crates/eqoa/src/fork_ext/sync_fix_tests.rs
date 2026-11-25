// Copyright LoopRook. SPDX-License-Identifier: AGPL-3.0-or-later
//! Fixes after the review of the upstream sync of 2026-11-25 (upstream's PC interface and HUD Scale): a chat a mod
//! hides, the loot-roll strips a mod holds, and the HUD numbers' places at a HUD Scale. Every test runs both
//! interfaces (PC and Controller) where the fix touches input or layout, and says what its negative control is.
use super::*;
use crate::fork_ext::addons::{
    health::InstanceToken,
    host::Host,
    visibility::{HudClaimsRegistry, publish_claims},
    window_takeover::{self, TakeoverKind},
};
use net::raid::Roll;

fn interface(pc: bool) {
    crate::settings::testing::reset(false);
    crate::settings::set_ui_mode(pc).unwrap();
    crate::hudlayout::testing_reset();
    assert_eq!(crate::settings::pc_mode(), pc);
}

fn hide(elements: &[&str]) {
    let mut reg = HudClaimsRegistry::new();
    let token = InstanceToken::next().unwrap();
    for e in elements {
        reg.disable(e, token, "test");
    }
    publish_claims(&reg);
}

/// A Play on a chat box with lines, drawn so its window is open where a click or the wheel can find it.
fn chat_play(mode: i32, detached: bool) -> Play {
    let mut p = tests::play();
    p.mode = mode;
    let mut wnd = crate::chat::WndChat::new();
    wnd.mode = 1;
    wnd.always = true;
    if detached {
        let mut routes = [0u8; crate::chat::KINDS];
        routes[0] = 1;
        wnd.set_tabs(routes, Default::default());
        assert!(wnd.set_detached(1, true), "a pane for tab 1");
    }
    let vui = crate::pointer::testing::vui();
    let mut nr = crate::pointer::testing::NullRaster;
    for i in 0..8 {
        let line = crate::ui::wide(&format!("line {i}"));
        match (detached, wnd.pane_mut(1)) {
            (true, Some(pane)) => pane.text.add_text(&line, [1.0; 4], 0, |_| 0),
            _ => wnd.text.add_text(&line, [1.0; 4], 0, |_| 0),
        }
    }
    wnd.draw(&vui, &mut nr, 1000, 0.5);
    wnd.draw(&vui, &mut nr, 2000, 0.5);
    if let Some(pane) = wnd.pane_mut(1) {
        pane.mode = 1;
        pane.draw(&vui, &mut nr, 1000, 0.5);
        pane.draw(&vui, &mut nr, 2000, 0.5);
    }
    p.wnd_chat = Some(wnd);
    p
}

/// How many points of a coarse window grid `act` is the chat box's at.
fn chat_hits(p: &mut Play, mut act: impl FnMut(&mut Play, (f64, f64)) -> bool) -> usize {
    let mut hits = 0;
    for y in (0..448).step_by(6) {
        for x in (0..640).step_by(6) {
            if act(p, (x as f64, y as f64)) {
                hits += 1;
            }
            // a press the box took is let go
            let _ = p.chat_release();
        }
    }
    hits
}

/// H2: a chat a mod hides (the stock box and the PC's detached panes) takes no click and no wheel, so the camera and
/// the target keep them. Control: the same points are the chat's with the chat shown; both interfaces; the box
/// and a detached pane (the PC's).
#[test]
fn sync_h2_a_hidden_chat_takes_no_click_and_no_wheel() {
    for (pc, detached) in [(true, false), (true, true), (false, false)] {
        interface(pc);
        let size = (640, 448);
        // the wheel (PC: over the box; Controller: while R2 holds the history, mode 33)
        let mode = if pc { 0 } else { 33 };
        hide(&[]);
        let mut p = chat_play(mode, detached);
        let shown = chat_hits(&mut p, |p, at| p.chat_wheel(at, size, 1.0));
        assert!(shown > 0, "control: pc={pc} detached={detached}: the shown chat takes the wheel");
        let clicks = chat_hits(&mut p, |p, at| p.chat_press(at, size, true));
        if pc {
            assert!(clicks > 0, "control: pc={pc} detached={detached}: the shown chat takes clicks");
        }
        hide(&["chat_window"]);
        let mut p = chat_play(mode, detached);
        assert_eq!(chat_hits(&mut p, |p, at| p.chat_wheel(at, size, 1.0)), 0, "pc={pc} detached={detached}: hidden, the wheel is the camera's");
        assert_eq!(chat_hits(&mut p, |p, at| p.chat_press(at, size, true)), 0, "pc={pc} detached={detached}: hidden, a click is the world's");
    }
    hide(&[]);
}

/// H1: Inline Keyboard Chat (PC, on by default) types into a box drawn inside the chat piece. With the stock chat
/// hidden by a mod that does not take Chat Entry, that box is blank: the line falls back to the game's own edit box.
/// Control: with the chat shown the inline box stays; the Controller interface never has one.
#[test]
fn sync_h1_a_hidden_chat_does_not_leave_the_player_typing_into_an_invisible_inline_box() {
    for pc in [true, false] {
        for hidden in [false, true] {
            interface(pc);
            hide(if hidden { &["chat_window"] } else { &[] });
            let mut p = chat_play(0, false);
            p.keyboard = true;
            assert!(crate::settings::client().inline_chat || !pc, "the PC interface has Inline Keyboard Chat by default");
            p.keyboard_chat_key("");
            assert_eq!(p.mode, 2, "the line is open (pc={pc} hidden={hidden})");
            assert_eq!(p.edit.inline_chat, pc, "the game opened the inline box in the PC interface only");
            a6_play::tick(&mut p);
            assert_eq!(p.edit.inline_chat, pc && !hidden, "pc={pc} hidden={hidden}: only a shown chat keeps its inline box");
            assert_eq!(p.mode, 2, "the line stays open: it is typed into the game's own edit box");
        }
    }
    hide(&[]);
}

fn host_with(id: &str, caps: &[&str], replaces: &[&str]) -> Host {
    let host = Host::new().unwrap();
    {
        let sess = crate::fork_ext::addons::native::session(&host.lua);
        let mut s = sess.borrow_mut();
        s.addon = id.to_string();
        s.native_manifest = true;
        s.capabilities = Some(caps.iter().map(|c| (*c).to_string()).collect());
        s.replaces = replaces.iter().map(|r| (*r).to_string()).collect();
    }
    host
}

/// M2: while a mod holds the loot-roll strips, the native strips' places take no wheel and cover no point (so the
/// wheel zooms the camera and a carried PC spell gem can be put back there). Control: unheld, they do; both interfaces.
#[test]
fn sync_m2_a_held_roll_strip_does_not_steal_the_wheel_or_cover_a_point() {
    for pc in [false, true] {
        interface(pc);
        window_takeover::reset_all_for_test();
        let mut p = tests::play();
        p.mode = 0;
        let item = net::dump::NetItem::default();
        for id in 1..=5 {
            p.raid_roll(Roll { id, seconds: 120, may_need: true, item: Some(Box::new(item.clone())) });
        }
        let (vui, mut nr) = (crate::pointer::testing::vui(), crate::pointer::testing::NullRaster);
        p.run_rolls(&vui, &mut nr);
        let size = (640, 448);
        let on = render::ui::window_from_frame(size, [450.0, 180.0]);
        let on = (on[0] as f64, on[1] as f64);
        let (covered, wheeled) = (p.rolls_cover([on.0 as f32, on.1 as f32], size), p.roll_wheel(on, size, -1.0));
        if !pc {
            assert!(covered && wheeled, "control: pc={pc}: unheld, the strips cover the point and the wheel is theirs");
        }
        let holder = host_with("roll_mod", &["eqoa.windows.takeover"], &["raid_roll"]);
        let ok: bool = holder.lua.load(r#"return assert(eqoa.windows.takeover("raid_roll", { scope = "all" }))"#).eval().unwrap();
        assert!(ok && window_takeover::is_taken(TakeoverKind::RaidRoll));
        p.run_rolls(&vui, &mut nr);
        assert!(!p.rolls_cover([on.0 as f32, on.1 as f32], size), "pc={pc}: held, the native strip covers nothing");
        assert!(!p.roll_wheel(on, size, -1.0), "pc={pc}: held, the wheel is the camera's");
        crate::fork_ext::addons::native_events::clear(&holder);
        window_takeover::reset_all_for_test();
    }
}

/// M1: `eqoa.hud.layout()` follows HUD Scale (either interface); the round minimap's capture rectangle follows the
/// scaled minimap. Control: at 100% they are as before; the cached layout is not stale when only the scale changes.
#[test]
fn sync_m1_hud_layout_and_the_minimap_capture_follow_hud_scale() {
    use crate::fork_ext::addons::hud_layout;
    let size = (640u32, 448u32);
    for pc in [false, true] {
        interface(pc);
        let at = |scale: u8| {
            let c = crate::settings::client();
            crate::settings::set_client(crate::settings::ClientOptions { hud_scale: scale, controller_hud_scale: scale, ..c });
        };
        let width = |json: &serde_json::Value, piece: &str| json[piece]["panel"]["width"].as_f64().unwrap();
        at(100);
        let base = hud_layout::hud_layout_json(size);
        let (player, health) = (width(&base, "player"), base["player"]["health"]["width"].as_f64().unwrap());
        let mut map = crate::map::Map::default();
        map.mini.up = true;
        let inner = |m: &crate::map::Map| m.api_minimap_transform([0.0; 2], size).unwrap().inner;
        let plain = inner(&map);
        at(150);
        let big = hud_layout::hud_layout_json(size);
        let scaled = |a: f64, b: f64| ((b / a) - 1.5).abs() < 0.02;
        assert!(scaled(player, width(&big, "player")), "pc={pc}: the player panel is 150% wide at HUD Scale 150");
        assert!(scaled(health, big["player"]["health"]["width"].as_f64().unwrap()), "pc={pc}: and its health bar");
        let big_inner = inner(&map);
        assert!(((big_inner[2] - big_inner[0]) / (plain[2] - plain[0]) - 1.5).abs() < 0.02, "pc={pc}: the minimap's capture is 150%: {plain:?} {big_inner:?}");
        at(100);
        let back = hud_layout::hud_layout_json(size);
        assert_eq!(width(&back, "player"), player, "pc={pc}: back at 100% the layout is the first again");
    }
    interface(false);
}

/// M1: the PC interface draws the pet at its own place (`hud.rs` PC_PET_AT), not under the group; the Controller
/// interface stacks it under the group (the control).
#[test]
fn sync_m1_the_pet_panel_is_at_its_pc_place_in_the_pc_interface() {
    for pc in [false, true] {
        interface(pc);
        let mut p = tests::play();
        p.pet_id = 7;
        p.group.in_group = true;
        p.group.members = vec![
            crate::group::Member { id: 77, name: "A".into(), active: true, ..Default::default() },
            crate::group::Member { id: 78, name: "B".into(), active: true, ..Default::default() },
        ];
        assert!(p.group.count() >= 2, "a group under the pet in the Controller interface");
        begin_iteration(&mut p, false);
        let y = crate::fork_ext::addons::hud_layout::get_hud_state().pet_y;
        assert_eq!(y, if pc { 93 } else { 93 + 48 * (p.group.count() as i32 - 1) }, "pc={pc}");
    }
    interface(false);
}
