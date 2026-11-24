// Code in this file is LoopRook's addition for EQOA Enhanced and is under the GNU Affero General Public License v3 or later (see LICENSE).
//! Performance round (core), part 3: the mods' snapshot is rebuilt on the GAME's 30 Hz step (the step the frame hook
//! tells `native_core::iteration`, the one that advances `Client.tick` and dispatches the mods' actions), not on
//! every drawn frame. The player's position and heading are patched in on every drawn frame, and a state event
//! produced between steps rebuilds at once, so a handler never reads a snapshot older than the event it was told about.
//!
//! These do not ask for the fixture's "manual" capture (which forces a rebuild on every call): an active mod VM
//! is what makes the core capture, as in the game. `native_core::iteration(true / false)` plays the frame hook.
use super::*;
use crate::fork_ext::addons::{
    action_broker,
    host::Host,
    native_a3,
    native_core::{self, CORE, Core},
    native_events,
};

fn creature(id: u32, name: &str) -> Option<net::entity::Entity> {
    Some(net::entity::Entity {
        id,
        kind: net::entity::TYPE_CREATURE,
        p18: 1,
        p19: 200,
        creature: Some(net::entity::Creature { name: name.into(), p256: 2, p264: 1, ..Default::default() }),
        ..Default::default()
    })
}

fn setup() -> (Play, Host) {
    crate::settings::testing::reset(false);
    CORE.with(|c| *c.borrow_mut() = Core::default());
    native_core::fixture_demand(false);
    action_broker::reset_all_for_test();
    native_a3::reset_for_test();
    super::updates::inactive();
    super::updates::set_no_upgrade_for_test(false);
    native_core::reset_game_step_for_test();
    let mut p = super::super::tests::play();
    let _ = p.loading.wait.take();
    p.hud = Some((
        crate::ui::Vui::new(Default::default(), Default::default(), vec![]),
        Default::default(),
        crate::hud::Hud::new(),
    ));
    let host = Host::with_test_capabilities(&["eqoa.actions.*"]).unwrap();
    // the test host asks for the fixture's manual capture (a rebuild on every call); the game does not
    native_core::fixture_demand(false);
    assert!(p.in_world_now() && native_core::capture_requested(), "an active mod VM makes the core capture");
    (p, host)
}
fn done() {
    super::updates::set_no_upgrade_for_test(false);
    native_core::reset_game_step_for_test();
}

/// 60 drawn frames at 60 Hz, the game stepping on every other one, something the snapshot is built from changing
/// every frame.
fn builds_over_a_second(game_steps: bool) -> u64 {
    let (mut p, _host) = setup();
    if game_steps {
        native_core::iteration(true);
    }
    p.forge_snapshot(None);
    let before = super::updates::builds_for_test();
    for i in 0..60 {
        if game_steps {
            native_core::iteration(i % 2 == 1);
        }
        p.dump.player.exp += 1;
        p.forge_snapshot(None);
    }
    done();
    super::updates::builds_for_test() - before
}

/// At 60 drawn frames a second the snapshot is built about 30 times a second, on the game's steps, not 60.
/// (Control: with no iteration ever seen, the old behaviour, every one of the 60 frames builds.)
#[test]
fn the_snapshot_is_built_on_the_30_hz_step_not_every_drawn_frame() {
    let stepped = builds_over_a_second(true);
    assert!((29..=31).contains(&stepped), "about 30 builds in 60 frames, got {stepped}");
    let every_frame = builds_over_a_second(false);
    assert!(every_frame >= 59, "control: without the step every frame builds, got {every_frame}");
}

/// A value that changed between two steps shows on the next game step, not before. (Control: before the step it is
/// still the old value.)
#[test]
fn a_value_changed_between_steps_shows_on_the_next_game_step() {
    let (mut p, _host) = setup();
    native_core::iteration(true);
    p.dump.player.exp = 100;
    assert_eq!(p.forge_snapshot(None).player.exp, 100);
    native_core::iteration(false);
    p.dump.player.exp = 250;
    assert_eq!(p.forge_snapshot(None).player.exp, 100, "control: between steps it is the older value");
    native_core::iteration(true);
    assert_eq!(p.forge_snapshot(None).player.exp, 250, "and the game's next step shows it");
    done();
}

/// The player's position and heading are fresh on every drawn frame, between steps too, so the minimap and anything
/// that follows the player moves smoothly. (Control: another changed field in the same frame is still step-old.)
#[test]
fn the_players_position_and_heading_are_fresh_on_every_drawn_frame() {
    let (mut p, _host) = setup();
    let mut w = Play::bundled_ui_walker();
    native_core::iteration(true);
    p.dump.player.exp = 1;
    assert_eq!(p.forge_snapshot(Some(&w)).player.exp, 1);
    for i in 0..3 {
        native_core::iteration(false);
        w.cam.walk.phys.position.x.0 += 3.0;
        w.cam.walk.phys.angles.x.0 += 0.1;
        p.dump.player.exp = 2 + i as i64;
        let s = p.forge_snapshot(Some(&w));
        assert_eq!(s.player.position[0], w.cam.walk.phys.position.x.0, "frame {i}: position is this frame's");
        assert!((s.player.heading_rad - w.cam.walk.phys.angles.x.0.rem_euclid(std::f32::consts::TAU)).abs() < 1e-4, "frame {i}: heading");
        assert_eq!(s.player.exp, 1, "frame {i}: control, the rest is from the last step");
    }
    done();
}

// ---- events between steps: a handler sees state at least as new as the event ------------------------------------

fn target_title_seen_by_handler(no_upgrade: bool) -> String {
    let (mut p, h) = setup();
    super::updates::set_no_upgrade_for_test(no_upgrade);
    h.execute("seen = ''; eqoa.events.on('target.changed', function() local i = eqoa.target.info(); seen = (i and i.title) or 'none' end)").unwrap();
    p.target = 0x2001;
    p.entities.slots[1] = creature(0x2001, "Mira");
    p.target_info = Some(4);
    p.rings.con = Some(3);
    p.target_title = "Old".into();
    // a game step: everything is built
    native_core::iteration(true);
    h.snapshot.set(p.forge_snapshot(None));
    native_events::dispatch(&h).unwrap();
    // between steps: the server describes the target and the core reports the change
    native_core::iteration(false);
    p.target_title = "New".into();
    native_core::defer(0, |c| c.emit("target.changed", serde_json::json!({})));
    h.snapshot.set(p.forge_snapshot(None));
    native_events::dispatch(&h).unwrap();
    let seen: String = h.lua.globals().get("seen").unwrap();
    done();
    seen
}

/// `target.changed` between steps: the handler's `eqoa.target.info()` is the new target's. (Control, the gate as it
/// was: the handler read the previous description.)
#[test]
fn target_changed_between_steps_shows_the_new_target_detail() {
    assert_eq!(target_title_seen_by_handler(false), "New");
    assert_eq!(target_title_seen_by_handler(true), "Old", "control: without the rebuild the handler is a step behind");
}

fn ui_mode_after_window_change(no_upgrade: bool) -> (u32, u64) {
    let (mut p, h) = setup();
    super::updates::set_no_upgrade_for_test(no_upgrade);
    h.execute("eqoa.events.on('windows.changed', function() end)").unwrap();
    native_core::iteration(true);
    h.snapshot.set(p.forge_snapshot(None));
    native_events::dispatch(&h).unwrap();
    let before = super::updates::builds_for_test();
    // between steps the merchant window opens: the windows module emits `windows.changed` at once
    native_core::iteration(false);
    p.mode = 10;
    let s = p.forge_snapshot(None);
    h.snapshot.set(s);
    native_events::dispatch(&h).unwrap();
    let rebuilt = super::updates::builds_for_test() - before;
    done();
    (s.ui_mode, rebuilt)
}

/// `windows.changed` between steps: the snapshot a handler reads has the new window mode (the merchant is open).
/// (Control, the gate as it was: the snapshot still said no window.)
#[test]
fn windows_changed_between_steps_shows_the_new_window_mode() {
    let (mode, rebuilt) = ui_mode_after_window_change(false);
    assert_eq!((mode, rebuilt), (10, 1), "rebuilt at once for the event");
    let (mode, rebuilt) = ui_mode_after_window_change(true);
    assert_eq!((mode, rebuilt), (0, 0), "control: the old gate kept the step-old snapshot");
}

/// A game step always rebuilds, so what an action's `action.result` (queued by the step that carried it out) tells a
/// mod is already in the snapshot: the player's state is new on that very frame. (Control: the same change on a
/// frame between steps, with no event, is not shown until the next step.)
#[test]
fn an_action_result_on_a_game_step_frame_sees_the_new_player_state() {
    let (mut p, h) = setup();
    h.execute("eqoa.events.on('action.result', function() end)").unwrap();
    native_core::iteration(true);
    p.dump.player.exp = 10;
    p.forge_snapshot(None);
    // the game steps; the action it carried out changed the player; its result is queued for the mods
    native_core::iteration(true);
    p.dump.player.exp = 20;
    native_core::defer(0, |c| c.emit("action.result", serde_json::json!({"state": "completed"})));
    let s = p.forge_snapshot(None);
    assert_eq!(s.player.exp, 20, "the frame of the game step has the state the action produced");
    // control: the same change one drawn frame later, no step, no event
    native_core::iteration(false);
    p.dump.player.exp = 30;
    assert_eq!(p.forge_snapshot(None).player.exp, 20, "control: between steps, with nothing to tell, it waits for the step");
    done();
}

/// Prints the cost of `forge_snapshot` per drawn frame at 60 Hz with the game stepping every other frame (after) and
/// with no step flag (every frame builds: before). Run with `--nocapture` (run_gates does). Debug profile.
#[test]
fn perf_core_step_timing() {
    fn per_call(game_steps: bool) -> f64 {
        let (mut p, _host) = setup();
        let mut w = Play::bundled_ui_walker();
        let frames = 600;
        let start = std::time::Instant::now();
        for i in 0..frames {
            if game_steps {
                native_core::iteration(i % 2 == 0);
            }
            w.cam.walk.phys.position.x.0 += 0.5;
            p.dump.player.exp += 1;
            p.forge_snapshot(Some(&w));
        }
        let ms = start.elapsed().as_secs_f64() * 1000.0 / frames as f64;
        done();
        ms
    }
    let before = per_call(false);
    let after = per_call(true);
    eprintln!("perf_core_step_timing: every frame {before:.3} ms, on the step {after:.3} ms per drawn frame (60 Hz, debug)");
    assert!(after > 0.0 && before > 0.0);
}
