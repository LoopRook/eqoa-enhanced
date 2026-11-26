// Copyright LoopRook. SPDX-License-Identifier: AGPL-3.0-or-later
//! The game's Stat Window held back by a mod (`eqoa.hud.hide('stat_window')`), as the player meets it: F4 and
//! `/stats`, the panel, a click on it, in the PC interface and the controller interface. The element's own rules
//! (claims, release, `all`) are in `addons/a12_tests.rs`.
use super::*;
use crate::fork_ext::addons::{
    health::InstanceToken,
    visibility::{HudClaimsRegistry, publish_claims},
};

struct Nowhere;
impl net::session::Datagrams for Nowhere {
    fn send_to(&mut self, _: &[u8], _: [u8; 4], _: u16) {}
    fn recv_from(&mut self) -> Option<(Vec<u8>, [u8; 4], u16)> {
        None
    }
}

fn play() -> Play {
    let mut s = net::session::Session::connect_with(Box::new(Nowhere), ([127, 0, 0, 1], 5000), ([127, 0, 0, 1], 10070), "test", [0; 32]).unwrap();
    s.self_id = Some(0x100);
    Play::new(s, net::session::Player::default(), Box::default())
}

/// A mod holds (or lets go of) the Stat Window.
fn mod_holds(on: bool) {
    let mut reg = HudClaimsRegistry::new();
    if on {
        let token = InstanceToken::next().unwrap();
        reg.disable("stat_window", token, "test");
    }
    publish_claims(&reg);
}

/// The game's keys for the window while a mod holds it back. F4 is `toggle_stat_window`, `/stats` is `stats_command`;
/// both end in `set_stat_window`. Run in both interfaces. Negative control: the same steps with no mod change the
/// setting (and, with the early return taken out of `set_stat_window`, the held steps fail).
#[test]
fn stat_window_while_held_f4_and_stats_do_nothing_and_the_players_setting_is_kept() {
    use crate::settings;
    for pc in [true, false] {
        settings::testing::reset(false);
        settings::set_ui_mode(pc).unwrap();
        mod_holds(false);
        let mut p = play();
        // control: no mod, F4 and /stats are upstream's (they turn it and keep it)
        p.toggle_stat_window();
        assert!(settings::stat_window(), "pc={pc}: F4 shows it with no mod");
        p.chat_line("/stats");
        assert!(!settings::stat_window(), "pc={pc}: /stats hides it with no mod");
        p.toggle_stat_window();
        assert!(settings::stat_window());
        p.cues.clear();

        // a mod hides it while it is open: F4 and /stats do not touch the player's setting, and no cue plays
        mod_holds(true);
        p.toggle_stat_window();
        p.chat_line("/stats");
        p.chat_line("/stats off");
        assert!(settings::stat_window(), "pc={pc}: the open window stays the player's choice while held");
        assert!(p.cues.is_empty(), "pc={pc}: no cue sound for a window that is not there");

        // closed, held: F4 and /stats on do not open it
        mod_holds(false);
        p.toggle_stat_window();
        assert!(!settings::stat_window());
        mod_holds(true);
        p.toggle_stat_window();
        p.chat_line("/stats on");
        assert!(!settings::stat_window(), "pc={pc}: F4 and /stats on do not open a held window");

        // the mod stops: the game's window is the player's again, as vanilla
        mod_holds(false);
        p.toggle_stat_window();
        assert!(settings::stat_window(), "pc={pc}: F4 works again once no mod holds it");
        settings::testing::reset(false);
    }
}

/// A click: with the window drawn (a panel) it is the window's own; with a mod holding it the draw makes no panel
/// (`play.rs` clears `stat_panel` before the gated draw), so there is nothing to find. The draw itself needs a
/// graphics adapter and the disc: see `stat_window_hidden_is_not_drawn_and_takes_no_click_in_either_interface`.
#[test]
fn stat_window_a_click_finds_nothing_without_a_panel() {
    let mut p = play();
    let panel = crate::statwnd::panel([3, 7, 10, 10]);
    p.stat_panel = Some(panel);
    let inside = [(panel.rect[0] + 20) as f32, (panel.rect[1] + 30) as f32];
    assert!(p.stat_point(inside), "control: a drawn panel takes the click");
    p.stat_panel = None;
    assert!(!p.stat_point(inside), "no panel (not drawn this frame): the click is the world's");
}

/// Manual, GPU and disc: the Stat Window drawn, then held back by a mod while open, then given back, in the PC
/// interface and in the controller's. Run alone with EQOA_ISO, EQOA_OFFLINE_HUD_OUT and an isolated EQOA_CACHE
/// (as `offline_hud_test`).
#[test]
#[ignore = "manual offline GPU interaction test: requires local disc"]
fn stat_window_hidden_is_not_drawn_and_takes_no_click_in_either_interface() {
    use crate::play::offline_hud_test::{fixture_play, offline_assets};
    use render::wgpu;
    let (mut lib, source, _output) = offline_assets();
    let size = (960, 540);
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&render::packed::device_descriptor(&adapter))).unwrap();
    let mut renderer = render::Renderer::new(&device, &queue, render::TARGET_FORMAT);
    let mut p = fixture_play();
    p.loading.wait = None;
    p.take_modules(None);
    p.attach_ui(&mut renderer, &device, &queue, render::TARGET_FORMAT, &lib);
    let scene = source.scene(&lib);
    let (walker, _, _) = source.walker(&mut lib, &scene).expect("offline world walker");
    let mut now = 1000i64;
    for pc in [true, false] {
        crate::settings::testing::reset(false);
        crate::settings::set_ui_mode(pc).unwrap();
        mod_holds(false);
        let mut frame = |p: &mut Play, now: &mut i64| {
            *now += 100;
            p.hud(renderer.ui.as_mut().unwrap(), &walker, *now, glam::Mat4::IDENTITY, size, true);
            renderer.ui.as_ref().unwrap().quads.len()
        };
        // control: window off, then on with no mod: drawn, takes a click on it
        let off = frame(&mut p, &mut now);
        assert!(p.stat_panel.is_none(), "pc={pc}: off, no panel");
        p.toggle_stat_window();
        let drawn = frame(&mut p, &mut now);
        let panel = p.stat_panel.expect("the Stat Window is drawn with no mod");
        assert!(drawn > off, "pc={pc}: it adds quads");
        let inside = [(panel.rect[0] + 20) as f32, (panel.rect[1] + 30) as f32];
        // a mod hides it while open: not drawn, no panel, a click on where it was finds nothing
        mod_holds(true);
        let held = frame(&mut p, &mut now);
        assert!(p.stat_panel.is_none(), "pc={pc}: held, no panel");
        assert_eq!(held, off, "pc={pc}: held, it draws nothing (as if it were closed)");
        assert!(!p.stat_point(inside), "pc={pc}: a click where it was is the world's");
        assert!(crate::settings::stat_window(), "pc={pc}: the player's own setting is kept");
        // the mod stops: the game's window is back, as before
        mod_holds(false);
        let back = frame(&mut p, &mut now);
        assert_eq!(back, drawn, "pc={pc}: given back, drawn as before");
        assert!(p.stat_point(inside), "pc={pc}: and takes its click again");
        mod_holds(false);
    }
}
