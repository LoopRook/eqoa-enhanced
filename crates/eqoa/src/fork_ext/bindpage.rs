// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Mod-owned keybinds on the Controls page and binds file serialization.
use crate::bindpage::Column;
use crate::binds::{self, Binds, ROWS, row_named};
use crate::pointer::{Hits, rows_rect};
use crate::settings::{read_config, write_config};
use std::sync::Mutex;
use vi::ui::{Raster, UI_COLORS, Vui, Window, wide};

const KEYS_FILE: &str = "mod-keys.opt";
const LEGACY_KEYS_FILE: &str = "keys.opt";
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
    pub enabled: bool,
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
        existing.enabled = true;
        if let Some(saved) = load_saved_key(addon, id) {
            existing.current_key = saved;
        }
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
        enabled: true,
    });
    true
}

pub fn set_addon_enabled(addon_id: &str, enabled: bool) {
    let mut binds = MOD_BINDS.lock().unwrap();
    for b in binds.iter_mut() {
        if b.addon_id == addon_id {
            b.enabled = enabled;
        }
    }
}

pub fn reset_in_memory() {
    let mut binds = MOD_BINDS.lock().unwrap();
    binds.clear();
}

pub fn get_key(addon: &str, id: &str) -> Option<String> {
    let binds = MOD_BINDS.lock().unwrap();
    binds
        .iter()
        .find(|b| b.addon_id == addon && b.id == id)
        .map(|b| b.current_key.clone())
}

pub fn set_key(addon: &str, id: &str, key: &str) -> bool {
    let mut binds = MOD_BINDS.lock().unwrap();
    if let Some(b) = binds.iter_mut().find(|b| b.addon_id == addon && b.id == id) {
        let key = key.to_ascii_lowercase();
        if save_mod_bind(addon, id, &key).is_err() {
            return false;
        }
        b.current_key = key;
        true
    } else {
        false
    }
}

pub fn extra_rows() -> i32 {
    let binds = MOD_BINDS.lock().unwrap();
    binds.iter().filter(|b| b.enabled).count() as i32
}

pub fn help_or_defaults(row: i32) -> String {
    let r = row as usize;
    if r >= ROWS.len() {
        let idx = r - ROWS.len();
        let binds = MOD_BINDS.lock().unwrap();
        if let Some(b) = binds.iter().filter(|b| b.enabled).nth(idx) {
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
    let Some(bind) = binds.iter().filter(|b| b.enabled).nth(idx) else {
        return false;
    };

    let width = w.size[0];
    let dim = UI_COLORS[2];
    let c = UI_COLORS[on as usize];
    let (kx, px) = (width - KEY_X, width - PAD_X);

    w.text_box(
        vui,
        r,
        &wide(&bind.description),
        40,
        at,
        width - KEY_X - 48,
        1,
        c,
        2,
    );
    hits.push(
        rows_rect(w, vui, r, 40, at, width - KEY_X - 48, 1),
        entry * 4,
    );

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
        hits.push(
            rows_rect(w, vui, r, x + 20, at, cw - 20, 1),
            entry * 4 + 1 + col,
        );
    }

    true
}

pub fn reset_defaults() {
    let mut binds = MOD_BINDS.lock().unwrap();
    for b in binds.iter_mut() {
        b.current_key = b.default_key.clone();
        let _ = save_mod_bind(&b.addon_id, &b.id, &b.current_key);
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
    let Some(bind) = binds.iter_mut().filter(|b| b.enabled).nth(idx) else {
        return false;
    };

    let key_name = name.to_ascii_lowercase();
    if let Err(error) = save_mod_bind(&bind.addon_id, &bind.id, &key_name) {
        *note = Some(format!("Key not kept: {error}"));
        return true;
    }
    bind.current_key = key_name.clone();

    *capture = None;
    *note = Some(format!(
        "{}: {}.",
        bind.description,
        binds::display(&key_name)
    ));
    cues.push(13);
    true
}

/// Copy every old mod binding before removing it from the native binds file.
/// Existing values in mod-keys.opt win, including an explicitly unbound key.
pub(crate) fn migrate_legacy_keys() -> Result<(), String> {
    let Some(legacy) = read_config(LEGACY_KEYS_FILE) else {
        return Ok(());
    };
    let saved = read_config(KEYS_FILE).unwrap_or_default();
    migrate_key_text(&legacy, &saved, write_config)
}

fn migrated_line(line: &str) -> Option<String> {
    if let Some(key) = line.strip_prefix("key.stats_box=") {
        Some(format!("mod.eqoa_stats_box.toggle={key}"))
    } else if line.starts_with("mod.") && line.contains('=') {
        Some(line.to_string())
    } else {
        None
    }
}

fn migrate_key_text(
    legacy: &[u8],
    saved: &[u8],
    mut write: impl FnMut(&str, &[u8]) -> Result<(), String>,
) -> Result<(), String> {
    let legacy = String::from_utf8_lossy(legacy);
    let mut out = String::from_utf8_lossy(saved).into_owned();
    let mut cleaned = String::new();
    let mut migrated = false;
    // Explicit mod.* choices beat the older Stats Box row, regardless of order.
    let lines: Vec<_> = legacy.lines().collect();
    for line in lines
        .iter()
        .filter(|line| !line.starts_with("key.stats_box="))
        .chain(
            lines
                .iter()
                .filter(|line| line.starts_with("key.stats_box=")),
        )
    {
        if let Some(line) = migrated_line(line) {
            migrated = true;
            let (name, _) = line.split_once('=').unwrap();
            if !out
                .lines()
                .any(|old| old.split_once('=').is_some_and(|(key, _)| key == name))
            {
                if !out.is_empty() && !out.ends_with('\n') {
                    out.push('\n');
                }
                out.push_str(&line);
                out.push('\n');
            }
        }
    }
    if !migrated {
        return Ok(());
    }
    for line in legacy.lines().filter(|line| migrated_line(line).is_none()) {
        cleaned.push_str(line);
        cleaned.push('\n');
    }
    // Always confirm the destination write, even if a previous cleanup failed.
    write(KEYS_FILE, out.as_bytes())?;
    write(LEGACY_KEYS_FILE, cleaned.as_bytes())
}

fn load_saved_key(addon: &str, id: &str) -> Option<String> {
    if let Err(error) = migrate_legacy_keys() {
        eprintln!("settings: mod key migration: {error}");
    }
    let prefix = format!("mod.{addon}.{id}=");
    for file in [KEYS_FILE, LEGACY_KEYS_FILE] {
        let bytes = read_config(file).unwrap_or_default();
        let text = String::from_utf8_lossy(&bytes);
        if let Some(value) = text.lines().find_map(|line| line.strip_prefix(&prefix)) {
            return Some(value.trim().to_ascii_lowercase());
        }
        if file == LEGACY_KEYS_FILE && addon == "eqoa_stats_box" && id == "toggle" {
            if let Some(value) = text
                .lines()
                .find_map(|line| line.strip_prefix("key.stats_box="))
            {
                return Some(value.trim().to_ascii_lowercase());
            }
        }
    }
    None
}

pub fn save_mod_bind(addon: &str, id: &str, key: &str) -> Result<(), String> {
    migrate_legacy_keys()?;
    let existing = read_config(KEYS_FILE).unwrap_or_default();
    let text = String::from_utf8_lossy(&existing);
    let key_line = format!("mod.{addon}.{id}={key}");
    let prefix = format!("mod.{addon}.{id}=");
    let mut out = String::new();
    for line in text.lines().filter(|line| !line.starts_with(&prefix)) {
        out.push_str(line);
        out.push('\n');
    }
    out.push_str(&key_line);
    out.push('\n');
    write_config(KEYS_FILE, out.as_bytes())
}

pub fn dispatch_keypress(key_name: &str) -> Option<(String, String)> {
    let key = key_name.to_ascii_lowercase();
    let binds = MOD_BINDS.lock().unwrap();
    for b in binds.iter().filter(|b| b.enabled) {
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
    fn legacy_mod_key_migration_writes_destination_before_cleanup_and_retries() {
        use std::collections::HashMap;
        let old = b"key.jump=space\nmod.third_party.open=f9\nkey.stats_box=j\nmod.eqoa_stats_box.toggle=f8\n";
        let saved = b"mod.third_party.open=f12\nmod.another.open=\n";
        for fail_at in [1, 2, usize::MAX] {
            let mut files = HashMap::from([
                (LEGACY_KEYS_FILE, old.to_vec()),
                (KEYS_FILE, saved.to_vec()),
            ]);
            let mut writes = Vec::new();
            let result = migrate_key_text(old, saved, |file, bytes| {
                writes.push(file.to_string());
                if writes.len() == fail_at {
                    return Err("injected write failure".into());
                }
                files.insert(
                    if file == KEYS_FILE {
                        KEYS_FILE
                    } else {
                        LEGACY_KEYS_FILE
                    },
                    bytes.to_vec(),
                );
                Ok(())
            });
            assert_eq!(writes[0], KEYS_FILE, "new store is always written first");
            assert_eq!(result.is_err(), fail_at != usize::MAX);
            if fail_at != usize::MAX {
                assert_eq!(
                    files[LEGACY_KEYS_FILE], old,
                    "failure keeps every old binding"
                );
            }
            if fail_at == 1 {
                assert_eq!(
                    writes.len(),
                    1,
                    "cleanup must not run after destination failure"
                );
                assert_eq!(files[KEYS_FILE], saved);
            }
            // A second start must safely finish either partially completed migration.
            let legacy = files[LEGACY_KEYS_FILE].clone();
            let destination = files[KEYS_FILE].clone();
            migrate_key_text(&legacy, &destination, |file, bytes| {
                files.insert(
                    if file == KEYS_FILE {
                        KEYS_FILE
                    } else {
                        LEGACY_KEYS_FILE
                    },
                    bytes.to_vec(),
                );
                Ok(())
            })
            .unwrap();
            assert_eq!(files[LEGACY_KEYS_FILE], b"key.jump=space\n");
            let destination = String::from_utf8(files[KEYS_FILE].clone()).unwrap();
            assert!(
                destination.contains("mod.eqoa_stats_box.toggle=f8\n"),
                "explicit old mod key wins over legacy Stats row"
            );
            assert!(
                destination.contains("mod.third_party.open=f12\n"),
                "already saved mod choice wins"
            );
            assert!(
                destination.contains("mod.another.open=\n"),
                "explicitly unbound values survive"
            );
            assert!(!destination.contains("key.stats_box="));
        }
    }

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
        assert_eq!(
            dispatch_keypress("k"),
            None,
            "old key does nothing after rebind"
        );

        // New key 'm' fires!
        assert_eq!(
            dispatch_keypress("m"),
            Some(("test_addon".into(), "toggle_window".into()))
        );
    }
    #[test]
    fn retained_keys_round_trip_with_mod_key_and_restart() {
        crate::settings::testing::reset(false);
        reset_in_memory();
        crate::settings::testing::put("keys.opt", b"key.attack=q\nkey.hide_ui=z\n");
        register("eqoa_stats_box", "toggle", "b", "Stats");
        assert!(set_key("eqoa_stats_box", "toggle", "f8"));
        reset_in_memory();
        register("eqoa_stats_box", "toggle", "b", "Stats");
        assert_eq!(get_key("eqoa_stats_box", "toggle"), Some("f8".into()));
        let text = String::from_utf8(crate::settings::testing::kept("keys.opt").unwrap()).unwrap();
        assert!(text.contains("key.attack=q") && text.contains("key.hide_ui=z"));
        assert!(!text.contains("mod."));
        let mod_text =
            String::from_utf8(crate::settings::testing::kept(KEYS_FILE).unwrap()).unwrap();
        assert!(mod_text.contains("mod.eqoa_stats_box.toggle=f8"));
        assert_eq!(dispatch_keypress("b"), None);
        assert_eq!(
            dispatch_keypress("f8"),
            Some(("eqoa_stats_box".into(), "toggle".into()))
        );
        crate::settings::testing::reset(false);
        reset_in_memory();
    }

    #[test]
    fn rebind_write_failure_keeps_old_key_and_reports_failure() {
        crate::settings::testing::reset(true);
        reset_in_memory();
        register("key_refusal", "toggle", "b", "Stats");
        assert!(!set_key("key_refusal", "toggle", "f8"));
        assert_eq!(get_key("key_refusal", "toggle"), Some("b".into()));
        crate::settings::testing::reset(false);
        reset_in_memory();
    }
}
