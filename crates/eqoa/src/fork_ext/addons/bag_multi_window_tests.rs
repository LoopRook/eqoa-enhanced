// Bag Viewer multi-select through the real window (included in manager tests): real pointer events with the modifiers held,
// the blue outline, the group menu, and what closing the window and zoning do to the selection. Needs a GPU adapter (counted
// as a SKIP without one); the logic of the actions is tested without one in `bag_multi_tests.rs`.
impl Bv {
    /// A click of `button` at `p` with Ctrl and/or Shift held (the window's own modifier state, as the client keeps it).
    fn click_with(&mut self, p: (f64, f64), button: winit::event::MouseButton, ctrl: bool, shift: bool) {
        use winit::event::{DeviceId, ElementState, WindowEvent};
        super::super::input::set_modifiers((ctrl, false, shift));
        self.point(p);
        for state in [ElementState::Pressed, ElementState::Released] {
            let ev = WindowEvent::MouseInput { device_id: DeviceId::dummy(), state, button };
            super::super::pointer_api::observe(&ev, self.win);
            self.frame();
        }
        super::super::input::set_modifiers((false, false, false));
    }
    fn ctrl_click(&mut self, n: usize) {
        let p = self.slot(n);
        self.click_with(p, winit::event::MouseButton::Left, true, false);
    }
    fn shift_click(&mut self, n: usize) {
        let p = self.slot(n);
        self.click_with(p, winit::event::MouseButton::Left, false, true);
    }
    fn right_click(&mut self, n: usize) {
        let p = self.slot(n);
        self.click_with(p, winit::event::MouseButton::Right, false, false);
    }
    fn selected(&self) -> Vec<String> {
        self.eval("(function() local X, t = require('multi'), {} for _, e in ipairs(require('bags').grid.items) do if X.is_selected(e.item) then t[#t + 1] = e.item.name end end return t end)()")
    }
    fn menu_title(&self) -> Option<String> {
        self.eval("(function() local m = require('bags').menu return m and m.title or nil end)()")
    }
}

#[test]
fn bag_multi_select_with_real_clicks_shows_a_blue_outline_opens_one_menu_and_clears_with_the_window_and_the_zone() {
    big_stack(bag_multi_select_with_real_clicks_body)
}
fn bag_multi_select_with_real_clicks_body() {
    use winit::keyboard::{Key, NamedKey};
    super::super::action_broker::reset_all_for_test();
    let Some(mut bv) = Bv::new() else { return };
    bv.play.bags_connect();
    bv.enable();
    bv.open();
    // Ctrl+click selects (no menu), outlines the slot in blue, and Ctrl+click again takes it out
    bv.ctrl_click(3);
    assert_eq!(bv.selected(), ["Iron Ring"]);
    assert!(bv.menu_labels().is_empty(), "selecting opens no menu");
    let tex = bv.frames(2);
    let px = rm_pixels(&bv.gpu, &tex);
    let blue = [115, 178, 255, 255];
    let edge = |bv: &Bv, n: usize| bv.at(8. + (n - 1) as f64 * 38., 46. + 18.);
    assert!(bv_near(&px, bv.win.0, edge(&bv, 3), 2, blue, 30), "the selected slot has a blue outline");
    assert!(!bv_near(&px, bv.win.0, edge(&bv, 4), 2, blue, 30), "and the next one does not");
    bv.ctrl_click(5);
    assert_eq!(bv.selected(), ["Iron Ring", "Elf Cloak"]);
    bv.ctrl_click(3);
    assert_eq!(bv.selected(), ["Elf Cloak"]);
    // Shift+click takes every slot from the one clicked last (3, whose Ctrl+click took it out) to this one
    bv.shift_click(8);
    assert_eq!(bv.selected(), ["Iron Ring", "Mage Robe", "Elf Cloak", "Hero Helm", "Rat Pelt", "Odd Thing"]);
    // right-click on an item that is not selected opens nothing; on a selected one it opens the group menu
    bv.right_click(1);
    assert!(bv.menu_labels().is_empty(), "{:?}", bv.menu_labels());
    bv.right_click(6);
    assert_eq!(bv.menu_title().as_deref(), Some("6 items selected"));
    let labels = bv.menu_labels();
    for want in ["Mark as junk", "Unmark", "Mark to bank", "Sell (6)", "Put in bank (6)", "Destroy..."] {
        assert!(labels.iter().any(|l| l == want), "{want} in {labels:?}");
    }
    let (_, _, sell_on, sell_why, _) = bv.menu_row("Sell").unwrap();
    assert!(!sell_on && sell_why.as_deref() == Some("no merchant nearby"), "{sell_why:?}");
    bv.key(Key::Named(NamedKey::Escape));
    assert!(bv.is_open() && bv.menu_labels().is_empty(), "Escape closes the menu first");
    assert_eq!(bv.selected().len(), 6, "and keeps the selection");
    // a plain click works as before and clears the selection
    let p = bv.slot(2);
    bv.click(p);
    assert!(bv.selected().is_empty());
    assert_eq!(bv.menu_title().as_deref(), Some("Healing Potion"));
    bv.key(Key::Named(NamedKey::Escape));
    // closing the window drops the selection
    bv.ctrl_click(3);
    assert_eq!(bv.selected().len(), 1);
    bv.toggle();
    assert!(!bv.is_open());
    assert_eq!(bv.eval::<i64>("require('multi').n"), 0, "closing the window clears the selection");
    bv.toggle();
    assert!(bv.is_open());
    assert!(bv.selected().is_empty());
    // zoning does too
    bv.ctrl_click(3);
    bv.ctrl_click(4);
    assert_eq!(bv.selected().len(), 2);
    bv.play.bags_zone();
    bv.frames(2);
    assert_eq!(bv.eval::<i64>("require('multi').n"), 0, "zoning clears the selection");
    bv.clean();
    super::super::action_broker::reset_all_for_test();
}
