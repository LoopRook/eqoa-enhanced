// 2026-11-27, the owner: "if they set it up the way they like it, then reduce the ui scale, maybe it should scale with the
// ui scale". The Chat mod's box keeps the size and place the player dragged (saved in frame units) and HUD Scale multiplies
// on top, in both interfaces. Included by a6_tests.rs.

/// The mod's claimed box (window pixels), found by sampling the pointer claim, in a window of `win`.
fn chat_scale_box(vm: u64, win: (u32, u32)) -> (f64, f64, f64, f64) {
    use super::pointer_api;
    pointer_api::position([0.0, 0.0], true, win);
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    let mut y = 0.0;
    while y < win.1 as f64 {
        let mut x = 0.0;
        while x < win.0 as f64 {
            if pointer_api::point_in_vm_claim(vm, x, y) {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
            }
            x += 2.0;
        }
        y += 2.0;
    }
    (x0, y0, x1, y1)
}

#[test]
fn chat_box_follows_hud_scale_in_both_interfaces_and_saves_what_the_player_sized() {
    use super::{hud_layout, native_settings, pointer_api};
    use std::{cell::RefCell, rc::Rc};
    let mut context = context();
    type Saved = Rc<RefCell<Vec<serde_json::Value>>>;
    let rig = |context: &mut Context, pc: bool, pct: u8, win: (u32, u32), initial: Option<serde_json::Value>| -> (Host, Bridge, Saved, u64) {
        fresh();
        ships("eqoa_chat");
        crate::settings::testing::reset(false);
        crate::settings::set_ui_mode(pc).unwrap();
        crate::settings::set_hud_scale_percent(pct);
        crate::hudlayout::testing_reset();
        hud_layout::set_hud_state(true, true, false, 93, 20);
        hud_layout::set_window_size(win.0, win.1);
        context.io_mut().set_display_size([win.0 as f32, win.1 as f32]);
        let host = Host::for_addon(&addon("eqoa_chat")).unwrap();
        let bridge = Bridge::install(&host).unwrap();
        let saved: Saved = Rc::default();
        let sink = saved.clone();
        native_settings::configure(&host, initial.clone(), Rc::new(move |v| {
            sink.borrow_mut().push(v.clone());
            Ok(())
        }));
        host.run_addon(&addon("eqoa_chat")).unwrap();
        if let Some(value) = initial {
            // a place the player dragged to, as the mod keeps it (x, y, w, h in frame units)
            let n = |k: &str| value[k].as_f64().map_or("nil".to_string(), |v| v.to_string());
            host.execute(&format!(
                "chat_probe.state.lx, chat_probe.state.ly, chat_probe.state.w, chat_probe.state.h = {}, {}, {}, {}",
                n("x"), n("y"), n("w"), n("h")
            ))
            .unwrap();
        }
        let vm = native_events::bus(&host.lua).borrow().vm;
        (host, bridge, saved, vm)
    };
    let ticks = |host: &Host, bridge: &Bridge, context: &mut Context, n: usize| {
        for _ in 0..n {
            CORE.with(|c| c.borrow_mut().frame += 1);
            frames(host, bridge, context, 1);
        }
    };
    let clamp = |v: f64, lo: f64, hi: f64| v.max(lo).min(hi.max(lo));
    let mut failures = Vec::new();
    for win in [(1024u32, 768u32), (1280, 720)] {
        // the HUD frame is 4:3 inside the window: window pixels per frame pixel, the smaller of the two axes
        let fw = (win.1 as f64 * 4.0 / 3.0).min(win.0 as f64);
        let fh = if fw < win.1 as f64 * 4.0 / 3.0 { fw * 3.0 / 4.0 } else { win.1 as f64 };
        let frame_k = (fw / 640.0).min(fh / 448.0);
        // where the box sits: the default (left, bottom) and one dragged to the top right. The game keeps a piece on
        // the side it sits at and grows it about that corner, kept on screen (hudlayout.rs scale/clamp, Place::of)
        for (pc, place, right, bottom) in [
            (false, None, false, true),
            (true, None, false, true),
            (false, Some(json!({"x": 430, "y": 6, "w": 200, "h": 100})), true, false),
            (true, Some(json!({"x": 430, "y": 6, "w": 200, "h": 100})), true, false),
        ] {
            let note = |pct: u8| format!("pc={pc} win={win:?} {} scale {pct}%", if right { "top-right" } else { "default" });
            let (host, bridge, _, vm) = rig(&mut context, pc, 100, win, place.clone());
            ticks(&host, &bridge, &mut context, 3);
            let base = chat_scale_box(vm, win);
            let k0: f64 = host.lua.load("return chat_probe.state.k").eval().unwrap_or(0.0);
            // control: at 100% the numbers are today's (the window's own scale, the box's size in frame units)
            let (w, h): (f64, f64) = host.lua.load("return chat_probe.state.w, chat_probe.state.h").eval().unwrap();
            if (k0 - frame_k).abs() > 1e-3 || ((base.2 - base.0) - w * frame_k).abs() > 4.0 || ((base.3 - base.1) - h * frame_k).abs() > 4.0 {
                failures.push(format!("{}: control: at 100% k {k0} (want {frame_k}), box {base:?} vs {w}x{h}", note(100)));
            }
            drop((host, bridge));
            for pct in [75u8, 150] {
                let s = f64::from(pct) / 100.0;
                let (host, bridge, _, vm) = rig(&mut context, pc, pct, win, place.clone());
                ticks(&host, &bridge, &mut context, 3);
                let got = chat_scale_box(vm, win);
                let k: f64 = host.lua.load("return chat_probe.state.k").eval().unwrap_or(0.0);
                let (bw, bh) = ((base.2 - base.0), (base.3 - base.1));
                let (ww, wh) = (win.0 as f64, win.1 as f64);
                let want_w = bw * s;
                let want_h = bh * s;
                // the same distance from the side it sits at (left or right, top or bottom), kept inside the window
                let want_x = clamp(if right { base.2 - want_w } else { base.0 }, 0.0, ww - want_w);
                let want_y = clamp(if bottom { base.3 - want_h } else { base.1 }, 0.0, wh - want_h);
                if (k - frame_k * s).abs() > 1e-3 {
                    failures.push(format!("{}: text/box factor k {k}, want {}", note(pct), frame_k * s));
                }
                if ((got.2 - got.0) - want_w).abs() > 4.0 || ((got.3 - got.1) - want_h).abs() > 4.0 {
                    failures.push(format!("{}: claimed box {got:?} is {}x{}, want {want_w}x{want_h}", note(pct), got.2 - got.0, got.3 - got.1));
                }
                if (got.0 - want_x).abs() > 4.0 || (got.1 - want_y).abs() > 4.0 {
                    failures.push(format!("{}: claimed box at {},{} want {want_x},{want_y}", note(pct), got.0, got.1));
                }
                // the far corner is the box's, just outside it is not
                let on = |x: f64, y: f64| {
                    pointer_api::position([0.0, 0.0], true, win);
                    pointer_api::point_in_vm_claim(vm, x, y)
                };
                if !on(got.2 - 2.0, got.3 - 2.0) || on(got.2 + 8.0, got.3 + 8.0) || (got.0 > 10.0 && on(got.0 - 8.0, got.1 + 4.0)) {
                    failures.push(format!("{}: claim does not match the drawn box {got:?}", note(pct)));
                }
                drop((host, bridge));
            }
        }
    }
    // a drag-resize made at 150% saves what gives the same box back at that scale, and reads right at 100%
    let win = (1024u32, 768u32);
    for pc in [false, true] {
        let (host, bridge, saved, vm) = rig(&mut context, pc, 150, win, None);
        ticks(&host, &bridge, &mut context, 3);
        let (w0, h0): (f64, f64) = host.lua.load("return chat_probe.state.w, chat_probe.state.h").eval().unwrap();
        let b = chat_scale_box(vm, win);
        let from = (b.2 - 3.0, b.3 - 3.0);
        let to = (from.0 - 60.0, from.1 - 45.0);
        pointer_api::position([from.0 as f32, from.1 as f32], true, win);
        context.io_mut().add_mouse_pos_event([from.0 as f32, from.1 as f32]);
        CORE.with(|c| c.borrow_mut().emit("pointer.pressed", json!({"x": from.0, "y": from.1, "inside_window": true, "button": "left"})));
        context.io_mut().add_mouse_button_event(dear_imgui_rs::MouseButton::Left, true);
        ticks(&host, &bridge, &mut context, 2);
        pointer_api::position([to.0 as f32, to.1 as f32], true, win);
        context.io_mut().add_mouse_pos_event([to.0 as f32, to.1 as f32]);
        ticks(&host, &bridge, &mut context, 2);
        context.io_mut().add_mouse_button_event(dear_imgui_rs::MouseButton::Left, false);
        ticks(&host, &bridge, &mut context, 3);
        assert!(!saved.borrow().is_empty(), "the size is saved at the end of the drag");
        // what the mod keeps (and writes with every save) is its state: size and place in frame units
        let (w1, h1, lx1, ly1): (f64, f64, f64, Option<f64>) = host
            .lua
            .load("return chat_probe.state.w, chat_probe.state.h, chat_probe.state.lx, chat_probe.state.ly")
            .eval()
            .unwrap();
        let k = 1.6 * 1.5;
        if ((w1 - w0) - (-60.0 / k)).abs() > 1.5 || ((h1 - h0) - (-45.0 / k)).abs() > 1.5 {
            failures.push(format!("pc={pc}: a drag of -60,-45 px at 150% saved {w0}x{h0} -> {w1}x{h1}, want a change of {:.1},{:.1}", -60.0 / k, -45.0 / k));
        }
        drop((host, bridge));
        // read back at 100%: the saved frame size at the window's own scale
        let (host2, bridge2, _, vm2) = rig(&mut context, pc, 100, win, Some(json!({"x": lx1, "y": ly1, "w": w1, "h": h1})));
        ticks(&host2, &bridge2, &mut context, 3);
        let back = chat_scale_box(vm2, win);
        if ((back.2 - back.0) - w1 * 1.6).abs() > 4.0 || ((back.3 - back.1) - h1 * 1.6).abs() > 4.0 {
            failures.push(format!("pc={pc}: saved {w1}x{h1} at 100% drew {back:?}"));
        }
    }
    crate::settings::testing::reset(false);
    assert!(failures.is_empty(), "the chat box does not follow HUD Scale:\n{}", failures.join("\n"));
}
