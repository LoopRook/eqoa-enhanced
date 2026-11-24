// Code in this file is LoopRook's addition for EQOA Enhanced and is under the GNU Affero General Public License v3 or later (see LICENSE).
//! Performance round (core): the per-VM listener index and the empty deferred flush change cost only.
//! Who gets which event, in which order, and what the 256-delivery budget counts stay as they were.
use super::{
    host::Host,
    native_core::{self, CORE, Core},
    native_events,
};
use serde_json::json;
use std::time::Instant;

fn reset() {
    CORE.with(|c| *c.borrow_mut() = Core::default());
}
fn emit(name: &str) {
    CORE.with(|c| c.borrow_mut().emit(name, json!({"world_id": 1, "initial": false, "in_world": false})));
}
fn global(h: &Host, name: &str) -> i64 {
    h.lua.globals().get::<i64>(name).unwrap_or(0)
}

/// A VM with no listener for a name is not handed that event, and the index says so without a scan; the VM
/// that listens still gets it. (Control: a listener for the name makes the index say yes.)
#[test]
fn a_vm_with_no_listener_receives_nothing_and_the_index_says_so() {
    reset();
    let heard = Host::new().unwrap();
    let deaf = Host::new().unwrap();
    heard.execute("n = 0; eqoa.events.on('world.entered', function() n = n + 1 end)").unwrap();
    deaf.execute("n = 0; eqoa.events.on('state.reset', function() n = n + 1 end)").unwrap();
    assert!(native_events::bus(&heard.lua).borrow().listens_for_test("world.entered"));
    assert!(!native_events::bus(&deaf.lua).borrow().listens_for_test("world.entered"));
    for h in [&heard, &deaf] {
        h.begin_iteration();
    }
    emit("world.entered");
    for h in [&heard, &deaf] {
        native_events::dispatch(h).unwrap();
        h.end_iteration();
    }
    assert_eq!((global(&heard, "n"), global(&deaf, "n")), (1, 0));
}

/// The index follows `on`, `off`, `once` and a handler that subscribes or unsubscribes during delivery, and
/// events keep arriving in the order they were emitted.
#[test]
fn the_index_follows_subscribe_off_once_and_handlers_that_change_subscriptions_and_order_is_kept() {
    reset();
    let h = Host::new().unwrap();
    h.execute(
        r#"
        log = {}
        local late
        a = eqoa.events.on('world.entered', function()
            log[#log+1] = 'a'
            if not late then late = eqoa.events.on('state.reset', function() log[#log+1] = 'late' end) end
        end)
        eqoa.events.once('state.reset', function() log[#log+1] = 'once' end)
        eqoa.events.on('events.dropped', function() log[#log+1] = 'g' end)
        "#,
    )
    .unwrap();
    h.begin_iteration();
    emit("world.entered");
    emit("state.reset");
    emit("events.dropped");
    emit("state.reset");
    native_events::dispatch(&h).unwrap();
    h.end_iteration();
    let log: String = h.lua.load("return table.concat(log, ',')").eval().unwrap();
    // `once` ran for the first state.reset and left; the handler added during world.entered heard both
    assert_eq!(log, "a,once,late,g,late");
    // off: the name stops being listened for, at once
    h.execute("assert(eqoa.events.off(a))").unwrap();
    assert!(!native_events::bus(&h.lua).borrow().listens_for_test("world.entered"));
    assert!(native_events::bus(&h.lua).borrow().listens_for_test("state.reset"));
    h.execute("log = {}").unwrap();
    h.begin_iteration();
    emit("world.entered");
    native_events::dispatch(&h).unwrap();
    h.end_iteration();
    assert_eq!(h.lua.load("return #log").eval::<i64>().unwrap(), 0);
}

/// An event nobody in a VM listens for still spends that VM's 256-delivery budget (counting is as before),
/// with other VMs in the same process listening or not. (Control: counting only listened-for events lets the
/// listener see its event in the first frame.)
#[test]
fn the_delivery_budget_counts_unlistened_events_per_vm_as_before() {
    reset();
    let h = Host::new().unwrap();
    let other = Host::new().unwrap();
    h.execute("n = 0; eqoa.events.on('world.entered', function() n = n + 1 end)").unwrap();
    other.execute("eqoa.events.on('state.reset', function() end)").unwrap();
    for host in [&h, &other] {
        host.begin_iteration();
    }
    for _ in 0..256 {
        emit("state.reset");
    }
    emit("world.entered");
    for host in [&h, &other] {
        native_events::dispatch(host).unwrap();
        native_events::dispatch(host).unwrap();
    }
    assert_eq!(global(&h, "n"), 0, "256 unlistened events used the budget");
    h.end_iteration();
    other.end_iteration();
    native_events::dispatch(&h).unwrap();
    assert_eq!(global(&h, "n"), 1, "delivered in the next frame");
}

/// A flush with nothing waiting does not touch the capture core (it used to take it every call, 73 times a
/// frame). Holding the core borrowed makes the old code panic. (Control: with the early return removed this panics.)
#[test]
fn an_empty_deferred_flush_does_not_borrow_the_core() {
    reset();
    CORE.with(|c| {
        let _held = c.borrow_mut();
        native_core::flush_deferred();
    });
}

/// Prints the time (run with `--nocapture`, e.g. `run_gates.py --only round --filter perf_core_events_timing`): 14 VMs, 200
/// events a frame, a few of them listened for, 400 frames. Debug profile unless run with --release.
#[test]
fn perf_core_events_timing() {
    reset();
    let hosts: Vec<Host> = (0..14).map(|_| Host::new().unwrap()).collect();
    for (i, h) in hosts.iter().enumerate() {
        // each VM listens for 3 of the 25 names, a few listen for none
        let src = if i < 11 {
            format!(
                "n = 0; for _, name in ipairs({{'timing.e{}','timing.e{}','timing.e{}'}}) do pcall(eqoa.events.on, name, function() n = n + 1 end) end; eqoa.events.on('world.entered', function() n = n + 1 end)",
                i, i + 3, i + 6
            )
        } else {
            "n = 0".to_string()
        };
        h.execute(&src).unwrap();
    }
    let names = ["world.entered", "state.reset", "events.dropped", "player.changed", "world.left"];
    let frames = 400;
    let mut total = std::time::Duration::ZERO;
    let mut handled = 0;
    for _ in 0..frames {
        let start = Instant::now();
        for h in &hosts {
            h.begin_iteration();
        }
        for k in 0..200 {
            emit(names[k % names.len()]);
        }
        for h in &hosts {
            native_core::flush_deferred();
            native_events::dispatch(h).unwrap();
            h.end_iteration();
        }
        total += start.elapsed();
    }
    for h in &hosts {
        handled += global(h, "n");
    }
    eprintln!("perf_core_events_timing: {:.3} ms per frame (14 VMs, 200 events), handled {handled}", total.as_secs_f64() * 1000.0 / frames as f64);
    assert!(handled > 0);
}
