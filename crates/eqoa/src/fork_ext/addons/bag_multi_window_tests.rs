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
    for want in ["Mark as junk", "Unmark junk", "Mark to bank", "Sell (6)", "Put in bank (6)", "Destroy..."] {
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

impl Bv {
    /// The corner letter the window draws for the stack with this name ("B" marked to bank), from the mod's own function.
    fn corner(&self, name: &str) -> Option<String> {
        self.eval(&format!(
            "(function() local B = require('bags') for _, e in ipairs(B.grid.items) do if e.item.name == {name:?} then return B.mark_letter(e.item) end end end)()"
        ))
    }
    fn saved_bank_marks(&self) -> bool {
        self.eval("((eqoa.files.read('helpers.cfg') or ''):find('bank:', 1, true)) ~= nil")
    }
}

/// The owner's bug (build 4b4626ac): a bank mark could be put on but not taken off. With the real window and real clicks:
/// the single menu and the group menu both offer "Unmark bank", the mark is saved, and its corner letter goes at once.
#[test]
fn bag_a_bank_mark_can_be_taken_off_from_the_item_menu_and_the_group_menu_with_real_clicks() {
    big_stack(bag_a_bank_mark_can_be_taken_off_body)
}
fn bag_a_bank_mark_can_be_taken_off_body() {
    super::super::action_broker::reset_all_for_test();
    let Some(mut bv) = Bv::new() else { return };
    bv.play.bags_connect();
    bv.enable();
    bv.open();
    // slots: 6 Hero Helm, 7 Rat Pelt, 8 Odd Thing. Mark the pelt to bank from its own menu.
    let p = bv.slot(7);
    bv.click(p);
    bv.frames(2);
    assert!(bv.menu_row("Unmark bank").is_none(), "nothing to unmark yet");
    let (x, y, on, _, _) = bv.menu_row("Bank this").expect("Bank this is offered");
    assert!(on);
    bv.click((x, y));
    bv.frames(2);
    assert_eq!(bv.corner("Rat Pelt").as_deref(), Some("B"));
    assert!(bv.saved_bank_marks());
    // its menu now offers Unmark bank (and no longer Bank this); the click takes the mark off at once and saves that
    let p = bv.slot(7);
    bv.click(p);
    bv.frames(2);
    assert!(bv.menu_row("Bank this").is_none(), "{:?}", bv.menu_labels());
    let (x, y, on, _, _) = bv.menu_row("Unmark bank").expect("Unmark bank is offered for a marked item");
    assert!(on);
    bv.click((x, y));
    bv.frames(2);
    assert_eq!(bv.corner("Rat Pelt"), None, "the corner letter is gone at once");
    assert!(!bv.saved_bank_marks(), "and the file no longer holds the mark");
    // the group menu: mark two from their own menus, then select three (one unmarked) and unmark the marked ones together
    for n in [7usize, 8] {
        let p = bv.slot(n);
        bv.click(p);
        bv.frames(2);
        let (x, y, _, _, _) = bv.menu_row("Bank this").unwrap();
        bv.click((x, y));
        bv.frames(2);
    }
    assert_eq!((bv.corner("Rat Pelt").as_deref(), bv.corner("Odd Thing").as_deref()), (Some("B"), Some("B")));
    bv.ctrl_click(6);
    bv.ctrl_click(7);
    bv.ctrl_click(8);
    bv.right_click(8);
    assert_eq!(bv.menu_title().as_deref(), Some("3 items selected"));
    let labels = bv.menu_labels();
    assert!(labels.iter().any(|l| l == "Mark to bank") && labels.iter().any(|l| l == "Unmark bank (2)"), "{labels:?}");
    let (x, y, on, _, _) = bv.menu_row("Unmark bank").unwrap();
    assert!(on);
    bv.click((x, y));
    bv.frames(2);
    assert_eq!((bv.corner("Rat Pelt"), bv.corner("Odd Thing")), (None, None), "both letters are gone at once");
    assert!(!bv.saved_bank_marks());
    assert_eq!(bv.result_text().map(|r| r.0).as_deref(), Some("Unmarked 2 items for the bank"));
    assert!(bv.selected().is_empty());
    bv.clean();
    super::super::action_broker::reset_all_for_test();
}
