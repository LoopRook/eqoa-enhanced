// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Mod-owned keybinds on the Controls page and binds file serialization.
use std::sync::Mutex;
use crate::bindpage::Column;
use crate::binds::{self, Binds, ROWS, row_named};
use crate::pointer::{Hits, rows_rect};
use crate::settings::{read_config, write_config};
use vi::ui::{Raster, UI_COLORS, Vui, Window, wide};

const KEYS_FILE: &str = "keys.opt";
const KEY_X: i32 = 280;
const KEY_W: i32 = 110;
const PAD_X: i32 = 138;
const PAD_W: i32 = 90;

#[derive(Clone, Debug)]
pub struct ModBind {
    pub addon_id: String,
    pub id: String,
    pub default_key: String,
    pub current_key: String,
    pub description: String,
}

static MOD_BINDS: Mutex<Vec<ModBind>> = Mutex::new(Vec::new());

pub(crate) fn free_fixture_key(b: &mut Binds) {
    b.bind_key(
        row_named("map_search").expect("Enhanced map search row"),
        "f24",
    );
}

pub fn register(addon: &str, id: &str, default_key: &str, description: &str) -> bool {
    let mut binds = MOD_BINDS.lock().unwrap();
    if let Some(existing) = binds.iter_mut().find(|b| b.addon_id == addon && b.id == id) {
        existing.default_key = default_key.to_ascii_lowercase();
        existing.description = description.to_string();
        return true;
    }

    let saved_key = load_saved_key(addon, id);
    let current_key = saved_key.unwrap_or_else(|| default_key.to_ascii_lowercase());

    binds.push(ModBind {
        addon_id: addon.to_string(),
        id: id.to_string(),
        default_key: default_key.to_ascii_lowercase(),
        current_key,
        description: description.to_string(),
    });
    true
}

pub fn get_key(addon: &str, id: &str) -> Option<String> {
    let binds = MOD_BINDS.lock().unwrap();
    binds.iter()
        .find(|b| b.addon_id == addon && b.id == id)
        .map(|b| b.current_key.clone())
}

pub fn set_key(addon: &str, id: &str, key: &str) -> bool {
    let mut binds = MOD_BINDS.lock().unwrap();
    if let Some(b) = binds.iter_mut().find(|b| b.addon_id == addon && b.id == id) {
        b.current_key = key.to_ascii_lowercase();
        save_mod_bind(addon, id, &b.current_key);
        true
    } else {
        false
    }
}

pub fn extra_rows() -> i32 {
    let binds = MOD_BINDS.lock().unwrap();
    binds.len() as i32
}

pub fn help_or_defaults(row: i32) -> String {
    let r = row as usize;
    if r >= ROWS.len() {
        let idx = r - ROWS.len();
        let binds = MOD_BINDS.lock().unwrap();
        if let Some(b) = binds.get(idx) {
            return format!("{} (from mod: {}).", b.description, b.addon_id);
        }
    }
    "Puts every key and button back as the game began. The macros (Options, Macros) keep what they press, and their triggers of keys no row has.".into()
}

pub fn draw(
    capture: Option<Column>,
    col_sel: i32,
    vui: &Vui,
    r: &mut dyn Raster,
    w: &mut Window,
    entry: i32,
    on: bool,
    at: i32,
    hits: &mut Hits,
) -> bool {
    let base = ROWS.len() as i32;
    if entry < base {
        return false;
    }
    let idx = (entry - base) as usize;
    let binds = MOD_BINDS.lock().unwrap();
    let Some(bind) = binds.get(idx) else {
        return false;
    };

    let width = w.size[0];
    let dim = UI_COLORS[2];
    let c = UI_COLORS[on as usize];
    let (kx, px) = (width - KEY_X, width - PAD_X);

    w.text_box(vui, r, &wide(&bind.description), 40, at, width - KEY_X - 48, 1, c, 2);
    hits.push(rows_rect(w, vui, r, 40, at, width - KEY_X - 48, 1), entry * 4);

    let waiting = on && capture == Some(Column::Keyboard);
    let kt = if waiting {
        "Press a key".to_string()
    } else if bind.current_key.is_empty() {
        "-".to_string()
    } else {
        binds::display(&bind.current_key)
    };
    let pt = "-".to_string();

    for (col, x, cw, t) in [(0, kx, KEY_W, kt), (1, px, PAD_W, pt)] {
        let sel = on && col_sel == col;
        w.draw_frame(vui, r, x, at, cw, 1, if sel { 0x60 } else { 0x40 });
        let tc = if col == 0 { UI_COLORS[0] } else { dim };
        w.text_box(vui, r, &wide(&t), x, at, cw, 1, tc, 8);
        hits.push(rows_rect(w, vui, r, x + 20, at, cw - 20, 1), entry * 4 + 1 + col);
    }

    true
}

pub fn reset_defaults() {
    let mut binds = MOD_BINDS.lock().unwrap();
    for b in binds.iter_mut() {
        b.current_key = b.default_key.clone();
        save_mod_bind(&b.addon_id, &b.id, &b.current_key);
    }
}

pub fn capture_key(
    capture: &mut Option<Column>,
    note: &mut Option<String>,
    row: usize,
    name: &str,
    cues: &mut Vec<i32>,
) -> bool {
    let base = ROWS.len();
    if row < base {
        return false;
    }
    let idx = row - base;
    let mut binds = MOD_BINDS.lock().unwrap();
    let Some(bind) = binds.get_mut(idx) else {
        return false;
    };

    let key_name = name.to_ascii_lowercase();
    bind.current_key = key_name.clone();
    save_mod_bind(&bind.addon_id, &bind.id, &key_name);

    *capture = None;
    *note = Some(format!("{}: {}.", bind.description, binds::display(&key_name)));
    cues.push(13);
    true
}

fn load_saved_key(addon: &str, id: &str) -> Option<String> {
    let bytes = read_config(KEYS_FILE)?;
    let text = String::from_utf8_lossy(&bytes);
    let prefix = format!("mod.{addon}.{id}=");
    for line in text.lines() {
        if let Some(val) = line.strip_prefix(&prefix) {
            return Some(val.trim().to_ascii_lowercase());
        }
    }
    if addon == "eqoa_stats_box" && id == "toggle" {
        for line in text.lines() {
            if let Some(val) = line.strip_prefix("key.stats_box=") {
                return Some(val.trim().to_ascii_lowercase());
            }
        }
    }
    None
}

fn save_mod_bind(addon: &str, id: &str, key: &str) {
    let existing = read_config(KEYS_FILE).unwrap_or_default();
    let text = String::from_utf8_lossy(&existing);
    let key_line = format!("mod.{addon}.{id}={key}");
    let prefix = format!("mod.{addon}.{id}=");

    let mut out = String::new();
    let mut replaced = false;
    for line in text.lines() {
        if line.starts_with(&prefix) {
            out.push_str(&key_line);
            out.push('\n');
            replaced = true;
        } else if line.starts_with("key.stats_box=") {
            continue;
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    if !replaced {
        out.push_str(&key_line);
        out.push('\n');
    }
    let _ = write_config(KEYS_FILE, out.as_bytes());
}

pub fn dispatch_keypress(key_name: &str) -> Option<(String, String)> {
    let key = key_name.to_ascii_lowercase();
    let binds = MOD_BINDS.lock().unwrap();
    for b in binds.iter() {
        if b.current_key == key {
            return Some((b.addon_id.clone(), b.id.clone()));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_frees_f2_while_production_keeps_map_search_on_it() {
        let row = row_named("map_search").unwrap();
        let mut fixture = Binds::default();
        assert_eq!(fixture.key(row), Some("f2"), "production default");
        free_fixture_key(&mut fixture);
        assert_eq!(fixture.key(row), Some("f24"));
        assert_eq!(
            Binds::default().key(row),
            Some("f2"),
            "only this fixture moved"
        );
    }

    #[test]
    fn mod_bind_register_rebind_and_negative_control() {
        let _ = register("test_addon", "toggle_window", "k", "Test Window");
        assert_eq!(get_key("test_addon", "toggle_window"), Some("k".into()));

        // Initial key 'k' fires
        let fired = dispatch_keypress("k");
        assert_eq!(fired, Some(("test_addon".into(), "toggle_window".into())));

        // Negative control: 'm' does not fire initially
        assert_eq!(dispatch_keypress("m"), None);

        // Rebind to 'm'
        assert!(set_key("test_addon", "toggle_window", "m"));
        assert_eq!(get_key("test_addon", "toggle_window"), Some("m".into()));

        // Negative control: old key 'k' does NOTHING now!
        assert_eq!(dispatch_keypress("k"), None, "old key does nothing after rebind");

        // New key 'm' fires!
        assert_eq!(dispatch_keypress("m"), Some(("test_addon".into(), "toggle_window".into())));
    }
}
