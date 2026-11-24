// LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE).
//! Click-through: one click goes to the front-most window under the pointer and to nothing behind it. Two real
//! bundled mods (Spellbook, Quest Journal) overlap in the middle of the screen; a scratch mod draws a real ImGui
//! window. Each test has a control that runs the same steps with the window order switched off
//! (`pointer_api::set_legacy_for_test`: the behaviour before windows had an order) and sees the click-through.
use super::{
    a1_mod_tests as h,
    bridge::Bridge,
    discovery::DiscoveredAddon,
    host::Host,
    native_events, pointer_api, pointer_order, window_sessions as ws,
};
use dear_imgui_rs::Context;
use std::path::PathBuf;
use winit::event::{DeviceId, ElementState, MouseButton, WindowEvent};

/// The window the tests run in (the HUD frame, where the bundled windows draw at scale 1).
const SIZE: (u32, u32) = (640, 448);

struct Mod {
    host: Host,
    bridge: Bridge,
    vm: u64,
    dir: Option<PathBuf>,
}

impl Mod {
    fn real(id: &str) -> Mod {
        Self::start(h::addon(id), None)
    }
    fn scratch(name: &str, source: &str) -> Mod {
        let dir = std::env::temp_dir().join(format!("eqoa-click-through-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("main.lua"), source).unwrap();
        let dir = dir.canonicalize().unwrap();
        let addon = DiscoveredAddon {
            id: name.into(),
            name: name.into(),
            entry: dir.join("main.lua"),
            script_root: dir.clone(),
            package_root: dir.clone(),
            module_dirs: vec![dir.clone()],
            resource_dirs: vec![dir.clone()],
            api: Some("eqoa/1".into()),
            entry_mode: super::discovery::EntryMode::Once,
            capabilities_declared: true,
            requested_capabilities: ["eqoa.events.on", "eqoa.imgui", "eqoa.input.pointer", "eqoa.input.claim_pointer"]
                .iter()
                .map(|c| c.to_string())
                .collect(),
            extensions: Default::default(),
            initially_enabled: false,
            mods_root: None,
        };
        Self::start(addon, Some(dir))
    }
    fn start(addon: DiscoveredAddon, dir: Option<PathBuf>) -> Mod {
        let host = Host::for_addon(&addon).unwrap();
        let bridge = Bridge::install(&host).unwrap();
        host.run_addon(&addon).unwrap();
        let vm = native_events::bus(&host.lua).borrow().vm;
        Mod { host, bridge, vm, dir }
    }
    fn clicks(&self) -> usize {
        native_events::pending_pointer_events(&self.host)
    }
    fn lua_bool(&self, code: &str) -> bool {
        self.host.lua.load(code).eval().unwrap()
    }
}

impl Drop for Mod {
    fn drop(&mut self) {
        native_events::clear(&self.host);
        if let Some(dir) = &self.dir {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

/// One frame the way the runtime draws it, every mod in turn; the name of the window ImGui says the pointer is
/// on, as the mods saw it while they drew.
fn frame(mods: &[&Mod], context: &mut Context) -> String {
    let ui = context.frame();
    let mut hovered = String::new();
    super::runtime::with_question_on_top(ui, || {
        for m in mods {
            m.bridge
                .invoke(|| {
                    native_events::dispatch(&m.host)?;
                    native_events::render(&m.host)
                })
                .unwrap();
        }
        hovered = hovered_window();
    });
    drop(context.render_legacy());
    hovered
}

fn frames(mods: &[&Mod], context: &mut Context, n: usize) {
    for _ in 0..n {
        frame(mods, context);
    }
}

fn hovered_window() -> String {
    unsafe {
        let g = dear_imgui_rs::sys::igGetCurrentContext();
        let w = (*g).HoveredWindow;
        if w.is_null() { String::new() } else { std::ffi::CStr::from_ptr((*w).Name).to_string_lossy().into_owned() }
    }
}

/// Where a window is in ImGui's display order (higher is in front).
fn z(name: &str) -> usize {
    let cname = std::ffi::CString::new(name).unwrap();
    let id = unsafe {
        let w = dear_imgui_rs::sys::igFindWindowByName(cname.as_ptr());
        assert!(!w.is_null(), "{name} was drawn");
        (*w).ID
    };
    pointer_order::z_of(id).unwrap_or_else(|| panic!("{name} is in ImGui's window list"))
}

fn rect(vm: u64) -> [f32; 4] {
    let claims = pointer_api::claim_rects_for_test(vm);
    assert!(!claims.is_empty(), "the mod claimed its window");
    claims[0]
}

fn inside(r: &[f32; 4], x: f32, y: f32) -> bool {
    x >= r[0] && x <= r[0] + r[2] && y >= r[1] && y <= r[1] + r[3]
}

/// A point on the screen that `keep` accepts and `reject` refuses, found on a coarse grid.
fn find_point(keep: impl Fn(f32, f32) -> bool) -> (f32, f32) {
    for y in (0..SIZE.1).step_by(4) {
        for x in (0..SIZE.0).step_by(4) {
            if keep(x as f32 + 0.5, y as f32 + 0.5) {
                return (x as f32 + 0.5, y as f32 + 0.5);
            }
        }
    }
    panic!("no such point on the screen");
}

fn move_to(x: f32, y: f32, context: &mut Context) {
    pointer_api::observe(
        &WindowEvent::CursorMoved {
            device_id: DeviceId::dummy(),
            position: winit::dpi::PhysicalPosition::new(x as f64, y as f64),
        },
        SIZE,
    );
    context.io_mut().add_mouse_pos_event([x, y]);
}

/// A left press at the pointer; `true` when the claim kept it from the game.
fn press(x: f32, y: f32, context: &mut Context) -> bool {
    press_button(x, y, MouseButton::Left, context)
}

fn press_button(x: f32, y: f32, button: MouseButton, context: &mut Context) -> bool {
    move_to(x, y, context);
    pointer_api::observe(
        &WindowEvent::MouseInput { device_id: DeviceId::dummy(), state: ElementState::Pressed, button },
        SIZE,
    )
}

fn wheel(x: f32, y: f32, context: &mut Context) {
    move_to(x, y, context);
    pointer_api::observe(
        &WindowEvent::MouseWheel {
            device_id: DeviceId::dummy(),
            delta: winit::event::MouseScrollDelta::LineDelta(0.0, 1.0),
            phase: winit::event::TouchPhase::Moved,
        },
        SIZE,
    );
}

/// Drain what the mods were told, as the runtime's dispatch does.
fn deliver(mods: &[&Mod]) {
    for m in mods {
        native_events::dispatch(&m.host).unwrap();
    }
}

fn setup() -> Context {
    h::fixture();
    // the layout the bundled windows draw in (another test may have left a different window size or Move HUD on)
    super::hud_layout::set_window_size(640, 448);
    crate::hudlayout::set_unlocked(false);
    pointer_api::set_legacy_for_test(false);
    pointer_api::position([0.0, 0.0], false, SIZE);
    h::context()
}

/// Spellbook, then Quest Journal opened over it: both claim the middle of the screen, Quest Journal in front.
fn two_windows(context: &mut Context) -> (Mod, Mod) {
    let back = Mod::real("eqoa_spellbook");
    let front = Mod::real("eqoa_quest_journal");
    h::press(&back.host, "k", "shift+k");
    frames(&[&back, &front], context, 3);
    h::press(&front.host, "j", "shift+j");
    frames(&[&back, &front], context, 4);
    assert!(ws::get(back.vm, "spellbook").is_some() && ws::get(front.vm, "quest_log").is_some());
    assert!(back.host.drain_logs().unwrap().is_empty() && front.host.drain_logs().unwrap().is_empty());
    (back, front)
}

#[test]
fn a_click_on_the_overlap_of_two_real_mod_windows_reaches_only_the_front_one() {
    let mut context = setup();
    let (back, front) = two_windows(&mut context);
    let (rb, rf) = (rect(back.vm), rect(front.vm));
    let overlap = find_point(|x, y| inside(&rb, x, y) && inside(&rf, x, y));
    assert!(
        z("eqoa_quest_journal_layer") > z("eqoa_spellbook_layer"),
        "the window opened last is drawn in front"
    );
    deliver(&[&back, &front]);

    // the click on the overlap: the front window gets it, the one behind it and the game do not
    let kept = press(overlap.0, overlap.1, &mut context);
    assert!(kept, "the claimed windows keep the click from the game");
    assert_eq!((front.clicks(), back.clicks()), (1, 0), "only the front window hears the press");
    wheel(overlap.0, overlap.1, &mut context);
    assert_eq!((front.clicks(), back.clicks()), (2, 0), "and the wheel");
    deliver(&[&back, &front]);

    // hover is the front window's too: the one behind sees the pointer outside itself
    assert!(front.lua_bool("return eqoa.input.pointer().inside_window"));
    assert!(!back.lua_bool("return eqoa.input.pointer().inside_window"), "no hover or tooltip behind the front window");

    // control: before the windows had an order, both heard it (this is the bug the owner saw)
    pointer_api::set_legacy_for_test(true);
    press(overlap.0, overlap.1, &mut context);
    assert_eq!((front.clicks(), back.clicks()), (1, 1), "control: the window behind got the click through the front one");
    assert!(back.lua_bool("return eqoa.input.pointer().inside_window"), "control: and its hover");
    pointer_api::set_legacy_for_test(false);
    deliver(&[&back, &front]);
}

#[test]
fn a_click_outside_the_overlap_still_reaches_the_window_behind_and_a_click_on_no_window_reaches_the_game() {
    let mut context = setup();
    let (back, front) = two_windows(&mut context);
    let (rb, rf) = (rect(back.vm), rect(front.vm));
    deliver(&[&back, &front]);
    // the Quest Journal sits inside the Spellbook's rectangle: a point on the Spellbook alone is above or below it
    let only_back = find_point(|x, y| inside(&rb, x, y) && !inside(&rf, x, y));
    let overlap = find_point(|x, y| inside(&rb, x, y) && inside(&rf, x, y));
    assert!(press(only_back.0, only_back.1, &mut context));
    assert_eq!((back.clicks(), front.clicks()), (1, 0), "the window behind still hears a click on its own part");
    deliver(&[&back, &front]);
    assert!(back.lua_bool("return eqoa.input.pointer().inside_window"), "and shows its hover there");
    assert!(press(overlap.0, overlap.1, &mut context));
    assert_eq!((back.clicks(), front.clicks()), (0, 1), "and the overlap is still the front window's alone");
    deliver(&[&back, &front]);
    // no window there: both mods hear it as they always did and the game gets it
    let nowhere = find_point(|x, y| !inside(&rb, x, y) && !inside(&rf, x, y));
    assert!(!press(nowhere.0, nowhere.1, &mut context), "a click on no window is the game's");
    assert_eq!((back.clicks(), front.clicks()), (1, 1), "mods that listen to every click still do");
    deliver(&[&back, &front]);
}

#[test]
fn a_press_on_a_window_behind_brings_it_to_the_front() {
    let mut context = setup();
    let (back, front) = two_windows(&mut context);
    let (rb, rf) = (rect(back.vm), rect(front.vm));
    let overlap = find_point(|x, y| inside(&rb, x, y) && inside(&rf, x, y));
    let only_back = find_point(|x, y| inside(&rb, x, y) && !inside(&rf, x, y));
    deliver(&[&back, &front]);
    let before = (z("eqoa_spellbook_layer"), z("eqoa_quest_journal_layer"));
    assert!(before.0 < before.1);
    // control: with the order off, the press does not raise it
    pointer_api::set_legacy_for_test(true);
    press(only_back.0, only_back.1, &mut context);
    frames(&[&back, &front], &mut context, 2);
    assert!(z("eqoa_spellbook_layer") < z("eqoa_quest_journal_layer"), "control: nothing was raised");
    pointer_api::set_legacy_for_test(false);
    deliver(&[&back, &front]);
    // the press on the window behind raises it: now the overlap is its own
    press(only_back.0, only_back.1, &mut context);
    assert!(pointer_order::raise_pending().is_some(), "the press marked the window to raise");
    deliver(&[&back, &front]);
    frames(&[&back, &front], &mut context, 2);
    assert!(z("eqoa_spellbook_layer") > z("eqoa_quest_journal_layer"), "the pressed window came to the front");
    press(overlap.0, overlap.1, &mut context);
    assert_eq!((back.clicks(), front.clicks()), (1, 0), "the overlap now belongs to the window that was raised");
    deliver(&[&back, &front]);
}

/// A mod window ImGui itself draws and hit-tests (no layer, real title bar and input). Its place is read from the
/// globals `wx, wy, ww, wh` every frame, so a test can put it where the claimed window it uses is.
const IMGUI_WINDOW: &str = r#"
    local ui = eqoa.imgui
    wx, wy, ww, wh = 20, 20, 200, 200
    eqoa.events.on("render", function()
        ui.SetNextWindowPos(wx, wy, ImGuiCond.Always)
        ui.SetNextWindowSize(ww, wh, ImGuiCond.Always)
        ui.Begin("click_through_imgui_window", true, ImGuiWindowFlags.NoSavedSettings + ImGuiWindowFlags.NoMove + ImGuiWindowFlags.NoResize)
        hovered = ui.IsWindowHovered()
        ui.Button("hold")
        ui.End()
    end)
"#;

/// Puts the scratch ImGui window where it overlaps the right edge of the claimed rectangle `book` by 60 pixels
/// and sticks out beyond it (`beyond`: whether it starts past the edge instead), and returns its rectangle.
fn place(window: &Mod, book: &[f32; 4], beyond: bool) -> [f32; 4] {
    let r = if beyond {
        [book[0] + book[2] + 10.0, book[1] + 60.0, 150.0, 100.0]
    } else {
        [book[0] + book[2] - 60.0, book[1] + 100.0, 200.0, 200.0]
    };
    window
        .host
        .lua
        .load(format!("wx, wy, ww, wh = {}, {}, {}, {}", r[0], r[1], r[2], r[3]))
        .exec()
        .unwrap();
    r
}

#[test]
fn an_imgui_window_in_front_of_a_custom_drawn_one_takes_the_click_alone() {
    let mut context = setup();
    let book = Mod::real("eqoa_spellbook");
    let window = Mod::scratch("imgui_front", IMGUI_WINDOW);
    h::press(&book.host, "k", "shift+k");
    frames(&[&book], &mut context, 4);
    let rb = rect(book.vm);
    // the ImGui window is drawn for the first time after the book: it is in front of it
    let rw = place(&window, &rb, false);
    frames(&[&book, &window], &mut context, 4);
    assert!(z("click_through_imgui_window") > z("eqoa_spellbook_layer"), "the ImGui window opened last is in front");
    let overlap = find_point(|x, y| inside(&rb, x, y) && inside(&rw, x, y));
    let only_book = find_point(|x, y| inside(&rb, x, y) && !inside(&rw, x, y));
    deliver(&[&book, &window]);
    press(overlap.0, overlap.1, &mut context);
    assert_eq!(book.clicks(), 0, "the click on the ImGui window does not reach the custom-drawn window behind it");
    assert!(!book.lua_bool("return eqoa.input.pointer().inside_window"), "no hover behind it either");
    deliver(&[&book, &window]);
    // outside the ImGui window the book still has its click
    press(only_book.0, only_book.1, &mut context);
    assert_eq!(book.clicks(), 1);
    deliver(&[&book, &window]);
    // control
    pointer_api::set_legacy_for_test(true);
    press(overlap.0, overlap.1, &mut context);
    assert_eq!(book.clicks(), 1, "control: the book behind the ImGui window got the click");
    pointer_api::set_legacy_for_test(false);
    deliver(&[&book, &window]);
}

#[test]
fn a_custom_drawn_window_in_front_of_an_imgui_one_hides_the_pointer_from_it() {
    let mut context = setup();
    let window = Mod::scratch("imgui_back", IMGUI_WINDOW);
    let book = Mod::real("eqoa_spellbook");
    frames(&[&window, &book], &mut context, 3);
    h::press(&book.host, "k", "shift+k");
    frames(&[&window, &book], &mut context, 4);
    let rb = rect(book.vm);
    let rw = place(&window, &rb, false);
    frames(&[&window, &book], &mut context, 2);
    assert!(z("eqoa_spellbook_layer") > z("click_through_imgui_window"), "the book opened last is in front");
    let overlap = find_point(|x, y| inside(&rb, x, y) && inside(&rw, x, y));
    let only_window = find_point(|x, y| inside(&rw, x, y) && !inside(&rb, x, y));
    deliver(&[&window, &book]);
    // the pointer on the book: the ImGui window under it has no hover, no click
    move_to(overlap.0, overlap.1, &mut context);
    frame(&[&window, &book], &mut context); // ImGui takes the position
    let hovered = frame(&[&window, &book], &mut context);
    assert_eq!(hovered, "", "the window behind the book does not see the pointer");
    assert!(!window.lua_bool("return hovered"), "nor does its own hover test");
    // on the ImGui window's own part it is hovered as usual
    move_to(only_window.0, only_window.1, &mut context);
    frame(&[&window, &book], &mut context);
    let hovered = frame(&[&window, &book], &mut context);
    assert_eq!(hovered, "click_through_imgui_window", "the part the book does not cover still takes the pointer");
    assert!(window.lua_bool("return hovered"));
    // control: with the order off the ImGui window under the book is hovered
    pointer_api::set_legacy_for_test(true);
    move_to(overlap.0, overlap.1, &mut context);
    frame(&[&window, &book], &mut context);
    let hovered = frame(&[&window, &book], &mut context);
    assert_eq!(hovered, "click_through_imgui_window", "control: the pointer reached the window behind the book");
    pointer_api::set_legacy_for_test(false);
}

#[test]
fn a_drag_that_began_on_an_imgui_window_is_kept_when_it_passes_under_a_claimed_one() {
    let mut context = setup();
    let window = Mod::scratch("imgui_drag", IMGUI_WINDOW);
    let book = Mod::real("eqoa_spellbook");
    frames(&[&window, &book], &mut context, 3);
    h::press(&book.host, "k", "shift+k");
    frames(&[&window, &book], &mut context, 4);
    let rb = rect(book.vm);
    // the window sits just beside the book; hold its button (the top left of the window) ...
    let rw = place(&window, &rb, true);
    frames(&[&window, &book], &mut context, 2);
    let start = (rw[0] + 20.0, rw[1] + 35.0);
    assert!(!inside(&rb, start.0, start.1));
    move_to(start.0, start.1, &mut context);
    frames(&[&window, &book], &mut context, 2);
    context.io_mut().add_mouse_button_event(dear_imgui_rs::input::MouseButton::Left, true);
    frames(&[&window, &book], &mut context, 2);
    let held = unsafe { (*dear_imgui_rs::sys::igGetCurrentContext()).ActiveId };
    assert_ne!(held, 0, "the button is held");
    // ... and drag on under the book
    let under = (rb[0] + rb[2] - 50.0, start.1);
    assert!(inside(&rb, under.0, under.1));
    move_to(under.0, under.1, &mut context);
    frames(&[&window, &book], &mut context, 2);
    let active = unsafe { (*dear_imgui_rs::sys::igGetCurrentContext()).ActiveId };
    assert_eq!(active, held, "the button is still held under the book");
    context.io_mut().add_mouse_button_event(dear_imgui_rs::input::MouseButton::Left, false);
    frames(&[&window, &book], &mut context, 2);
}

#[test]
fn the_bag_viewers_item_menu_over_another_window_takes_left_and_right_clicks_alone() {
    let mut context = setup();
    let book = Mod::real("eqoa_spellbook");
    let bag = Mod::real("eqoa_bag_viewer");
    h::press(&book.host, "k", "shift+k");
    frames(&[&book, &bag], &mut context, 3);
    h::press(&bag.host, "b", "shift+b");
    // an empty pack: the bag has no inventory to read in this fixture, so it is given one empty row to draw
    bag.host
        .lua
        .load("local B = require('bags'); B.is_grid_stale = function() return false end; B.grid = {rows = 1, total_rows = 1, items = {}, money = 0, used = 0, capacity = 8}")
        .exec()
        .unwrap();
    frames(&[&book, &bag], &mut context, 4);
    assert!(ws::get(bag.vm, "inventory").is_some(), "the Bag Viewer opened");
    assert!(z("eqoa_bag_viewer_layer") > z("eqoa_spellbook_layer"), "the Bag Viewer opened last is in front");
    // the menu an item's click builds (its rows are the bag's own: one use, one greyed), drawn on the next frame
    bag.host
        .lua
        .load("local B = require('bags'); B.menu = {title = 'Test item', rows = {{label = 'Use', text = 'Use', enabled = true}, {label = 'Destroy', enabled = false, reason = 'asks first'}}, fx = 8, fy = 46}")
        .exec()
        .unwrap();
    frames(&[&book, &bag], &mut context, 2);
    let (mx, my, mw, mh): (f32, f32, f32, f32) =
        bag.host.lua.load("local r = require('bags').menu_rect; return r.x, r.y, r.w, r.h").eval().unwrap();
    let (rb, rbook) = (rect(bag.vm), rect(book.vm));
    let menu = [mx, my, mw, mh];
    assert!(
        inside(&rb, mx, my) && inside(&rb, mx + mw, my + mh),
        "the menu is drawn inside the Bag Viewer's claimed window ({menu:?} in {rb:?}), so the claim covers it"
    );
    let row = (mx + mw * 0.5, my + 24.0);
    assert!(inside(&rbook, row.0, row.1), "a menu row is over the Spellbook too");
    deliver(&[&book, &bag]);
    for button in [MouseButton::Left, MouseButton::Right] {
        assert!(press_button(row.0, row.1, button, &mut context));
        assert_eq!((bag.clicks(), book.clicks()), (1, 0), "{button:?}: only the menu's window hears the click");
        deliver(&[&book, &bag]);
        // the menu is still there for the next click (a right-click closes it: put it back)
        bag.host
            .lua
            .load("local B = require('bags'); B.menu = B.menu or {title = 'Test item', rows = {{label = 'Use', text = 'Use', enabled = true}, {label = 'Destroy', enabled = false, reason = 'asks first'}}, fx = 8, fy = 46}")
            .exec()
            .unwrap();
        frames(&[&book, &bag], &mut context, 2);
    }
    // the Spellbook shows no hover under the menu
    move_to(row.0, row.1, &mut context);
    assert!(!book.lua_bool("return eqoa.input.pointer().inside_window"));
    // control: before the order, the Spellbook heard the menu's clicks too
    pointer_api::set_legacy_for_test(true);
    press_button(row.0, row.1, MouseButton::Right, &mut context);
    assert_eq!((bag.clicks(), book.clicks()), (1, 1), "control: the click went through the menu to the Spellbook");
    pointer_api::set_legacy_for_test(false);
    deliver(&[&book, &bag]);
}
