// Code in this file is LoopRook's addition for EQOA Enhanced and is under the GNU Affero General Public License v3 or later (see LICENSE).
//! Performance round (core), part 3: the mods' snapshot is rebuilt on the game's 30 Hz step (`frames.rs`), not on
//! every drawn frame. The player's position and heading are patched in on every drawn frame; everything else
//! may be up to one step old.
//!
//! These do not ask for the fixture's "manual" capture (which forces a rebuild on every call): an active mod VM
//! is what makes the core capture, as in the game. The step clock is a test seam (`set_test_clock`).
use super::*;
use crate::fork_ext::addons::{
    action_broker,
    host::Host,
    native_a3,
    native_core::{self, CORE, Core},
};

fn setup() -> (Play, Host) {
    crate::settings::testing::reset(false);
    CORE.with(|c| *c.borrow_mut() = Core::default());
    native_core::fixture_demand(false);
    action_broker::reset_all_for_test();
    native_a3::reset_for_test();
    super::updates::inactive();
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

/// 60 drawn frames at 60 Hz with something the snapshot is built from changing every frame.
fn builds_over_a_second(clock: bool) -> u64 {
    let (mut p, _host) = setup();
    super::updates::set_test_clock(clock.then_some(1000.0));
    let before = super::updates::builds_for_test();
    for i in 0..60 {
        if clock {
            super::updates::set_test_clock_keep(1000.0 + i as f64 * 1000.0 / 60.0);
        }
        p.dump.player.exp += 1;
        p.forge_snapshot(None);
    }
    super::updates::set_test_clock(None);
    super::updates::builds_for_test() - before
}

/// At 60 drawn frames a second the snapshot is built about 30 times a second, not 60. (Control: with no step clock,
/// the old behaviour, every one of the 60 frames builds.)
#[test]
fn the_snapshot_is_built_on_the_30_hz_step_not_every_drawn_frame() {
    let stepped = builds_over_a_second(true);
    assert!((28..=32).contains(&stepped), "about 30 builds in 60 frames, got {stepped}");
    let every_frame = builds_over_a_second(false);
    assert!(every_frame >= 59, "control: without the step every frame builds, got {every_frame}");
}

/// A value that changed between two steps shows on the next step, not before; the next step is at most one
/// game step (33 ms) away. (Control: before the step it is still the old value.)
#[test]
fn a_value_changed_between_steps_shows_on_the_next_step() {
    let (mut p, _host) = setup();
    super::updates::set_test_clock(Some(0.0));
    p.dump.player.exp = 100;
    let first = p.forge_snapshot(None);
    assert_eq!(first.player.exp, 100);
    // 16.7 ms later (a drawn frame between steps): the change is not shown yet
    super::updates::set_test_clock_keep(16.7);
    p.dump.player.exp = 250;
    assert_eq!(p.forge_snapshot(None).player.exp, 100, "control: between steps it is the older value");
    // 33.4 ms: the next step
    super::updates::set_test_clock_keep(33.4);
    assert_eq!(p.forge_snapshot(None).player.exp, 250, "and the next step shows it");
    super::updates::set_test_clock(None);
}

/// The player's position and heading are fresh on every drawn frame, between steps too, so the minimap and
/// anything that follows the player moves smoothly. (Control: other fields in the same frame are still step-old.)
#[test]
fn the_players_position_and_heading_are_fresh_on_every_drawn_frame() {
    let (mut p, _host) = setup();
    let mut w = Play::bundled_ui_walker();
    super::updates::set_test_clock(Some(0.0));
    p.dump.player.exp = 1;
    let first = p.forge_snapshot(Some(&w));
    assert_eq!(first.player.exp, 1);
    for (i, ms) in [5.0, 11.0, 16.7].into_iter().enumerate() {
        super::updates::set_test_clock_keep(ms);
        w.cam.walk.phys.position.x.0 += 3.0;
        w.cam.walk.phys.angles.x.0 += 0.1;
        p.dump.player.exp = 2 + i as i64;
        let s = p.forge_snapshot(Some(&w));
        assert_eq!(s.player.position[0], w.cam.walk.phys.position.x.0, "frame {i}: position is this frame's");
        assert!((s.player.heading_rad - w.cam.walk.phys.angles.x.0.rem_euclid(std::f32::consts::TAU)).abs() < 1e-4, "frame {i}: heading is this frame's");
        assert_eq!(s.player.exp, 1, "frame {i}: control, the rest is from the last step");
    }
    super::updates::set_test_clock(None);
}

/// Prints the cost of `forge_snapshot` per drawn frame at 60 Hz with a step clock (after) and without (every
/// frame builds: before). Run with `--nocapture` (run_gates does). Debug profile unless run with --release.
#[test]
fn perf_core_step_timing() {
    fn per_call(clock: bool) -> f64 {
        let (mut p, _host) = setup();
        let mut w = Play::bundled_ui_walker();
        super::updates::set_test_clock(clock.then_some(1000.0));
        let frames = 600;
        let start = std::time::Instant::now();
        for i in 0..frames {
            if clock {
                super::updates::set_test_clock_keep(1000.0 + i as f64 * 1000.0 / 60.0);
            }
            w.cam.walk.phys.position.x.0 += 0.5;
            p.dump.player.exp += 1;
            p.forge_snapshot(Some(&w));
        }
        let ms = start.elapsed().as_secs_f64() * 1000.0 / frames as f64;
        super::updates::set_test_clock(None);
        ms
    }
    let before = per_call(false);
    let after = per_call(true);
    eprintln!("perf_core_step_timing: every frame {before:.3} ms, on the step {after:.3} ms per drawn frame (60 Hz, debug)");
    assert!(after > 0.0 && before > 0.0);
}
