// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Mod option toggles for Enhanced HUD options and `eqoa.options.toggle`.
use std::cell::RefCell;

pub const MOD_OPTION_BASE: i32 = 2000;

#[derive(Clone, Debug)]
pub struct ModToggle {
    pub item_id: i32,
    pub addon_id: String,
    pub id: String,
    pub label: String,
    pub default_value: bool,
    pub current_value: bool,
}

thread_local! {
    static TOGGLES: RefCell<Vec<ModToggle>> = const { RefCell::new(Vec::new()) };
}

pub fn register_toggle(addon_id: &str, id: &str, label: &str, default_value: bool) -> (bool, i32) {
    TOGGLES.with(|t| {
        let mut t = t.borrow_mut();
        if let Some(pos) = t.iter().position(|o| o.addon_id == addon_id && o.id == id) {
            let item_id = t[pos].item_id;
            t[pos].label = label.to_string();
            t[pos].default_value = default_value;
            (t[pos].current_value, item_id)
        } else {
            let item_id = MOD_OPTION_BASE + t.len() as i32;
            let current_value = default_value;
            t.push(ModToggle {
                item_id,
                addon_id: addon_id.to_string(),
                id: id.to_string(),
                label: label.to_string(),
                default_value,
                current_value,
            });
            (current_value, item_id)
        }
    })
}

pub fn unregister_addon_toggles(addon_id: &str) {
    TOGGLES.with(|t| {
        t.borrow_mut().retain(|o| o.addon_id != addon_id);
    });
}

pub fn get_toggle(addon_id: &str, id: &str) -> Option<bool> {
    TOGGLES.with(|t| {
        t.borrow().iter().find(|o| o.addon_id == addon_id && o.id == id).map(|o| o.current_value)
    })
}

pub fn set_toggle(addon_id: &str, id: &str, value: bool) -> bool {
    TOGGLES.with(|t| {
        let mut t = t.borrow_mut();
        if let Some(o) = t.iter_mut().find(|o| o.addon_id == addon_id && o.id == id) {
            o.current_value = value;
            true
        } else {
            false
        }
    })
}

pub fn toggle_by_item(item_id: i32) -> bool {
    TOGGLES.with(|t| {
        let mut t = t.borrow_mut();
        if let Some(o) = t.iter_mut().find(|o| o.item_id == item_id) {
            o.current_value = !o.current_value;
            true
        } else {
            false
        }
    })
}

pub fn value_by_item(item_id: i32) -> Option<bool> {
    TOGGLES.with(|t| {
        t.borrow().iter().find(|o| o.item_id == item_id).map(|o| o.current_value)
    })
}

pub fn active_mod_items() -> Vec<(i32, String)> {
    TOGGLES.with(|t| {
        t.borrow().iter().map(|o| (o.item_id, o.label.clone())).collect()
    })
}

pub fn active_mod_item_ids() -> Vec<i32> {
    TOGGLES.with(|t| {
        t.borrow().iter().map(|o| o.item_id).collect()
    })
}
