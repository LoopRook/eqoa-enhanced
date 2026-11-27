// LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE).
// 2026-11-27h, the owner's play of dev 4936f2c0: "I clicked in the auction window and it seems to interact with the addon
// menu if it's behind it." The client's own window (Mods & Addons) was built BEFORE the pointer was settled for the frame
// (`pointer_order::begin_frame`), so a click on a mod window drawn in front of it also pressed what lay under it in Mods &
// Addons. The runtime now builds every frame's windows through `pointer_order::windows_in_order`: the pointer first, then the
// client's windows, then the mods'. Included by click_through_tests.rs (its real Spellbook window, its frame and press
// helpers). The Spellbook stands in for the Auction: both draw a claimed window in their own layer (`claim_pointer` inside
// `ImGui.Begin`), which is what the order reads.

/// A real ImGui window of the client's, built in the frame's client part as Mods & Addons is: one big button. Its place and
/// size are its own once made (FirstUseEver), so a click can bring it to the front.
const WO_CLIENT: [f32; 4] = [40.0, 40.0, 560.0, 380.0];
/// Inside the client window's button.
const WO_BUTTON: [f32; 4] = [70.0, 90.0, 480.0, 280.0];

/// One frame as the runtime builds it (`settle_first`: `windows_in_order`, the order now), or as it was built before
/// (`settle_first = false`: the client's window first, then the pointer settled with the mods'). `true` when the client
/// window's button was pressed in this frame.
fn wo_frame(mods: &[&Mod], context: &mut Context, settle_first: bool) -> bool {
    let ui: &dear_imgui_rs::Ui = context.frame();
    let mut clicked = false;
    let client = |ui: &dear_imgui_rs::Ui, clicked: &mut bool| {
        ui.window("Mods & Addons stand-in##owner1127h")
            .position([WO_CLIENT[0], WO_CLIENT[1]], dear_imgui_rs::Condition::FirstUseEver)
            .size([WO_CLIENT[2], WO_CLIENT[3]], dear_imgui_rs::Condition::FirstUseEver)
            .build(|| {
                if ui.button_with_size("Everything here is one button##owner1127h", [WO_BUTTON[2] + 40.0, WO_BUTTON[3] + 60.0]) {
                    *clicked = true;
                }
            });
    };
    let draw_mods = |_: &dear_imgui_rs::Ui, _: &mut bool| {
        for m in mods {
            m.bridge
                .invoke(|| {
                    native_events::dispatch(&m.host)?;
                    native_events::render(&m.host)
                })
                .unwrap();
        }
    };
    if settle_first {
        super::pointer_order::windows_in_order(ui, &mut clicked, client, draw_mods);
    } else {
        client(ui, &mut clicked);
        super::runtime::with_question_on_top(ui, || draw_mods(ui, &mut false));
    }
    drop(context.render_legacy());
    clicked
}

/// A left click at a point as the player makes it: the window's own press (`pointer_api`, what the mods hear) and ImGui's
/// button down and up over two frames. What the mod heard of it, and whether the client window's button was pressed.
fn wo_click(at: (f32, f32), book: &Mod, mods: &[&Mod], context: &mut Context, settle_first: bool) -> (usize, bool) {
    deliver(mods);
    press(at.0, at.1, context);
    let heard = book.clicks();
    context.io_mut().add_mouse_button_event(dear_imgui_rs::MouseButton::Left, true);
    let mut pressed = wo_frame(mods, context, settle_first);
    context.io_mut().add_mouse_button_event(dear_imgui_rs::MouseButton::Left, false);
    pressed |= wo_frame(mods, context, settle_first);
    pressed |= wo_frame(mods, context, settle_first);
    (heard, pressed)
}

/// The client window first, then the Spellbook opened over it (as the Auction was over Mods & Addons).
fn wo_setup(settle_first: bool) -> (Context, Mod) {
    let mut context = setup();
    let book = Mod::real("eqoa_spellbook");
    for _ in 0..3 {
        wo_frame(&[&book], &mut context, settle_first);
    }
    h::press(&book.host, "k", "shift+k");
    for _ in 0..4 {
        wo_frame(&[&book], &mut context, settle_first);
    }
    assert!(ws::get(book.vm, "spellbook").is_some(), "control: the Spellbook opened over the client's window");
    (context, book)
}

/// A click on a mod window drawn in front of the client's own window (Mods & Addons) is the mod window's alone: the mod hears
/// it, and the button of the client's window under it is not pressed. Control (the order the runtime used before): the same
/// click also pressed the client's button.
#[test]
fn owner1127h_a_click_on_a_mod_window_in_front_of_mods_and_addons_does_not_press_what_is_behind() {
    for settle_first in [true, false] {
        let (mut context, book) = wo_setup(settle_first);
        let rb = rect(book.vm);
        let at = find_point(|x, y| inside(&rb, x, y) && inside(&WO_BUTTON, x, y));
        let (heard, pressed) = wo_click(at, &book, &[&book], &mut context, settle_first);
        assert_eq!(heard, 1, "the Spellbook, in front, hears the click (settle first: {settle_first})");
        if settle_first {
            assert!(!pressed, "the client's window behind the Spellbook is not pressed");
        } else {
            assert!(pressed, "control: built before the pointer was settled, the client's window behind was pressed too");
        }
    }
}

/// The reverse: the player brings the client's window to the front (a click on it where the Spellbook does not cover it),
/// then clicks where the two overlap: the client's button is pressed and the Spellbook, behind, hears nothing.
#[test]
fn owner1127h_with_mods_and_addons_in_front_a_click_is_its_alone() {
    let (mut context, book) = wo_setup(true);
    let rb = rect(book.vm);
    let off = find_point(|x, y| inside(&WO_BUTTON, x, y) && !inside(&rb, x, y));
    // (a point no mod window claims is told to every mod, as every click on the world is: the Spellbook ignores it)
    let (_, pressed) = wo_click(off, &book, &[&book], &mut context, true);
    assert!(pressed, "control: a click on the client's window where nothing covers it presses it, and brings it to the front");
    let at = find_point(|x, y| inside(&rb, x, y) && inside(&WO_BUTTON, x, y));
    let (heard, pressed) = wo_click(at, &book, &[&book], &mut context, true);
    assert!(pressed, "the client's window, now in front, is pressed");
    assert_eq!(heard, 0, "and the Spellbook behind it hears nothing");
}

/// The runtime builds its frame's windows through `windows_in_order` (the pointer settled before Mods & Addons is built), so
/// the tests above drive the order the player gets. Negative control: before, `render_managed` called `controls(...)` (Mods &
/// Addons) before `with_question_on_top` settled the pointer.
#[test]
fn owner1127h_the_runtime_builds_mods_and_addons_after_the_pointer_is_settled() {
    let src = include_str!("runtime.rs").replace("\r\n", "\n");
    let start = src.find("fn render_managed(").expect("render_managed");
    let body = &src[start..start + src[start..].find("\nfn ").unwrap_or(src.len() - start)];
    let order = body.find("pointer_order::windows_in_order(").expect("render_managed builds its windows through windows_in_order");
    let client = body.find("controls(ui, manager").expect("Mods & Addons is built in render_managed");
    assert!(order < client, "Mods & Addons is built inside windows_in_order, after the pointer is settled");
    assert!(!body[..order].contains("ui.window("), "no window is built before the pointer is settled");
}
