// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Mod option toggles for Enhanced HUD options and `eqoa.options.toggle`.
use serde_json::{Value as Json, json};
use std::cell::RefCell;
use std::collections::HashSet;
use std::path::PathBuf;

pub const MOD_OPTION_BASE: i32 = 2000;

#[derive(Clone, Debug)]
pub struct ModToggle {
    pub item_id: i32,
    pub addon_id: String,
    pub id: String,
    pub label: String,
    pub default_value: bool,
    pub current_value: bool,
    pub enabled: bool,
    pub explicit_in_memory: bool,
}

pub const BUNDLED_TOGGLES: &[(&str, &str, &str, i32, bool)] = &[
    ("eqoa_stats_box", "stats_box", "Stats Box", 2000, false),
    (
        "eqoa_target_of_target",
        "target_of_target",
        "Target of Target",
        2001,
        false,
    ),
    ("eqoa_mana_bars", "mana_bars", "Mana Bars", 2002, true),
    ("eqoa_hud_numbers", "hud_numbers", "HUD Numbers", 2003, true),
    (
        "eqoa_hud_numbers",
        "target_hp_percent",
        "Target HP %",
        2004,
        true,
    ),
    ("eqoa_hud_numbers", "xp_details", "XP Details", 2005, true),
];

fn default_toggles() -> Vec<ModToggle> {
    BUNDLED_TOGGLES
        .iter()
        .map(|&(addon_id, id, label, item_id, default_value)| ModToggle {
            item_id,
            addon_id: addon_id.to_string(),
            id: id.to_string(),
            label: label.to_string(),
            default_value,
            current_value: default_value,
            enabled: true,
            explicit_in_memory: false,
        })
        .collect()
}

thread_local! {
    static TOGGLES: RefCell<Vec<ModToggle>> = RefCell::new(default_toggles());
    static DISABLED_ADDONS: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
    static STORAGE_ROOT: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

pub fn set_storage_root(root: Option<PathBuf>) {
    STORAGE_ROOT.with(|r| *r.borrow_mut() = root);
}

pub fn reset_in_memory() {
    TOGGLES.with(|t| *t.borrow_mut() = default_toggles());
    DISABLED_ADDONS.with(|d| d.borrow_mut().clear());
}

fn resolve_addon_id<'a>(addon_id: &'a str, id: &str) -> &'a str {
    if addon_id.is_empty() || addon_id == "mod" || addon_id == "test" {
        if let Some((bundled_addon, _, _, _, _)) =
            BUNDLED_TOGGLES.iter().find(|(_, tid, _, _, _)| *tid == id)
        {
            return bundled_addon;
        }
        if addon_id.is_empty() || addon_id == "mod" {
            return "test";
        }
    }
    addon_id
}

fn bundled_info(addon_id: &str, id: &str) -> Option<(i32, &'static str, bool)> {
    BUNDLED_TOGGLES
        .iter()
        .find(|(a, i, _, _, _)| *a == addon_id && *i == id)
        .map(|(_, _, label, item, def)| (*item, *label, *def))
}

pub(crate) fn load_persisted_json(addon_id: &str) -> Option<Json> {
    let v2_key = super::storage::Storage::key_for_addon_id(addon_id);
    let rel_path = format!("addons/{v2_key}.json");
    if let Some(bytes) = crate::settings::read_config(&rel_path) {
        if let Ok(json) = serde_json::from_slice::<Json>(&bytes) {
            return Some(json);
        }
    }
    let root = STORAGE_ROOT.with(|r| r.borrow().clone()).or_else(|| {
        #[cfg(not(test))]
        {
            std::env::var_os("EQOA_CONFIG")
                .map(PathBuf::from)
                .or_else(crate::settings::config_dir)
        }
        #[cfg(test)]
        {
            None
        }
    });
    if let Some(root) = root {
        let path = root.join("addons").join(format!("{v2_key}.json"));
        if let Ok(bytes) = std::fs::read(path) {
            if let Ok(json) = serde_json::from_slice::<Json>(&bytes) {
                return Some(json);
            }
        }
    }
    None
}

pub fn load_persisted_toggle(addon_id: &str, id: &str) -> Option<bool> {
    let json = load_persisted_json(addon_id)?;
    let entries = json.get("table")?.as_array()?;
    for entry in entries {
        let pair = entry.as_array()?;
        if pair.len() == 2 && pair[0].as_str() == Some(id) {
            return pair[1].as_bool();
        }
    }
    None
}

fn persist_toggle(addon_id: &str, id: &str, value: bool) -> Result<(), String> {
    let v2_key = super::storage::Storage::key_for_addon_id(addon_id);
    let rel_path = format!("addons/{v2_key}.json");
    let mut entries: Vec<Json> = load_persisted_json(addon_id)
        .and_then(|j| j.get("table").and_then(Json::as_array).cloned())
        .unwrap_or_default();

    let mut found = false;
    for entry in &mut entries {
        if let Some(pair) = entry.as_array_mut() {
            if pair.len() == 2 && pair[0].as_str() == Some(id) {
                pair[1] = Json::Bool(value);
                found = true;
                break;
            }
        }
    }
    if !found {
        entries.push(json!([id, value]));
    }

    let new_json = json!({ "table": entries });
    let bytes = serde_json::to_vec(&new_json).map_err(|e| e.to_string())?;

    if let Some(root) = STORAGE_ROOT.with(|r| r.borrow().clone()) {
        super::storage::Storage::new(Some(root)).save_encoded(&v2_key, &new_json)?;
    }
    #[cfg(test)]
    {
        crate::settings::write_config(&rel_path, &bytes)?;
    }
    #[cfg(not(test))]
    {
        let _ = (&rel_path, &bytes);
        if STORAGE_ROOT.with(|r| r.borrow().is_none()) {
            if let Some(dir) = std::env::var_os("EQOA_CONFIG")
                .map(PathBuf::from)
                .or_else(crate::settings::config_dir)
            {
                super::storage::Storage::new(Some(dir)).save_encoded(&v2_key, &new_json)?;
            }
        }
    }
    Ok(())
}

pub fn set_addon_enabled(addon_id: &str, enabled: bool) {
    DISABLED_ADDONS.with(|d| {
        let mut d = d.borrow_mut();
        if enabled {
            d.remove(addon_id);
        } else {
            d.insert(addon_id.to_string());
        }
    });
    TOGGLES.with(|t| {
        for o in t.borrow_mut().iter_mut() {
            if o.addon_id == addon_id {
                o.enabled = enabled;
            }
        }
    });
}

pub fn register_toggle(addon_id: &str, id: &str, label: &str, default_value: bool) -> (bool, i32) {
    let addon_id = resolve_addon_id(addon_id, id);
    let enabled = DISABLED_ADDONS.with(|d| !d.borrow().contains(addon_id));
    TOGGLES.with(|t| {
        let mut t = t.borrow_mut();
        if let Some(pos) = t.iter().position(|o| o.addon_id == addon_id && o.id == id) {
            let item_id = t[pos].item_id;
            t[pos].label = label.to_string();
            t[pos].default_value = default_value;
            t[pos].enabled = enabled;
            if !t[pos].explicit_in_memory {
                if let Some(p) = load_persisted_toggle(addon_id, id) {
                    t[pos].current_value = p;
                } else {
                    t[pos].current_value = default_value;
                }
                t[pos].explicit_in_memory = true;
            }
            (t[pos].current_value, item_id)
        } else {
            let persisted = load_persisted_toggle(addon_id, id);
            let next_id = t
                .iter()
                .map(|o| o.item_id)
                .max()
                .unwrap_or(MOD_OPTION_BASE + 5)
                + 1;
            let item_id = bundled_info(addon_id, id)
                .map(|(item, _, _)| item)
                .unwrap_or(next_id);
            let current_value = persisted.unwrap_or(default_value);
            t.push(ModToggle {
                item_id,
                addon_id: addon_id.to_string(),
                id: id.to_string(),
                label: label.to_string(),
                default_value,
                current_value,
                enabled,
                explicit_in_memory: true,
            });
            (current_value, item_id)
        }
    })
}

pub fn unregister_addon_toggles(addon_id: &str) {
    set_addon_enabled(addon_id, false);
}

pub fn get_toggle(addon_id: &str, id: &str) -> Option<bool> {
    let addon_id = resolve_addon_id(addon_id, id);
    TOGGLES.with(|t| {
        let mut t = t.borrow_mut();
        if let Some(o) = t.iter_mut().find(|o| o.addon_id == addon_id && o.id == id) {
            if !o.explicit_in_memory {
                if let Some(p) = load_persisted_toggle(addon_id, id) {
                    o.current_value = p;
                }
                o.explicit_in_memory = true;
            }
            return Some(o.current_value);
        }
        let persisted = load_persisted_toggle(addon_id, id);
        if let Some((item_id, label, def)) = bundled_info(addon_id, id) {
            let val = persisted.unwrap_or(def);
            let enabled = DISABLED_ADDONS.with(|d| !d.borrow().contains(addon_id));
            t.push(ModToggle {
                item_id,
                addon_id: addon_id.to_string(),
                id: id.to_string(),
                label: label.to_string(),
                default_value: def,
                current_value: val,
                enabled,
                explicit_in_memory: true,
            });
            Some(val)
        } else {
            persisted
        }
    })
}

pub fn set_toggle_persisted(addon_id: &str, id: &str, value: bool) -> Result<(), String> {
    let addon_id = resolve_addon_id(addon_id, id);
    persist_toggle(addon_id, id, value)?;
    let enabled = DISABLED_ADDONS.with(|d| !d.borrow().contains(addon_id));
    TOGGLES.with(|t| {
        let mut t = t.borrow_mut();
        if let Some(o) = t.iter_mut().find(|o| o.addon_id == addon_id && o.id == id) {
            o.current_value = value;
            o.explicit_in_memory = true;
        } else {
            let next_id = t
                .iter()
                .map(|o| o.item_id)
                .max()
                .unwrap_or(MOD_OPTION_BASE + 5)
                + 1;
            let (item_id, label, def) = bundled_info(addon_id, id)
                .map(|(i, l, d)| (i, l.to_string(), d))
                .unwrap_or_else(|| (next_id, id.to_string(), false));
            t.push(ModToggle {
                item_id,
                addon_id: addon_id.to_string(),
                id: id.to_string(),
                label,
                default_value: def,
                current_value: value,
                enabled,
                explicit_in_memory: true,
            });
        }
    });
    Ok(())
}

pub fn set_toggle(addon_id: &str, id: &str, value: bool) -> bool {
    let addon_id = resolve_addon_id(addon_id, id);
    let _ = persist_toggle(addon_id, id, value);
    let enabled = DISABLED_ADDONS.with(|d| !d.borrow().contains(addon_id));
    TOGGLES.with(|t| {
        let mut t = t.borrow_mut();
        if let Some(o) = t.iter_mut().find(|o| o.addon_id == addon_id && o.id == id) {
            o.current_value = value;
            o.explicit_in_memory = true;
            true
        } else {
            let next_id = t
                .iter()
                .map(|o| o.item_id)
                .max()
                .unwrap_or(MOD_OPTION_BASE + 5)
                + 1;
            let (item_id, label, def) = bundled_info(addon_id, id)
                .map(|(i, l, d)| (i, l.to_string(), d))
                .unwrap_or_else(|| (next_id, id.to_string(), false));
            t.push(ModToggle {
                item_id,
                addon_id: addon_id.to_string(),
                id: id.to_string(),
                label,
                default_value: def,
                current_value: value,
                enabled,
                explicit_in_memory: true,
            });
            true
        }
    })
}

pub fn toggle_by_item(item_id: i32) -> bool {
    let changed = TOGGLES.with(|t| {
        let mut t = t.borrow_mut();
        if let Some(o) = t.iter_mut().find(|o| o.item_id == item_id && o.enabled) {
            if !o.explicit_in_memory {
                if let Some(p) = load_persisted_toggle(&o.addon_id, &o.id) {
                    o.current_value = p;
                }
            }
            o.current_value = !o.current_value;
            o.explicit_in_memory = true;
            Some((o.addon_id.clone(), o.id.clone(), o.current_value))
        } else {
            None
        }
    });
    if let Some((addon_id, id, val)) = changed {
        let _ = persist_toggle(&addon_id, &id, val);
        true
    } else {
        false
    }
}

pub fn value_by_item(item_id: i32) -> Option<bool> {
    TOGGLES.with(|t| {
        let mut t = t.borrow_mut();
        let o = t.iter_mut().find(|o| o.item_id == item_id && o.enabled)?;
        if !o.explicit_in_memory {
            if let Some(p) = load_persisted_toggle(&o.addon_id, &o.id) {
                o.current_value = p;
                o.explicit_in_memory = true;
            }
        }
        Some(o.current_value)
    })
}

pub fn active_mod_items() -> Vec<(i32, String)> {
    TOGGLES.with(|t| {
        t.borrow()
            .iter()
            .filter(|o| o.enabled)
            .map(|o| (o.item_id, o.label.clone()))
            .collect()
    })
}

pub fn active_mod_item_ids() -> Vec<i32> {
    TOGGLES.with(|t| {
        t.borrow()
            .iter()
            .filter(|o| o.enabled)
            .map(|o| o.item_id)
            .collect()
    })
}
