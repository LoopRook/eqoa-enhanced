// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Mod-owned keybinds on the Controls page and binds file serialization.
use crate::bindpage::Column;
use crate::binds::{self, Binds, Input, Moved, ROWS, row_named};
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
    /// A key was saved for it (the player's own choice) rather than its default.
    pub saved: bool,
    /// The default key it was refused because something else had it (round 7):
    /// its row starts empty and says why.
    pub refused: String,
}

static MOD_BINDS: Mutex<Vec<ModBind>> = Mutex::new(Vec::new());

pub fn register(addon: &str, id: &str, default_key: &str, description: &str) -> bool {
    // The game's keys first: the lock below must not be held while they load.
    let game = crate::settings::with_binds(Clone::clone);
    let mut binds = MOD_BINDS.lock().unwrap();
    if let Some(existing) = binds.iter_mut().find(|b| b.addon_id == addon && b.id == id) {
        existing.default_key = default_key.to_ascii_lowercase();
        existing.description = description.to_string();
        existing.enabled = true;
        if let Some(saved) = load_saved_key(addon, id) {
            existing.current_key = saved;
            existing.saved = true;
        }
        return true;
    }

    let saved_key = load_saved_key(addon, id);
    let default_key = default_key.to_ascii_lowercase();
    let saved = saved_key.is_some();
    let mut refused = String::new();
    let current_key = saved_key.unwrap_or_else(|| {
        // A default never takes a key in use (round 7): it starts empty.
        if default_in_use(&game, &binds, &default_key) {
            refused = default_key.clone();
            String::new()
        } else {
            default_key.clone()
        }
    });

    binds.push(ModBind {
        addon_id: addon.to_string(),
        id: id.to_string(),
        default_key,
        current_key,
        description: description.to_string(),
        enabled: true,
        saved,
        refused,
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
        b.saved = true;
        b.refused.clear();
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
        let game = crate::settings::with_binds(Clone::clone);
        let binds = MOD_BINDS.lock().unwrap();
        if let Some(i) = enabled_index(&binds, idx) {
            let b = &binds[i];
            let mut help = format!("{} (from mod: {}).", b.description, b.addon_id);
            if let Some(why) = conflict_text(&game, &binds, i) {
                help.push(' ');
                help.push_str(&why);
            } else if !b.refused.is_empty() && b.current_key.is_empty() {
                help.push_str(&format!(
                    " Its default key {} was in use, so it starts with no key{}.",
                    binds::display(&b.refused),
                    owner_text(&game, &binds, i, &b.refused)
                        .map_or(String::new(), |who| format!(": {who}'s")),
                ));
            }
            return help;
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
    ensure_labels(vui);
    if entry < base {
        // A game row whose key a mod also has (an old saved layout) is marked;
        // the client then draws the row as it always did.
        if game_row_shared(entry as usize) {
            mark(vui, r, w, at, on);
        }
        return false;
    }
    let idx = (entry - base) as usize;
    let game = crate::settings::with_binds(Clone::clone);
    let binds = MOD_BINDS.lock().unwrap();
    let Some(i) = enabled_index(&binds, idx) else {
        return false;
    };
    let bind = &binds[i];
    if marked(&game, &binds, i) {
        mark(vui, r, w, at, on);
    }

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
    // The game's defaults come back with these, so a default is judged against
    // them (the macros keep their triggers, `controls_defaults`).
    let game = crate::settings::with_binds(Binds::controls_defaults);
    let mut binds = MOD_BINDS.lock().unwrap();
    for i in 0..binds.len() {
        let default = binds[i].default_key.clone();
        let blocked = binds[i].enabled && default_in_use(&game, &binds[..i], &default);
        let b = &mut binds[i];
        b.refused = if blocked { default.clone() } else { String::new() };
        b.current_key = if blocked { String::new() } else { default };
        b.saved = true;
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
    let key_name = name.to_ascii_lowercase();
    if row < ROWS.len() {
        return game_row_capture(capture, note, row, name, &key_name, cues);
    }
    mod_row_capture(capture, note, row, &key_name, cues)
}

// ---- keys in common between mods and the game (round 7) --------------------------------
//
// One press does one thing. A control the game has wins over a mod's key, a
// key in a macro's trigger wins over a mod's, and between mods the player's
// own saved key wins over a default, then the name decides (`priority`), so
// the winner does not depend on the order the mods loaded in. Giving a row a
// key another has swaps or frees as the Controls page does between two game
// rows. Nothing here rewrites a saved key on its own.

/// The game row a key is on, as its dispatch sees it: moved there, or a
/// default key (the first or an alternate) its row was not moved off.
fn game_row(game: &Binds, key: &str) -> Option<usize> {
    if key.is_empty() {
        return None;
    }
    game.row_on(key).or_else(|| {
        ROWS.iter()
            .position(|r| r.keys.contains(&key))
            .filter(|&r| !game.moved(r))
    })
}

fn enabled_index(binds: &[ModBind], nth: usize) -> Option<usize> {
    binds
        .iter()
        .enumerate()
        .filter(|(_, b)| b.enabled)
        .nth(nth)
        .map(|(i, _)| i)
}

/// Whose key wins when several mods have it: a saved choice before a default,
/// then by addon and id.
fn priority(b: &ModBind) -> (bool, &str, &str) {
    (!b.saved, &b.addon_id, &b.id)
}

/// The enabled mod bindings with `key`, the winner first.
fn holders(binds: &[ModBind], key: &str, skip: Option<usize>) -> Vec<usize> {
    let mut found: Vec<usize> = (0..binds.len())
        .filter(|&i| {
            Some(i) != skip && binds[i].enabled && !key.is_empty() && binds[i].current_key == key
        })
        .collect();
    found.sort_by(|&a, &b| priority(&binds[a]).cmp(&priority(&binds[b])));
    found
}

/// Whether a default key is taken: by a game row, a macro's trigger, or a mod
/// that has it already.
fn default_in_use(game: &Binds, binds: &[ModBind], key: &str) -> bool {
    !key.is_empty()
        && (game_row(game, key).is_some()
            || game.key_in_macros(key)
            || !holders(binds, key, None).is_empty())
}

/// The binding that does something on `key`, if any: none when the game or a
/// macro has the key.
fn firing(game: &Binds, binds: &[ModBind], key: &str) -> Option<usize> {
    if key.is_empty() || game_row(game, key).is_some() || game.key_in_macros(key) {
        return None;
    }
    holders(binds, key, None).first().copied()
}

/// Whether the mod binding `addon`/`id` acts on a press of `key` now. The
/// bind callback asks this at press time, so a key two controls share still
/// does one thing.
pub(crate) fn fires(addon: &str, id: &str, key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    let game = crate::settings::with_binds(Clone::clone);
    let binds = MOD_BINDS.lock().unwrap();
    firing(&game, &binds, &key).is_some_and(|i| binds[i].addon_id == addon && binds[i].id == id)
}

enum Other {
    Game(usize),
    Macros(Vec<usize>),
    Mod(usize),
}

/// What else has binding `i`'s key: the game first, then a macro, then a mod.
fn other_on(game: &Binds, binds: &[ModBind], i: usize) -> Option<Other> {
    let key = &binds[i].current_key;
    if key.is_empty() {
        return None;
    }
    if let Some(r) = game_row(game, key) {
        return Some(Other::Game(r));
    }
    let ms = game.macros_with(&Input::Key(key.clone()));
    if !ms.is_empty() {
        return Some(Other::Macros(ms));
    }
    holders(binds, key, Some(i)).first().map(|&j| Other::Mod(j))
}

fn marked(game: &Binds, binds: &[ModBind], i: usize) -> bool {
    binds[i].enabled && other_on(game, binds, i).is_some()
}

/// Whether some enabled mod has a key game row `row` is on.
fn game_row_shared(row: usize) -> bool {
    let binds = MOD_BINDS.lock().unwrap();
    if binds.iter().all(|b| !b.enabled || b.current_key.is_empty()) {
        return false;
    }
    crate::settings::with_binds(|game| {
        binds.iter().any(|b| {
            b.enabled && !b.current_key.is_empty() && game_row(game, &b.current_key) == Some(row)
        })
    })
}

/// Who has `key`, as a note names it: a game row's label, a macro, a mod's words.
fn owner_text(game: &Binds, binds: &[ModBind], skip: usize, key: &str) -> Option<String> {
    if let Some(r) = game_row(game, key) {
        return Some(row_label(r));
    }
    let ms = game.macros_with(&Input::Key(key.to_string()));
    if !ms.is_empty() {
        return Some(crate::bindpage::macro_names(&ms));
    }
    holders(binds, key, Some(skip))
        .first()
        .map(|&j| binds[j].description.clone())
}

/// The Controls page's words on a mod row whose key is shared, and who wins.
fn conflict_text(game: &Binds, binds: &[ModBind], i: usize) -> Option<String> {
    let key = binds::display(&binds[i].current_key);
    Some(match other_on(game, binds, i)? {
        Other::Game(r) => format!(
            "{key} is also {}'s, and the game's control wins: this does nothing until it has another key.",
            row_label(r)
        ),
        Other::Macros(ms) => format!(
            "{key} is in a trigger of {}, which wins: this does nothing until it has another key.",
            crate::bindpage::macro_names(&ms)
        ),
        Other::Mod(j) => {
            if firing(game, binds, &binds[i].current_key) == Some(i) {
                format!(
                    "{key} is also {}'s, which does nothing now: give one of them another key.",
                    binds[j].description
                )
            } else {
                format!(
                    "{key} is also {}'s, which wins: this does nothing until it has another key.",
                    binds[j].description
                )
            }
        }
    })
}

/// The game rows' labels as the page says them. The page's words come from the
/// draw (`ensure_labels`), as the capture hook is not handed them.
static LABELS: Mutex<(i32, Vec<String>)> = Mutex::new((i32::MIN, Vec::new()));

fn ensure_labels(vui: &Vui) {
    let mut cache = LABELS.lock().unwrap();
    if cache.0 != vui.language || cache.1.len() != ROWS.len() {
        cache.0 = vui.language;
        cache.1 = (0..ROWS.len())
            .map(|r| crate::bindpage::label(vui, r))
            .collect();
    }
}

fn row_label(row: usize) -> String {
    let cache = LABELS.lock().unwrap();
    if let Some(label) = cache.1.get(row) {
        return label.clone();
    }
    // before any draw: the row's label without its word numbers
    let mut out = String::new();
    let mut skip = false;
    for c in ROWS[row].label.chars() {
        match c {
            '{' => skip = true,
            '}' => skip = false,
            _ if !skip => out.push(c),
            _ => {}
        }
    }
    out.trim().to_string()
}

fn mark(vui: &Vui, r: &mut dyn Raster, w: &mut Window, at: i32, on: bool) {
    w.text_box(vui, r, &wide("!"), 20, at, 14, 1, UI_COLORS[on as usize], 8);
}

/// Keys applied one by one; any failure puts back the ones already kept.
fn apply_mod_keys(binds: &mut [ModBind], changes: &[(usize, String)]) -> Result<(), String> {
    let mut done: Vec<(usize, String, bool, String)> = Vec::new();
    for (i, key) in changes {
        let was = (
            binds[*i].current_key.clone(),
            binds[*i].saved,
            binds[*i].refused.clone(),
        );
        if let Err(error) = save_mod_bind(&binds[*i].addon_id, &binds[*i].id, key) {
            for (j, old, saved, refused) in done.into_iter().rev() {
                let _ = save_mod_bind(&binds[j].addon_id, &binds[j].id, &old);
                binds[j].current_key = old;
                binds[j].saved = saved;
                binds[j].refused = refused;
            }
            return Err(error);
        }
        done.push((*i, was.0, was.1, was.2));
        binds[*i].current_key = key.clone();
        binds[*i].saved = true;
        binds[*i].refused.clear();
    }
    Ok(())
}

/// A game row left with no key at all: the keys written out and read back
/// as a saved layout says it. A row with default keys says it with an empty
/// line; a row with none (a bar's slot) by having no line.
fn without_key(game: &Binds, row: usize) -> Binds {
    let line = format!("key.{}=", ROWS[row].id);
    let mut text: String = game
        .to_text()
        .lines()
        .filter(|l| !l.starts_with(&line))
        .map(|l| format!("{l}
"))
        .collect();
    if !ROWS[row].keys.is_empty() {
        text.push_str(&format!("{line}
"));
    }
    Binds::from_text(&text)
}

/// Whether the key a control leaves can go to the one that gave up its own:
/// free of the game, the macros and every mod but the ones giving theirs up.
fn can_take(game: &Binds, binds: &[ModBind], key: &str, leaving: &[usize]) -> bool {
    !key.is_empty()
        && !binds::reserved(key)
        && game_row(game, key).is_none()
        && !game.key_in_macros(key)
        && holders(binds, key, None).iter().all(|i| leaving.contains(i))
}

fn kept_text(game: &Binds, row: usize) -> String {
    match game.key(row) {
        Some(k) if !k.is_empty() => format!("{}: {}.", row_label(row), binds::display(k)),
        _ => format!("{} has no key now.", row_label(row)),
    }
}

fn mod_text(b: &ModBind) -> String {
    if b.current_key.is_empty() {
        format!("{} has no key now.", b.description)
    } else {
        format!("{}: {}.", b.description, binds::display(&b.current_key))
    }
}

/// A game row given a key (the client's capture has checked the key may be
/// bound). With no mod on the key the client does it all; with one, the mod
/// takes the key the row leaves, as a second game row would, or has none.
fn game_row_capture(
    capture: &mut Option<Column>,
    note: &mut Option<String>,
    row: usize,
    name: &str,
    key: &str,
    cues: &mut Vec<i32>,
) -> bool {
    let mut game = crate::settings::binds();
    let mut binds = MOD_BINDS.lock().unwrap();
    let held = holders(&binds, key, None);
    if held.is_empty() {
        return false;
    }
    let had = game.macros_with(&Input::Key(name.to_string()));
    let old = game.key(row).map(str::to_string).filter(|k| !k.is_empty());
    let moved = game.bind_key(row, name);
    let mut changes: Vec<(usize, String)> = held.iter().map(|&i| (i, String::new())).collect();
    // the first mod takes the key the row left, if it is free and no game row took it
    if let (Moved::Freed(_) | Moved::Already, Some(old)) = (&moved, &old)
        && old != key
        && can_take(&game, &binds, old, &held)
    {
        changes[0].1 = old.clone();
    }
    if let Err(error) = apply_mod_keys(&mut binds, &changes) {
        *note = Some(format!("Key not kept: {error}"));
        cues.push(9);
        return true;
    }
    crate::settings::set_binds(game.clone());
    *capture = None;
    let me = format!("{}: {}.", row_label(row), binds::display(name));
    let mut text = match &moved {
        Moved::Swapped(o) => format!("{me} {}", kept_text(&game, *o)),
        _ => me,
    };
    for (i, _) in &changes {
        text.push(' ');
        text.push_str(&mod_text(&binds[*i]));
    }
    if let Moved::Freed(Some(left)) = &moved
        && !left.is_empty()
        && !changes.iter().any(|(_, k)| k == left)
    {
        text.push_str(&format!(" {} does nothing now.", binds::display(left)));
    }
    if !had.is_empty() {
        text.push_str(&format!(
            " {} is no longer in a trigger of {}.",
            binds::display(name),
            crate::bindpage::macro_names(&had)
        ));
    }
    *note = Some(text);
    cues.push(13);
    true
}

fn mod_row_capture(
    capture: &mut Option<Column>,
    note: &mut Option<String>,
    row: usize,
    key: &str,
    cues: &mut Vec<i32>,
) -> bool {
    let mut game = crate::settings::binds();
    let mut binds = MOD_BINDS.lock().unwrap();
    let Some(i) = enabled_index(&binds, row - ROWS.len()) else {
        return false;
    };
    let in_macros = game.macros_with(&Input::Key(key.to_string()));
    if !in_macros.is_empty() {
        *note = Some(format!(
            "{} is in a trigger of {}: choose another key, or change that trigger on the Macros page.",
            binds::display(key),
            crate::bindpage::macro_names(&in_macros)
        ));
        // said at once: the page's help shows the prompt while it waits
        *capture = None;
        cues.push(9);
        return true;
    }
    let game_on = game_row(&game, key);
    let others = holders(&binds, key, Some(i));
    let old = binds[i].current_key.clone();
    let mut changes = vec![(i, key.to_string())];
    let mut moved_game = false;
    if game_on.is_some() || !others.is_empty() {
        let mut leaving = others.clone();
        leaving.push(i);
        let give_back = (old != key && can_take(&game, &binds, &old, &leaving)).then(|| old.clone());
        match game_on {
            Some(g) => {
                game = match &give_back {
                    Some(back) => {
                        let mut moved = game.clone();
                        moved.bind_key(g, back);
                        moved
                    }
                    None => without_key(&game, g),
                };
                moved_game = true;
                changes.extend(others.iter().map(|&j| (j, String::new())));
            }
            None => {
                for (n, &j) in others.iter().enumerate() {
                    let key = if n == 0 { give_back.clone().unwrap_or_default() } else { String::new() };
                    changes.push((j, key));
                }
            }
        }
    }
    if let Err(error) = apply_mod_keys(&mut binds, &changes) {
        *note = Some(format!("Key not kept: {error}"));
        return true;
    }
    if moved_game {
        crate::settings::set_binds(game.clone());
    }
    *capture = None;
    let mut text = format!("{}: {}.", binds[i].description, binds::display(key));
    if let Some(g) = game_on {
        text.push(' ');
        text.push_str(&kept_text(&game, g));
    }
    for (j, _) in changes.iter().skip(1) {
        text.push(' ');
        text.push_str(&mod_text(&binds[*j]));
    }
    *note = Some(text);
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

fn native_mod_key(line: &str) -> bool {
    ["key.stats_box=", "key.map_search=", "key.map_stop="]
        .iter()
        .any(|prefix| line.starts_with(prefix))
}
fn migrated_line(line: &str) -> Option<String> {
    for (old, new) in [
        ("key.stats_box=", "mod.eqoa_stats_box.toggle="),
        ("key.map_search=", "mod.eqoa_map_search.open="),
        ("key.map_stop=", "mod.eqoa_map_search.clear="),
    ] {
        if let Some(key) = line.strip_prefix(old) {
            return Some(format!("{new}{key}"));
        }
    }
    (line.starts_with("mod.") && line.contains('=')).then(|| line.to_string())
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
        .filter(|line| !native_mod_key(line))
        .chain(lines.iter().filter(|line| native_mod_key(line)))
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
    let game = crate::settings::with_binds(Clone::clone);
    let binds = MOD_BINDS.lock().unwrap();
    firing(&game, &binds, &key).map(|i| (binds[i].addon_id.clone(), binds[i].id.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round4b_review_key_migration_keeps_existing_mod_collision() {
        let mut files = std::collections::HashMap::new();
        migrate_key_text(
            b"key.map_search=f8\n",
            b"mod.third_party.open=f8\n",
            |name, bytes| {
                files.insert(name.to_string(), bytes.to_vec());
                Ok(())
            },
        )
        .unwrap();
        let saved = String::from_utf8(files[KEYS_FILE].clone()).unwrap();
        assert!(
            saved.contains("mod.third_party.open=f8\n"),
            "another mod's explicit binding was overwritten"
        );
        assert!(
            saved.contains("mod.eqoa_map_search.open=f8\n"),
            "the player's rebound search key was lost"
        );
        assert_eq!(saved.lines().count(), 2);
        assert!(files[LEGACY_KEYS_FILE].is_empty());
    }

    #[test]
    fn round4b_migration_keeps_rebound_search_and_stop_keys() {
        let legacy = b"key.map_search=f8\nkey.map_stop=f9\nkey.jump=space\n";
        let mut files = std::collections::HashMap::new();
        migrate_key_text(legacy, b"mod.eqoa_map_search.clear=\n", |name, bytes| {
            files.insert(name.to_string(), bytes.to_vec());
            Ok(())
        })
        .unwrap();
        let saved = String::from_utf8(files[KEYS_FILE].clone()).unwrap();
        assert!(saved.contains("mod.eqoa_map_search.open=f8\n"));
        assert!(saved.contains("mod.eqoa_map_search.clear=\n"));
        assert!(!saved.contains("clear=f9"));
        assert_eq!(files[LEGACY_KEYS_FILE], b"key.jump=space\n");
        let mut writes = 0;
        migrate_key_text(b"key.jump=space\n", saved.as_bytes(), |_, _| {
            writes += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(writes, 0, "unrelated keys never trigger a migration write");
    }

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
    fn round4b_gate_native_search_bind_rows_are_removed() {
        assert!(row_named("map_search").is_none());
        assert!(row_named("map_stop").is_none());
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
