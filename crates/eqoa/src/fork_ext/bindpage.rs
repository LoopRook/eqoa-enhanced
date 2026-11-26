// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Mod-owned keybinds on the Controls page and binds file serialization.
use crate::bindpage::Column;
use crate::binds::{self, Binds, Input, Moved, ROWS, row_named};
use crate::pointer::{Hits, rows_rect};
use crate::settings::{read_config, write_config};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, Ordering};
#[cfg(test)]
use std::sync::atomic::AtomicU64;
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
    /// The key it has now, worked out by `resolve` from `stored` and the default.
    pub current_key: String,
    pub description: String,
    pub enabled: bool,
    /// What `mod-keys.opt` holds for it (the player's own choice; Some("") is
    /// "no key"), or None when it has only its default.
    pub stored: Option<String>,
    /// The key it was refused (a default or an old saved key that something
    /// else has, or that is the client's): its row is empty and says why. It
    /// is worked out again each time, so it survives a restart.
    pub refused: String,
}

static MOD_BINDS: Mutex<Vec<ModBind>> = Mutex::new(Vec::new());

/// Interface profile under which `resolve` last ran: 0 unset, 1 Controller, 2 PC.
static RESOLVED_PC: AtomicU8 = AtomicU8::new(0);

#[cfg(test)]
static RESOLVE_COUNT: AtomicU64 = AtomicU64::new(0);
#[cfg(test)]
static PC_INDEX_REBUILDS: AtomicU64 = AtomicU64::new(0);

struct PcRowIndex {
    binds: Binds,
    /// First row `r` whose `profile_key(r, true)` is `Some(k)` for each non-empty key `k`.
    by_key: HashMap<String, usize>,
    /// Whether `binds.profile_key(r, true) == r_default(r)` for each row `r`.
    on_default: Vec<bool>,
}

thread_local! {
    static PC_ROW_INDEX: RefCell<Option<PcRowIndex>> = const { RefCell::new(None) };
    /// The game's bindings as `resolve` last judged the mods' keys against them: `sync_interface_mode` resolves again
    /// when they are not the bindings now (a key moved or freed on the Controls page, a macro trigger added, changed or
    /// removed on the Macros page, Defaults), so the mods' keys follow the game's with no upstream hook.
    static RESOLVED_AGAINST: RefCell<Option<Binds>> = const { RefCell::new(None) };
}

fn pc_tag() -> u8 {
    if crate::binds::pc_shortcuts() { 2 } else { 1 }
}

#[cfg(test)]
pub(crate) fn resolve_count_for_test() -> u64 {
    RESOLVE_COUNT.load(Ordering::Relaxed)
}

#[cfg(test)]
pub(crate) fn pc_index_rebuilds_for_test() -> u64 {
    PC_INDEX_REBUILDS.load(Ordering::Relaxed)
}

/// The Controls row that opens and closes the Mods & Addons window (the addon manager),
/// beside the mod rows (owner, 2026-11-20). It is a mod binding like any other (the same
/// rule, notes, saved file, cancel) that no mod owns: it is there with no mod enabled and
/// the client acts on it itself (`manager_wins`, `Runtime::route_key`). Its default key is
/// this one constant: change it here. An old build reads `mod.enhanced.mods_addons=` as
/// the entry of a mod it does not have, and ignores it.
pub(crate) const MANAGER_ADDON: &str = "enhanced";
pub(crate) const MANAGER_ID: &str = "mods_addons";
pub(crate) const MANAGER_LABEL: &str = "Mods & Addons";
pub(crate) const MANAGER_DEFAULT_KEY: &str = "shift+f10";

/// Saved keys that were the default of a bundled binding until upstream's sync of 2026-11-25 gave the key to the
/// game (F10 is Settings, F4 the Stat Window): (mod, binding, key). The save is dropped, once, while the game
/// still has the key, and the binding is on its default again (the manager's is Shift+F10, Map Search's
/// "Clear map guide" has none). A key the player chose himself, or one the game gave up, is kept.
const KEYS_TAKEN_BY_THE_GAME: &[(&str, &str, &str)] = &[
    (MANAGER_ADDON, MANAGER_ID, "f10"),
    ("eqoa_map_search", "clear", "f4"),
];

/// Whether the client is somewhere the Mods & Addons window would not be shown or usable (the
/// loading screen, the full map, a world change; at character select and with a game window up
/// there is no plain play at all). The viewer tells this before each key
/// (`report_viewer::addon_event`); the manager row then does nothing and takes no press. It
/// never changes when an ordinary mod key fires.
static MANAGER_BLOCKED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(crate) fn set_manager_blocked(blocked: bool) {
    MANAGER_BLOCKED.store(blocked, std::sync::atomic::Ordering::Relaxed);
}

pub(crate) fn manager_blocked() -> bool {
    MANAGER_BLOCKED.load(std::sync::atomic::Ordering::Relaxed)
}

/// Whether any enabled binding is on a modifier combination: only then can a press be a
/// mod's alone (`owns_press`), so with none the key path has nothing to ask.
static ANY_COMBO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(crate) fn any_combo_bound() -> bool {
    ANY_COMBO.load(std::sync::atomic::Ordering::Relaxed)
}

/// Puts the row among the mod rows (the runtime does this as it starts).
pub(crate) fn register_manager_row() {
    register(MANAGER_ADDON, MANAGER_ID, MANAGER_DEFAULT_KEY, MANAGER_LABEL);
}

fn is_manager(b: &ModBind) -> bool {
    b.addon_id == MANAGER_ADDON && b.id == MANAGER_ID
}

/// The key the manager row has now, ready to compare with a key event without making a name
/// (so a key press with no mod key set costs no allocation).
struct ManagerKey {
    mods: (bool, bool, bool, bool),
    key: winit::keyboard::Key,
    base: String,
}

static MANAGER_KEY: Mutex<Option<ManagerKey>> = Mutex::new(None);

fn set_manager_key(key: Option<&str>) {
    let spec = key.filter(|k| !k.is_empty()).map(|k| {
        let ((sup, ctrl, alt, shift), base) = parse_key(k);
        ManagerKey { mods: (sup, ctrl, alt, shift), key: binds::key_of(base), base: base.to_string() }
    });
    *MANAGER_KEY.lock().unwrap() = spec;
}

/// Whether a key event is the manager row's key (its modifiers held, its key): a cheap test that
/// makes no name and allocates nothing.
pub(crate) fn manager_key_matches(
    key: &winit::keyboard::Key,
    base: Option<&str>,
    mods: (bool, bool, bool, bool),
) -> bool {
    use winit::keyboard::Key;
    let guard = MANAGER_KEY.lock().unwrap();
    let Some(spec) = guard.as_ref() else {
        return false;
    };
    if spec.mods != mods {
        return false;
    }
    match (&spec.key, key) {
        (Key::Named(a), Key::Named(b)) => a == b,
        (Key::Character(_), _) => match base {
            Some(base) => base == spec.base,
            None => matches!(key, Key::Character(c) if c.eq_ignore_ascii_case(&spec.base)),
        },
        _ => false,
    }
}

/// Whether the manager row is what acts on this press (`combo`): it has the key and no game
/// control, macro, client key or winning mod does (the one rule, `firing_press`).
pub(crate) fn manager_wins(combo: &str) -> bool {
    let combo = normalize_key(&combo.to_ascii_lowercase());
    let game = crate::settings::with_binds(Clone::clone);
    let binds = MOD_BINDS.lock().unwrap();
    firing_press(&game, &binds, &combo).is_some_and(|i| is_manager(&binds[i]))
}

pub fn register(addon: &str, id: &str, default_key: &str, description: &str) -> bool {
    // The game's keys first: the lock below must not be held while they load.
    let game = crate::settings::with_binds(Clone::clone);
    let mut binds = MOD_BINDS.lock().unwrap();
    let mut stored = load_saved_key(addon, id);
    if let Some(&(_, _, old)) = KEYS_TAKEN_BY_THE_GAME.iter().find(|(a, i, _)| (*a, *i) == (addon, id)) {
        // once: the first time this binding is registered after the key went to the game, whatever the interface in use
        // then; a key bound afterwards is the player's, whichever interface is in use at a restart
        if !taken_settled(addon, id) {
            let drop = stored.as_deref() == Some(old) && game_owner(&game, old).is_some();
            if settle_taken(addon, id, drop).is_ok() && drop {
                crate::say(&format!("controls: {addon}.{id} was on {old}, which the game has now: back on its default"));
                stored = None;
            }
        }
    }
    if let Some(existing) = binds.iter_mut().find(|b| b.addon_id == addon && b.id == id) {
        existing.default_key = normalize_key(&default_key.to_ascii_lowercase());
        existing.description = description.to_string();
        existing.enabled = true;
        if stored.is_some() {
            existing.stored = stored;
        }
    } else {
        binds.push(ModBind {
            addon_id: addon.to_string(),
            id: id.to_string(),
            default_key: normalize_key(&default_key.to_ascii_lowercase()),
            current_key: String::new(),
            description: description.to_string(),
            enabled: true,
            stored,
            refused: String::new(),
        });
    }
    resolve(&game, &mut binds);
    true
}

pub fn set_addon_enabled(addon_id: &str, enabled: bool) {
    let tag = pc_tag();
    let mut changed = false;
    {
        let mut binds = MOD_BINDS.lock().unwrap();
        for b in binds.iter_mut() {
            if b.addon_id == addon_id && b.enabled != enabled {
                b.enabled = enabled;
                changed = true;
            }
        }
    }
    if !changed && RESOLVED_PC.load(Ordering::Relaxed) == tag {
        return;
    }
    // The game's keys first: the lock below must not be held while they load.
    let game = crate::settings::with_binds(Clone::clone);
    let mut binds = MOD_BINDS.lock().unwrap();
    // A mod coming back (or an interface switch) is checked again against what has the keys now.
    resolve(&game, &mut binds);
}

/// Re-resolves mod defaults once if the active keyboard interface (PC vs Controller) changed, or the game's own
/// bindings did (the Controls and Macros pages). The steady frame costs one comparison of the bindings with the ones
/// last resolved against (no clone): `perf_sync_interface_mode_*` measures it.
pub(crate) fn sync_interface_mode() {
    let tag = pc_tag();
    if RESOLVED_PC.load(Ordering::Relaxed) == tag && crate::settings::with_binds(|game| RESOLVED_AGAINST.with(|r| r.borrow().as_ref() == Some(game))) {
        return;
    }
    let game = crate::settings::with_binds(Clone::clone);
    let mut binds = MOD_BINDS.lock().unwrap();
    resolve(&game, &mut binds);
}

pub fn reset_in_memory() {
    let mut binds = MOD_BINDS.lock().unwrap();
    binds.clear();
    set_manager_key(None);
    ANY_COMBO.store(false, std::sync::atomic::Ordering::Relaxed);
    RESOLVED_PC.store(0, Ordering::Relaxed);
    PC_ROW_INDEX.with(|idx| *idx.borrow_mut() = None);
    RESOLVED_AGAINST.with(|r| *r.borrow_mut() = None);
    *SESSION.lock().unwrap() = None;
}

pub fn get_key(addon: &str, id: &str) -> Option<String> {
    let binds = MOD_BINDS.lock().unwrap();
    binds
        .iter()
        .find(|b| b.addon_id == addon && b.id == id)
        .map(|b| b.current_key.clone())
}

pub fn set_key(addon: &str, id: &str, key: &str) -> bool {
    let game = crate::settings::with_binds(Clone::clone);
    let mut binds = MOD_BINDS.lock().unwrap();
    let Some(i) = binds.iter().position(|b| b.addon_id == addon && b.id == id) else {
        return false;
    };
    let key = normalize_key(&key.to_ascii_lowercase());
    if save_mod_bind(addon, id, &key).is_err() {
        return false;
    }
    binds[i].stored = Some(key);
    resolve(&game, &mut binds);
    true
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
            let mut help = if is_manager(b) {
                "Opens the Mods & Addons window, and closes it again. It has no mod; it is the client's own.".to_string()
            } else {
                format!("{} (from mod: {}).", b.description, b.addon_id)
            };
            if let Some(why) = conflict_text(&game, &binds, i) {
                help.push(' ');
                help.push_str(&why);
            } else if !b.refused.is_empty() && b.current_key.is_empty() {
                help.push(' ');
                help.push_str(&refused_text(&game, &binds, i));
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

/// What each mod's kept key was when the Options window opened.
static SESSION: Mutex<Option<Vec<(String, String, Option<String>)>>> = Mutex::new(None);

/// Options opens (`options.rs`, beside the game's own snapshot, `saved_binds`): the mod
/// keys as they are, so a cancel can put them back with the game's. Mod keys are still
/// written as they are changed; only a cancel rewrites them.
pub fn session_begin() {
    let binds = MOD_BINDS.lock().unwrap();
    let snapshot = binds
        .iter()
        .map(|b| (b.addon_id.clone(), b.id.clone(), b.stored.clone()))
        .collect();
    *SESSION.lock().unwrap() = Some(snapshot);
}

/// Options cancelled (Triangle on the menu, without saving): every mod key back as it was
/// when the window opened, in memory and in `mod-keys.opt`, with the game's keys that
/// `options.rs` has just put back. A key whose file cannot be written stays as it is in
/// memory too (memory and file agree) and the failure is said.
pub fn session_cancel() {
    let errors = restore_session(|addon, id, key| match key {
        Some(key) => save_mod_bind(addon, id, key),
        None => forget_mod_bind(addon, id),
    });
    for error in errors {
        crate::say(&format!("a mod key was not put back: {error}"));
    }
}

/// `session_cancel` with the writing handed in: the errors, one per key not put back.
fn restore_session(mut write: impl FnMut(&str, &str, Option<&str>) -> Result<(), String>) -> Vec<String> {
    let Some(snapshot) = SESSION.lock().unwrap().take() else {
        return Vec::new();
    };
    let game = crate::settings::with_binds(Clone::clone);
    let mut binds = MOD_BINDS.lock().unwrap();
    let mut errors = Vec::new();
    for (addon, id, stored) in snapshot {
        let Some(b) = binds.iter_mut().find(|b| b.addon_id == addon && b.id == id) else {
            continue;
        };
        if b.stored == stored {
            continue;
        }
        match write(&addon, &id, stored.as_deref()) {
            Ok(()) => b.stored = stored,
            Err(error) => errors.push(format!("{addon}.{id}: {error}")),
        }
    }
    resolve(&game, &mut binds);
    errors
}

pub fn reset_defaults() {
    // The game's defaults come back with these, so a default is judged against
    // them (the macros keep their triggers, `controls_defaults`). Every mod,
    // those that are off too, goes back to having only its default: nothing
    // is kept for it, so a refused default is refused again after a restart
    // and says why.
    //
    // In the PC interface Defaults resets only the PC keyboard overrides, and what is shared (the
    // controller's bindings, the macros) is kept (upstream's page says so): the mods' keys are the
    // keyboard's, shared by both, so they stay as the player has them and are judged again against
    // the PC profile's defaults.
    let pc = crate::binds::pc_shortcuts();
    let game = crate::settings::with_binds(|b| b.controls_defaults_profile(pc));
    let mut binds = MOD_BINDS.lock().unwrap();
    if !pc {
        for b in binds.iter_mut() {
            let _ = forget_mod_bind(&b.addon_id, &b.id);
            b.stored = None;
        }
    }
    resolve(&game, &mut binds);
}

pub fn capture_key(
    capture: &mut Option<Column>,
    note: &mut Option<String>,
    row: usize,
    name: &str,
    cues: &mut Vec<i32>,
) -> bool {
    let key_name = normalize_key(&name.to_ascii_lowercase());
    trace_capture(row, &key_name);
    if row < ROWS.len() {
        return game_row_capture(capture, note, row, name, &key_name, cues);
    }
    mod_row_capture(capture, note, row, &key_name, cues)
}

// ---- keys in common between mods and the game (round 7) --------------------------------
//
// One press does one thing. THE RULE (owner, 2026-11-20), decided here and nowhere else:
//   * A key is a NAME: the modifiers held written before the key (`binds::combo_name`, then
//     Super first, `normalize_key`). Shift+Space is a key of its own, not Space. A mod may
//     hold a modifier combination over a key the game uses plain; while it does, that
//     combination BELONGS TO THE MOD: the mod fires and the press is the mod's alone (the
//     game does not act on the plain key for it, `owns_press`, consumed in
//     `Runtime::route_key`); the plain key keeps working for the game.
//   * A real conflict is the SAME NAME bound in the game (`game_owner`: a game row, or a key in
//     a macro's trigger, by that exact name: a bare key the game uses, or the combination
//     itself bound there). The game's control wins at press time (`firing`), and Controls
//     marks it; giving a mod such a key swaps or frees the game row as the page does between
//     two game rows (a macro's key is refused).
//   * Among mods: a key the player saved beats a default (`resolve`), then the addon id, then
//     the binding id (so it does not depend on the order mods loaded) (`priority`).
//   * A key the client keeps for itself (`never_mod_key`) is no mod's at all.
// Nothing here rewrites a saved key on its own: an old shared key is marked and settled at
// press time. What a press TYPES (a layout's Shift+7) never decides anything: the name of
// the combination does.

/// Keys the client takes for itself, beside or before the game's key handling
/// (`App::key_input`, `clipboard_key`): full screen, the report keys, Escape's
/// clean-ups, Windows' Alt+Enter, the clipboard's Ctrl+C, V and X and a Mac's Cmd (Super)
/// forms of them and Cmd+, . A mod never has them, whatever other modifier is on them.
fn never_mod_key(key: &str) -> bool {
    let ((sup, ctrl, alt, _), base) = parse_key(key);
    binds::reserved(base)
        || base == "escape"
        || (alt && base == "enter")
        || (ctrl && matches!(base, "c" | "v" | "x"))
        || (sup && matches!(base, "c" | "v" | "x" | ","))
}

/// A key name taken apart: its modifiers (Ctrl, Alt, Shift and Super, in any order and
/// whatever the order they were written in) and the key. (`binds::split_combo` does not
/// know Super.)
fn parse_key(name: &str) -> ((bool, bool, bool, bool), &str) {
    let (mut sup, mut ctrl, mut alt, mut shift) = (false, false, false, false);
    let mut rest = name;
    while let Some((m, tail)) = rest.split_once('+') {
        if tail.is_empty() {
            break;
        }
        match m {
            "super" => sup = true,
            "ctrl" => ctrl = true,
            "alt" => alt = true,
            "shift" => shift = true,
            _ => break,
        }
        rest = tail;
    }
    ((sup, ctrl, alt, shift), rest)
}

/// A key name written the way the client names a press: Super, Ctrl, Alt, Shift, then the
/// key (`ctrl+shift+u`). Done when a name is read (a file, a default, a capture), so
/// `shift+ctrl+u` by hand is the key a press of it is named.
pub(crate) fn normalize_key(name: &str) -> String {
    let ((sup, ctrl, alt, shift), base) = parse_key(name);
    let named = binds::combo_name(ctrl, alt, shift, base);
    if sup { format!("super+{named}") } else { named }
}

/// Whether any mod has a row on Controls (so a capture there can use what presses showed).
pub(crate) fn has_rows() -> bool {
    !MOD_BINDS.lock().unwrap().is_empty()
}

/// What the game takes a key as.
enum Owner {
    Row(usize),
    Macros(Vec<usize>),
}

/// The game row a key is on, as its dispatch sees it: moved there, or a
/// default key (the first or an alternate) its row was not moved off.
fn game_row(game: &Binds, key: &str) -> Option<usize> {
    if key.is_empty() {
        return None;
    }
    // the interface in use: the PC keyboard has its own profile (1..0 on the bars, its own overrides)
    if crate::binds::pc_shortcuts() {
        return PC_ROW_INDEX.with(|slot| {
            let mut slot = slot.borrow_mut();
            let stale = slot.as_ref().is_none_or(|idx| idx.binds != *game);
            if stale {
                let mut by_key = HashMap::with_capacity(ROWS.len());
                let mut on_default = Vec::with_capacity(ROWS.len());
                for r in 0..ROWS.len() {
                    let pk = game.profile_key(r, true);
                    if let Some(k) = pk.filter(|k| !k.is_empty()) {
                        by_key.entry(k.to_string()).or_insert(r);
                    }
                    on_default.push(pk == r_default(r));
                }
                #[cfg(test)]
                PC_INDEX_REBUILDS.fetch_add(1, Ordering::Relaxed);
                *slot = Some(PcRowIndex {
                    binds: game.clone(),
                    by_key,
                    on_default,
                });
            }
            let idx = slot.as_ref().unwrap();
            idx.by_key.get(key).copied().or_else(|| {
                ROWS.iter()
                    .position(|r| r.keys.contains(&key))
                    .filter(|&r| !game.moved(r) && idx.on_default[r])
            })
        });
    }
    game.row_on(key).or_else(|| {
        ROWS.iter()
            .position(|r| r.keys.contains(&key))
            .filter(|&r| !game.moved(r))
    })
}

/// A row's first default key (what its profile key is while it is on its default).
fn r_default(row: usize) -> Option<&'static str> {
    ROWS[row].keys.first().copied()
}

fn owner_of(game: &Binds, key: &str) -> Option<Owner> {
    if let Some(r) = game_row(game, key) {
        return Some(Owner::Row(r));
    }
    let ms = game.macros_with(&Input::Key(key.to_string()));
    (!ms.is_empty()).then_some(Owner::Macros(ms))
}

/// What the game has on a key by that exact name: a row (moved there or by default), or a
/// key in a macro's trigger. The game names no Super combination, so those are never its own.
fn game_owner(game: &Binds, key: &str) -> Option<Owner> {
    owner_of(game, key)
}

/// One line in the log for every key a Controls row is given: which kind of row, the name the
/// page was handed, and what the game has on that exact name. A key that was taken when the
/// player expected a refusal (or the reverse) can then be read from the log.
fn trace_capture(row: usize, key: &str) {
    let game = crate::settings::with_binds(Clone::clone);
    let has = match owner_of(&game, key) {
        Some(Owner::Row(r)) => format!("row {}", ROWS[r].id),
        Some(Owner::Macros(ms)) => crate::bindpage::macro_names(&ms),
        None => "nothing".to_string(),
    };
    crate::say(&format!(
        "controls: {} row {row} given {key}: the game has {has} on that name{}",
        if row < ROWS.len() { "game" } else { "mod" },
        if never_mod_key(key) { "; the client's own key" } else { "" },
    ));
}

fn enabled_index(binds: &[ModBind], nth: usize) -> Option<usize> {
    binds
        .iter()
        .enumerate()
        .filter(|(_, b)| b.enabled)
        .nth(nth)
        .map(|(i, _)| i)
}

/// Whose key wins when several mods have it (rule 2): by addon, then by id. (A saved
/// key beating a default is settled where keys are worked out, `resolve`: a default
/// is never given a key a saved one has, so the two never share one.)
fn priority(b: &ModBind) -> (&str, &str) {
    (&b.addon_id, &b.id)
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

/// Whether a default key is taken: the client's, the game's or a macro's, or a
/// mod that has it already.
fn default_in_use(game: &Binds, binds: &[ModBind], key: &str) -> bool {
    !key.is_empty()
        && (never_mod_key(key)
            || game_owner(game, key).is_some()
            || !holders(binds, key, None).is_empty())
}

/// Every mod's key worked out again from what is kept and what each default
/// may have: a kept key is its own (a client key is refused, and said); the
/// defaults then take their keys one by one in name order, each only if nothing
/// has the key. Run whenever a mod arrives, goes, comes back, is given a key
/// or has them put back, so the result never depends on the order any of
/// that happened in.
fn resolve(game: &Binds, binds: &mut [ModBind]) {
    #[cfg(test)]
    RESOLVE_COUNT.fetch_add(1, Ordering::Relaxed);
    RESOLVED_PC.store(pc_tag(), Ordering::Relaxed);
    RESOLVED_AGAINST.with(|r| {
        let mut r = r.borrow_mut();
        // a clone only when the bindings differ from the ones kept (the first resolve, or after an edit)
        if r.as_ref() != Some(game) {
            *r = Some(game.clone());
        }
    });
    for b in binds.iter_mut() {
        b.refused.clear();
        match &b.stored {
            Some(k) if never_mod_key(k) => {
                b.refused = k.clone();
                b.current_key.clear();
            }
            Some(k) => b.current_key = k.clone(),
            None => b.current_key.clear(),
        }
    }
    let mut order: Vec<usize> = (0..binds.len()).filter(|&i| binds[i].stored.is_none()).collect();
    order.sort_by(|&a, &b| {
        (binds[a].addon_id.as_str(), binds[a].id.as_str())
            .cmp(&(binds[b].addon_id.as_str(), binds[b].id.as_str()))
    });
    for i in order {
        let default = binds[i].default_key.clone();
        if default.is_empty() {
            continue;
        }
        if default_in_use(game, binds, &default) {
            binds[i].refused = default;
        } else {
            binds[i].current_key = default;
        }
    }
    set_manager_key(binds.iter().find(|b| is_manager(b) && b.enabled).map(|b| b.current_key.as_str()));
    ANY_COMBO.store(
        binds.iter().any(|b| {
            b.enabled && {
                let ((sup, ctrl, alt, shift), _) = parse_key(&b.current_key);
                sup || ctrl || alt || shift
            }
        }),
        std::sync::atomic::Ordering::Relaxed,
    );
}

/// The binding that does something on a key, if any (rules 1 to 3), from what is known
/// without a press (`game_owner`). Marks and tests; a real press uses `firing_press`.
fn firing(game: &Binds, binds: &[ModBind], key: &str) -> Option<usize> {
    if key.is_empty() || never_mod_key(key) || game_owner(game, key).is_some() {
        return None;
    }
    holders(binds, key, None).first().copied()
}

/// The binding that does something on THIS press, by the press's own name (`combo`: the
/// modifiers held written before the key): none when the game has that exact name (a bare key
/// it uses, or the combination bound there) or it is the client's.
fn firing_press(game: &Binds, binds: &[ModBind], combo: &str) -> Option<usize> {
    firing(game, binds, combo)
}

/// Whether the mod binding `addon`/`id` acts on this press now (see `firing_press`). The bind
/// callback asks this at press time, so a key two controls share still does one thing.
pub(crate) fn fires(addon: &str, id: &str, combo: &str) -> bool {
    let combo = normalize_key(&combo.to_ascii_lowercase());
    let game = crate::settings::with_binds(Clone::clone);
    let binds = MOD_BINDS.lock().unwrap();
    let winner = firing_press(&game, &binds, &combo);
    if winner.is_none() && !never_mod_key(&combo) && owner_of(&game, &combo).is_some() {
        // said once per key and binding: a mod key the game has by that name
        static SAID: Mutex<Vec<String>> = Mutex::new(Vec::new());
        let line = format!("{addon}.{id} {combo}");
        let mut said = SAID.lock().unwrap();
        if binds.iter().any(|b| b.addon_id == addon && b.id == id && b.current_key == combo) && !said.contains(&line) {
            said.push(line);
            crate::say(&format!("controls: mod {addon}.{id} on {combo} does nothing: the game has that key"));
        }
    }
    winner.is_some_and(|i| binds[i].addon_id == addon && binds[i].id == id)
}

/// Whether this press is a mod's alone: it carries a modifier (so it is a key of its own,
/// not the plain key the game uses) and a mod holds it and would fire. The window's key
/// event is then consumed (`Runtime::route_key`): the game does not act on the plain key for
/// it. A bare key is never taken from the game this way.
pub(crate) fn owns_press(combo: &str) -> bool {
    let ((sup, ctrl, alt, shift), _) = parse_key(combo);
    if !(sup || ctrl || alt || shift) {
        return false;
    }
    let combo = normalize_key(&combo.to_ascii_lowercase());
    let game = crate::settings::with_binds(Clone::clone);
    let binds = MOD_BINDS.lock().unwrap();
    // the manager row's press is its own only where its window is usable
    firing_press(&game, &binds, &combo).is_some_and(|i| !(is_manager(&binds[i]) && manager_blocked()))
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
    match game_owner(game, key) {
        Some(Owner::Row(r)) => return Some(Other::Game(r)),
        Some(Owner::Macros(ms)) => return Some(Other::Macros(ms)),
        None => {}
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
            b.enabled
                && matches!(game_owner(game, &b.current_key), Some(Owner::Row(r)) if r == row)
        })
    })
}

/// Who has `key`, as a note names it: a game row's label, a macro, a mod's words.
fn owner_text(game: &Binds, binds: &[ModBind], skip: usize, key: &str) -> Option<String> {
    match game_owner(game, key) {
        Some(Owner::Row(r)) => return Some(row_label(r)),
        Some(Owner::Macros(ms)) => return Some(crate::bindpage::macro_names(&ms)),
        None => {}
    }
    holders(binds, key, Some(skip))
        .first()
        .map(|&j| binds[j].description.clone())
}

/// Why a mod row is empty: its default or old saved key was refused.
fn refused_text(game: &Binds, binds: &[ModBind], i: usize) -> String {
    let key = &binds[i].refused;
    let shown = binds::display(key);
    if never_mod_key(key) {
        return format!("{shown} is a key the client keeps for itself, so this has no key: choose another.");
    }
    match owner_text(game, binds, i, key) {
        Some(who) => format!("{shown} is {who}'s, so this has no key: choose another."),
        None => format!("{shown} was in use when this loaded, so it has no key: choose another."),
    }
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
    let mut done: Vec<(usize, Option<String>)> = Vec::new();
    for (i, key) in changes {
        let was = binds[*i].stored.clone();
        if let Err(error) = save_mod_bind(&binds[*i].addon_id, &binds[*i].id, key) {
            for (j, old) in done.into_iter().rev() {
                let _ = match &old {
                    Some(k) => save_mod_bind(&binds[j].addon_id, &binds[j].id, k),
                    None => forget_mod_bind(&binds[j].addon_id, &binds[j].id),
                };
                binds[j].stored = old;
            }
            return Err(error);
        }
        done.push((*i, was));
        binds[*i].stored = Some(key.clone());
    }
    Ok(())
}

/// A game row left with no key at all: the keys written out and read back
/// as a saved layout says it. A row with default keys says it with an empty
/// line; a row with none (a bar's slot) by having no line.
fn without_key(game: &Binds, row: usize) -> Binds {
    if crate::binds::pc_shortcuts() {
        // the PC profile's own line: an empty one is no key (the controller's key is not touched)
        let line = format!("pc_key.{}=", ROWS[row].id);
        let mut text: String = game.to_text().lines().filter(|l| !l.starts_with(&line)).map(|l| format!("{l}
")).collect();
        text.push_str(&format!("{line}
"));
        return Binds::from_text(&text);
    }
    let line = format!("key.{}=", ROWS[row].id);
    let mut text: String = game
        .to_text()
        .lines()
        .filter(|l| !l.starts_with(&line))
        .map(|l| format!("{l}\n"))
        .collect();
    if !ROWS[row].keys.is_empty() {
        text.push_str(&format!("{line}\n"));
    }
    Binds::from_text(&text)
}

/// Whether the key a control leaves can go to the one that gave up its
/// own: not the client's, free of the game, the macros and every mod but the
/// ones giving theirs up.
fn can_take(game: &Binds, binds: &[ModBind], key: &str, leaving: &[usize]) -> bool {
    !key.is_empty()
        && !never_mod_key(key)
        && !binds::reserved(key)
        && game_owner(game, key).is_none()
        && holders(binds, key, None).iter().all(|i| leaving.contains(i))
}

fn kept_text(game: &Binds, row: usize) -> String {
    match game.profile_key(row, crate::binds::pc_shortcuts()) {
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
    let had = game.macros_with(&Input::Key(name.to_string()));
    let pc = crate::binds::pc_shortcuts();
    let old = game.profile_key(row, pc).map(str::to_string).filter(|k| !k.is_empty());
    let moved = game.bind_key_profile(row, name, pc);
    // the mods on that exact name (a mod on Shift+Space is not on Space): `game` is only a
    // copy until `set_binds`, so with none the client does it all
    let exact = holders(&binds, key, None);
    if exact.is_empty() {
        return false;
    }
    let held: Vec<usize> = exact.clone();
    let mut changes: Vec<(usize, String)> = held.iter().map(|&i| (i, String::new())).collect();
    // the first mod on the key takes the key the row left, if it is free and no game row took it
    if let (Some(_), Moved::Freed(_) | Moved::Already, Some(old)) =
        (exact.first(), &moved, &old)
        && old != key
        && can_take(&game, &binds, old, &held)
    {
        changes[0].1 = old.clone();
    }
    if let Err(error) = apply_mod_keys(&mut binds, &changes) {
        *note = Some(format!("Key not kept: {error}"));
        *capture = None;
        cues.push(9);
        return true;
    }
    crate::settings::set_binds(game.clone());
    resolve(&game, &mut binds);
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

/// A refusal said at once: the page's help shows its prompt while it waits.
fn refuse(capture: &mut Option<Column>, note: &mut Option<String>, cues: &mut Vec<i32>, text: String) -> bool {
    crate::say(&format!("controls: refused: {text}"));
    *note = Some(text);
    *capture = None;
    cues.push(9);
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
    let shown = binds::display(key);
    if never_mod_key(key) {
        return refuse(
            capture,
            note,
            cues,
            format!("{shown} is a key the client keeps for itself: choose another key."),
        );
    }
    let owner = game_owner(&game, key);
    match &owner {
        Some(Owner::Macros(ms)) => {
            return refuse(
                capture,
                note,
                cues,
                format!(
                    "{shown} is in a trigger of {}: choose another key, or change that trigger on the Macros page.",
                    crate::bindpage::macro_names(ms)
                ),
            );
        }
        _ => {}
    }
    let game_on = match owner {
        Some(Owner::Row(r)) => Some(r),
        _ => None,
    };
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
                        moved.bind_key_profile(g, back, crate::binds::pc_shortcuts());
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
        *capture = None;
        return true;
    }
    if moved_game {
        crate::settings::set_binds(game.clone());
    }
    resolve(&game, &mut binds);
    *capture = None;
    crate::say(&format!("controls: mod {}.{} kept {key}", binds[i].addon_id, binds[i].id));
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
            return Some(normalize_key(&value.trim().to_ascii_lowercase()));
        }
        if file == LEGACY_KEYS_FILE && addon == "eqoa_stats_box" && id == "toggle" {
            if let Some(value) = text
                .lines()
                .find_map(|line| line.strip_prefix("key.stats_box="))
            {
                return Some(normalize_key(&value.trim().to_ascii_lowercase()));
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

/// The marker line in `mod-keys.opt` that says this binding's saved key was looked at once (`KEYS_TAKEN_BY_THE_GAME`).
fn taken_marker(addon: &str, id: &str) -> String {
    format!("migrated.{addon}.{id}=keys_taken_by_the_game_2026_11_25")
}

fn taken_settled(addon: &str, id: &str) -> bool {
    let marker = taken_marker(addon, id);
    let bytes = read_config(KEYS_FILE).unwrap_or_default();
    String::from_utf8_lossy(&bytes).lines().any(|line| line.trim() == marker)
}

/// Writes the marker (and takes the binding's saved key out when `drop`), in one write of the whole file: after it the
/// saved key of this binding is never touched again, so a key the player binds later stays.
fn settle_taken(addon: &str, id: &str, drop: bool) -> Result<(), String> {
    let existing = read_config(KEYS_FILE).unwrap_or_default();
    let text = String::from_utf8_lossy(&existing);
    let prefix = format!("mod.{addon}.{id}=");
    let mut out = String::new();
    for line in text.lines().filter(|line| !(drop && line.starts_with(&prefix))) {
        out.push_str(line);
        out.push('\n');
    }
    out.push_str(&taken_marker(addon, id));
    out.push('\n');
    write_config(KEYS_FILE, out.as_bytes())
}

/// Takes a mod's entry out of `mod-keys.opt`: it has only its default again.
fn forget_mod_bind(addon: &str, id: &str) -> Result<(), String> {
    let existing = read_config(KEYS_FILE).unwrap_or_default();
    let text = String::from_utf8_lossy(&existing);
    let prefix = format!("mod.{addon}.{id}=");
    if !text.lines().any(|line| line.starts_with(&prefix)) {
        return Ok(());
    }
    let mut out = String::new();
    for line in text.lines().filter(|line| !line.starts_with(&prefix)) {
        out.push_str(line);
        out.push('\n');
    }
    write_config(KEYS_FILE, out.as_bytes())
}

pub fn dispatch_keypress(key_name: &str) -> Option<(String, String)> {
    let key = normalize_key(&key_name.to_ascii_lowercase());
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
        crate::settings::testing::reset(false);
        reset_in_memory();
        let _ = register("test_addon", "toggle_window", "9", "Test Window");
        assert_eq!(get_key("test_addon", "toggle_window"), Some("9".into()));

        // Initial key 'u' fires
        let fired = dispatch_keypress("9");
        assert_eq!(fired, Some(("test_addon".into(), "toggle_window".into())));

        // Negative control: 'y' does not fire initially
        assert_eq!(dispatch_keypress("y"), None);

        // Rebind to 'y'
        assert!(set_key("test_addon", "toggle_window", "y"));
        assert_eq!(get_key("test_addon", "toggle_window"), Some("y".into()));

        // Negative control: old key 'u' does NOTHING now!
        assert_eq!(
            dispatch_keypress("9"),
            None,
            "old key does nothing after rebind"
        );

        // New key 'y' fires!
        assert_eq!(
            dispatch_keypress("y"),
            Some(("test_addon".into(), "toggle_window".into()))
        );
    }
    #[test]
    fn retained_keys_round_trip_with_mod_key_and_restart() {
        crate::settings::testing::reset(false);
        reset_in_memory();
        crate::settings::testing::put("keys.opt", b"key.attack=q\nkey.hide_ui=z\n");
        register("eqoa_stats_box", "toggle", "b", "Stats");
        assert!(set_key("eqoa_stats_box", "toggle", "f2"));
        reset_in_memory();
        register("eqoa_stats_box", "toggle", "b", "Stats");
        assert_eq!(get_key("eqoa_stats_box", "toggle"), Some("f2".into()));
        let text = String::from_utf8(crate::settings::testing::kept("keys.opt").unwrap()).unwrap();
        assert!(text.contains("key.attack=q") && text.contains("key.hide_ui=z"));
        assert!(!text.contains("mod."));
        let mod_text =
            String::from_utf8(crate::settings::testing::kept(KEYS_FILE).unwrap()).unwrap();
        assert!(mod_text.contains("mod.eqoa_stats_box.toggle=f2"));
        assert_eq!(dispatch_keypress("b"), None);
        assert_eq!(
            dispatch_keypress("f2"),
            Some(("eqoa_stats_box".into(), "toggle".into()))
        );
        crate::settings::testing::reset(false);
        reset_in_memory();
    }

    #[test]
    fn round7e_fix2_a_cancel_whose_write_fails_leaves_memory_and_file_agreeing() {
        crate::settings::testing::reset(false);
        reset_in_memory();
        register("alpha", "act", "b", "Alpha");
        session_begin();
        assert!(set_key("alpha", "act", "9"));
        let errors = restore_session(|_, _, _| Err("disk full".into()));
        assert_eq!(errors.len(), 1, "the failure is reported: {errors:?}");
        assert_eq!(get_key("alpha", "act"), Some("9".into()), "memory stays with the file");
        let file = String::from_utf8(crate::settings::testing::kept(KEYS_FILE).unwrap()).unwrap();
        assert!(file.contains("mod.alpha.act=9"), "{file}");
        // control: with the write working the key is put back
        session_begin();
        assert!(set_key("alpha", "act", "y"));
        assert!(restore_session(|a, i, k| match k {
            Some(k) => save_mod_bind(a, i, k),
            None => forget_mod_bind(a, i),
        })
        .is_empty());
        assert_eq!(get_key("alpha", "act"), Some("9".into()));
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

    // ---- sync 2026-11-25 (H3): the PC interface has its own keyboard profile ----------------------------------

    fn interface(pc: bool) {
        crate::settings::testing::reset(false);
        crate::settings::set_ui_mode(pc).unwrap();
        assert_eq!(crate::binds::pc_shortcuts(), pc, "the interface under test");
        reset_in_memory();
    }

    fn with_pc_key(id: &str, key: &str) {
        let mut text = crate::settings::binds().to_text();
        text.push_str(&format!("pc_key.{id}={key}
"));
        crate::settings::set_binds(Binds::from_text(&text));
    }

    /// The game's rows as a mod's key is judged against them: in the PC interface 7..0 are Bars 1-2's slots and a
    /// PC override (`pc_key.<row>=`) is a key the game has; in the Controller interface neither is.
    #[test]
    fn sync_h3_a_mod_key_is_judged_against_the_interfaces_own_keys() {
        let slot = row_named("bar2_slot2").unwrap();
        let three = row_named("pcbar3_slot1").unwrap();
        for pc in [true, false] {
            interface(pc);
            with_pc_key("pcbar3_slot1", "shift+1");
            let game = crate::settings::binds();
            assert_eq!(game_row(&game, "7"), pc.then_some(slot), "7 is Bar 2's second slot in the PC interface only");
            assert_eq!(game_row(&game, "shift+1"), pc.then_some(three), "a PC override is the PC interface's key only");
            // a mod's default on them: refused where the game has the key, kept where it does not
            register("alpha", "seven", "7", "Seven");
            register("alpha", "over", "shift+1", "Over");
            assert_eq!(get_key("alpha", "seven").as_deref(), Some(if pc { "" } else { "7" }), "pc={pc}");
            assert_eq!(get_key("alpha", "over").as_deref(), Some(if pc { "" } else { "shift+1" }), "pc={pc}");
            assert_eq!(dispatch_keypress("7").is_some(), !pc, "and at the press");
            assert_eq!(dispatch_keypress("shift+1").is_some(), !pc, "and at the press");
        }
        crate::settings::testing::reset(false);
        reset_in_memory();
    }

    /// Giving a game row the key a mod has changes the interface's own profile: the PC's overrides in the PC
    /// interface, the Controller's keys in the other (the other profile is left as it was).
    #[test]
    fn sync_h3_a_capture_on_a_game_row_changes_only_the_interfaces_own_profile() {
        let art = row_named("art").unwrap();
        for pc in [true, false] {
            interface(pc);
            register("alpha", "act", "", "Act");
            assert!(set_key("alpha", "act", "b"));
            let (mut capture, mut note, mut cues) = (Some(crate::bindpage::Column::Keyboard), None, Vec::new());
            assert!(capture_key(&mut capture, &mut note, art, "b", &mut cues), "a mod has the key: the fork's page does it");
            let text = crate::settings::binds().to_text();
            assert_eq!(text.contains("pc_key.art=b"), pc, "pc={pc}: {text}");
            assert_eq!(text.contains("key.art=b
") && !text.contains("pc_key.art=b"), !pc, "pc={pc}: {text}");
            assert_eq!(get_key("alpha", "act").as_deref(), Some("f9"), "the mod took the key Art left (f9 in both profiles)");
        }
        crate::settings::testing::reset(false);
        reset_in_memory();
    }

    /// A mod given a key a game row has takes it from that row in the interface's own profile only.
    #[test]
    fn sync_h3_a_capture_on_a_mod_row_takes_the_key_from_the_interfaces_own_profile() {
        let three = row_named("pcbar3_slot1").unwrap();
        interface(true);
        with_pc_key("pcbar3_slot1", "shift+1");
        register("alpha", "act", "", "Act");
        let controller_before: Vec<String> = crate::settings::binds().to_text().lines().filter(|l| l.starts_with("key.")).map(str::to_string).collect();
        let (mut capture, mut note, mut cues) = (Some(crate::bindpage::Column::Keyboard), None, Vec::new());
        assert!(capture_key(&mut capture, &mut note, ROWS.len(), "shift+1", &mut cues));
        assert_eq!(get_key("alpha", "act").as_deref(), Some("shift+1"));
        let game = crate::settings::binds();
        assert_eq!(game.profile_key(three, true).unwrap_or(""), "", "the PC row has no key now");
        let after: Vec<String> = game.to_text().lines().filter(|l| l.starts_with("key.")).map(str::to_string).collect();
        assert_eq!(after, controller_before, "the Controller's keys are not touched by a PC capture");
        crate::settings::testing::reset(false);
        reset_in_memory();
    }

    /// Defaults on the Controls page: in the PC interface the mods' keys (shared with the Controller's) stay; in the
    /// Controller interface they go back to their defaults, as before.
    #[test]
    fn sync_h3_defaults_in_the_pc_interface_keep_the_mods_keys() {
        for pc in [true, false] {
            interface(pc);
            register("alpha", "act", "y", "Act");
            assert!(set_key("alpha", "act", "f2"));
            reset_defaults();
            assert_eq!(get_key("alpha", "act").as_deref(), Some(if pc { "f2" } else { "y" }), "pc={pc}");
        }
        crate::settings::testing::reset(false);
        reset_in_memory();
    }

    /// Owner decisions 2026-11-25: F10 and F4 belong to the game now. A saved F10 for the Mods & Addons row and a saved
    /// F4 for Map Search's "Clear map guide" (the old defaults) go back to their defaults while the game has the key; a key
    /// the player chose, or one the game gave up, stays.
    #[test]
    fn sync_old_default_keys_the_game_has_now_go_back_to_their_defaults() {
        crate::settings::testing::reset(false);
        reset_in_memory();
        crate::settings::testing::put(KEYS_FILE, b"mod.enhanced.mods_addons=f10
mod.eqoa_map_search.clear=f4
mod.eqoa_map_search.open=f4
");
        register_manager_row();
        register("eqoa_map_search", "clear", "", "Clear map guide");
        register("eqoa_map_search", "open", "f2", "Open");
        assert_eq!(get_key(MANAGER_ADDON, MANAGER_ID).as_deref(), Some("shift+f10"));
        assert_eq!(get_key("eqoa_map_search", "clear").as_deref(), Some(""), "unbound by default");
        assert_eq!(get_key("eqoa_map_search", "open").as_deref(), Some("f4"), "control: another binding's saved key is the player's, kept");
        let kept = String::from_utf8(crate::settings::testing::kept(KEYS_FILE).unwrap()).unwrap();
        assert!(!kept.contains("mods_addons=f10") && !kept.contains("clear=f4"), "{kept}");
        // control: a key the player chose stays
        reset_in_memory();
        crate::settings::testing::put(KEYS_FILE, b"mod.enhanced.mods_addons=f8
mod.eqoa_map_search.clear=f6
");
        register_manager_row();
        register("eqoa_map_search", "clear", "", "Clear map guide");
        assert_eq!(get_key(MANAGER_ADDON, MANAGER_ID).as_deref(), Some("f8"));
        assert_eq!(get_key("eqoa_map_search", "clear").as_deref(), Some("f6"));
        crate::settings::testing::reset(false);
        reset_in_memory();
    }

    /// M3 of the 2026-11-25 delta review: the drop of an old saved F10 / F4 happens once. After the first run a key the
    /// player binds (F10 for Mods & Addons in the PC interface) is never taken again, not at a restart in the other
    /// interface and not at one in the same. Negative control (run red): with the marker ignored, the Controller
    /// restart (where the game's Settings has F10 too) wiped the player's own F10.
    #[test]
    fn sync_the_old_key_drop_is_once_and_a_later_f10_in_either_interface_stays() {
        interface(true);
        crate::settings::testing::put(KEYS_FILE, b"mod.enhanced.mods_addons=f10
mod.eqoa_map_search.clear=f4
");
        register_manager_row();
        register("eqoa_map_search", "clear", "", "Clear map guide");
        assert_eq!(get_key(MANAGER_ADDON, MANAGER_ID).as_deref(), Some("shift+f10"), "the first run drops the old save");
        let first = String::from_utf8(crate::settings::testing::kept(KEYS_FILE).unwrap()).unwrap();
        assert!(!first.contains("mod.enhanced.mods_addons=") && !first.contains("clear=f4"), "{first}");
        assert!(first.contains("migrated.enhanced.mods_addons=") && first.contains("migrated.eqoa_map_search.clear="), "{first}");
        // the player now binds F10 and F4 himself, in the PC interface
        assert!(set_key(MANAGER_ADDON, MANAGER_ID, "f10"));
        assert!(set_key("eqoa_map_search", "clear", "f4"));
        for pc in [false, true, false] {
            // a restart in the other interface, then back (the files stay: not `interface()`)
            crate::settings::set_ui_mode(pc).unwrap();
            reset_in_memory();
            register_manager_row();
            register("eqoa_map_search", "clear", "", "Clear map guide");
            let kept = String::from_utf8(crate::settings::testing::kept(KEYS_FILE).unwrap()).unwrap();
            assert!(kept.contains("mod.enhanced.mods_addons=f10") && kept.contains("mod.eqoa_map_search.clear=f4"), "pc={pc}: {kept}");
        }
        crate::settings::testing::reset(false);
        reset_in_memory();
    }

    /// The first run in the Controller interface drops the old default too (the game has F10 there as well), and the
    /// marker is written even for a binding that had nothing saved, so a key bound later is the player's.
    #[test]
    fn sync_the_old_key_drop_also_runs_first_in_the_controller_and_marks_a_binding_with_no_save() {
        interface(false);
        crate::settings::testing::put(KEYS_FILE, b"mod.enhanced.mods_addons=f10
");
        register_manager_row();
        register("eqoa_map_search", "clear", "", "Clear map guide");
        assert_eq!(get_key(MANAGER_ADDON, MANAGER_ID).as_deref(), Some("shift+f10"));
        let kept = String::from_utf8(crate::settings::testing::kept(KEYS_FILE).unwrap()).unwrap();
        assert!(kept.contains("migrated.eqoa_map_search.clear="), "a binding with no saved key is marked too: {kept}");
        assert!(set_key("eqoa_map_search", "clear", "f4"));
        reset_in_memory();
        register_manager_row();
        register("eqoa_map_search", "clear", "", "Clear map guide");
        assert_eq!(get_key("eqoa_map_search", "clear").as_deref(), Some("f4"), "a later F4 stays");
        crate::settings::testing::reset(false);
        reset_in_memory();
    }

    /// Regression test (PC interface perf): `game_row` in PC mode uses a cached profile index
    /// that matches the exact `profile_key` scan for every key and rebuilds only when `Binds` changes.
    #[test]
    fn perf_pc_mode_game_row_caches_profile_index_and_matches_naive_scan() {
        let naive = |game: &Binds, key: &str| -> Option<usize> {
            if key.is_empty() {
                return None;
            }
            (0..ROWS.len())
                .find(|&r| game.profile_key(r, true) == Some(key))
                .or_else(|| {
                    ROWS.iter()
                        .position(|r| r.keys.contains(&key))
                        .filter(|&r| !game.moved(r) && game.profile_key(r, true) == r_default(r))
                })
        };
        let keys = [
            "1", "7", "0", "w", "g", "space", "v", "`", "~", "f4", "f10", "shift+1", "b", "y",
            "shift+f10", "ctrl+9", "",
        ];
        interface(true);
        let base_rebuilds = pc_index_rebuilds_for_test();
        let game = crate::settings::binds();
        for &k in &keys {
            assert_eq!(game_row(&game, k), naive(&game, k), "default PC binds for {k:?}");
        }
        assert_eq!(
            pc_index_rebuilds_for_test() - base_rebuilds,
            1,
            "all lookups on unchanged Binds share one index build"
        );
        // Repeated lookups must not rebuild.
        for _ in 0..20 {
            assert_eq!(game_row(&game, "7"), naive(&game, "7"));
            assert_eq!(game_row(&game, "shift+f10"), naive(&game, "shift+f10"));
        }
        assert_eq!(pc_index_rebuilds_for_test() - base_rebuilds, 1);

        // Negative control: mutating Binds invalidates the cache and rebuilds once.
        with_pc_key("pcbar3_slot1", "shift+1");
        with_pc_key("art", "b");
        let modified = crate::settings::binds();
        for &k in &keys {
            assert_eq!(game_row(&modified, k), naive(&modified, k), "modified PC binds for {k:?}");
        }
        assert_eq!(
            pc_index_rebuilds_for_test() - base_rebuilds,
            2,
            "negative control: changed Binds triggers exactly one rebuild"
        );
        crate::settings::testing::reset(false);
        reset_in_memory();
    }

    /// Regression test (PC interface perf): redundant `set_addon_enabled` and steady `sync_interface_mode`
    /// do not re-run `resolve()`, while real enable/disable transitions and PC <-> Controller switches do.
    #[test]
    fn perf_redundant_set_addon_enabled_skips_resolve_but_mode_switch_and_toggle_run_it() {
        interface(true);
        register("alpha", "seven", "7", "Seven");
        assert_eq!(get_key("alpha", "seven").as_deref(), Some(""), "7 is Bar 2 slot 2 in PC mode");

        let before = resolve_count_for_test();
        for _ in 0..20 {
            set_addon_enabled("alpha", true);
            set_addon_enabled("mod_without_binds", true);
            sync_interface_mode();
        }
        assert_eq!(
            resolve_count_for_test(),
            before,
            "steady-state calls must not re-run resolve()"
        );

        // Negative control 1: real disable -> enable transition runs resolve().
        set_addon_enabled("alpha", false);
        assert_eq!(resolve_count_for_test(), before + 1);
        set_addon_enabled("alpha", true);
        assert_eq!(resolve_count_for_test(), before + 2);

        // Negative control 2: switching PC -> Controller runs resolve() once and frees '7'.
        crate::settings::set_ui_mode(false).unwrap();
        sync_interface_mode();
        assert_eq!(resolve_count_for_test(), before + 3);
        assert_eq!(get_key("alpha", "seven").as_deref(), Some("7"), "7 is free in Controller mode");
        for _ in 0..10 {
            sync_interface_mode();
        }
        assert_eq!(resolve_count_for_test(), before + 3, "steady Controller frames do not re-resolve");

        // Switching Controller -> PC runs resolve() once and refuses '7' again.
        crate::settings::set_ui_mode(true).unwrap();
        sync_interface_mode();
        assert_eq!(resolve_count_for_test(), before + 4);
        assert_eq!(get_key("alpha", "seven").as_deref(), Some(""), "7 is taken again in PC mode");

        crate::settings::testing::reset(false);
        reset_in_memory();
    }

    // ---- batch 8 review H1: the mods' keys follow the game's bindings as they change -----------------------------

    /// Binds as the Controls and Macros pages leave them: the player's edit, then `set_binds` (no hook of ours runs).
    fn edit_game(f: impl FnOnce(&mut Binds)) {
        let mut b = crate::settings::binds();
        f(&mut b);
        crate::settings::set_binds(b);
    }

    /// A key the game refused to a mod is the mod's once the player moves the game's row off it, and the mod loses a
    /// key the player binds a game row (or a macro trigger) onto, with no restart and no interface switch. Controls:
    /// the keys stay as they were until the frame sync runs (the old code never re-resolved on these edits), and a
    /// steady frame with no edit resolves nothing.
    #[test]
    fn game_key_and_macro_edits_re_resolve_the_mods_keys_without_a_restart() {
        use crate::binds::{Input, row_named};
        for pc in [false, true] {
            interface(pc);
            let stat = row_named("stat_window").unwrap();
            // a key no game row has (a row on it would swap, and take F4 for itself)
            let free = ["insert", "home", "end", "pageup", "pagedown", "f9", "f12"]
                .into_iter()
                .find(|k| game_row(&crate::settings::binds(), k).is_none())
                .expect("a free key");
            register("alpha", "tog", "f4", "Toggle");
            assert_eq!(get_key("alpha", "tog").as_deref(), Some(""), "pc={pc}: F4 is the Stat Window's: refused");
            let steady = resolve_count_for_test();
            for _ in 0..5 {
                sync_interface_mode();
            }
            assert_eq!(resolve_count_for_test(), steady, "pc={pc}: a steady frame resolves nothing");
            // the player moves the Stat Window row to F8 on the Controls page: F4 is free
            edit_game(|b| {
                b.bind_key_profile(stat, free, pc);
            });
            assert_eq!(get_key("alpha", "tog").as_deref(), Some(""), "pc={pc}: control: not before the frame sync");
            sync_interface_mode();
            assert_eq!(get_key("alpha", "tog").as_deref(), Some("f4"), "pc={pc}: the freed key is the mod's");
            assert_eq!(resolve_count_for_test(), steady + 1);
            sync_interface_mode();
            assert_eq!(resolve_count_for_test(), steady + 1, "pc={pc}: and then steady");
            // the player binds a game row back onto F4: the mod loses it
            edit_game(|b| {
                b.bind_key_profile(stat, "f4", pc);
            });
            sync_interface_mode();
            assert_eq!(get_key("alpha", "tog").as_deref(), Some(""), "pc={pc}: the game's key again");
            // Defaults (the whole of the game's bindings put back) after a move
            edit_game(|b| {
                b.bind_key_profile(stat, free, pc);
            });
            sync_interface_mode();
            assert_eq!(get_key("alpha", "tog").as_deref(), Some("f4"));
            edit_game(|b| *b = Binds::default());
            sync_interface_mode();
            assert_eq!(get_key("alpha", "tog").as_deref(), Some(""), "pc={pc}: Defaults give F4 back to the Stat Window");
            // a macro trigger added on the Macros page takes the mod's key; removed, gives it back (macro triggers
            // are the Controller keys, so this one is a key no row has in either interface)
            register("alpha", "macro", free, "Macro key");
            assert_eq!(get_key("alpha", "macro").as_deref(), Some(free), "pc={pc}: a free key is the mod's");
            edit_game(|b| {
                b.set_trigger(0, None, vec![Input::Key(free.into())]).unwrap();
            });
            sync_interface_mode();
            assert_eq!(get_key("alpha", "macro").as_deref(), Some(""), "pc={pc}: a macro trigger on its key takes it");
            edit_game(|b| {
                b.remove_trigger(0, 0);
            });
            sync_interface_mode();
            assert_eq!(get_key("alpha", "macro").as_deref(), Some(free), "pc={pc}: and gives it back");
            crate::settings::testing::reset(false);
            reset_in_memory();
        }
    }

    /// The steady frame's cost: with nothing changed, `sync_interface_mode` is one comparison of the game's bindings with
    /// the ones last resolved against, no clone and no resolve. Measured over many calls and printed (a debug build, so
    /// well above a release build's), with a loose bound that a clone of the bindings per frame would break.
    #[test]
    fn perf_sync_interface_mode_is_one_comparison_when_nothing_changed() {
        interface(true);
        register("alpha", "seven", "7", "Seven");
        let steady = resolve_count_for_test();
        let calls = 20_000u32;
        let start = std::time::Instant::now();
        for _ in 0..calls {
            sync_interface_mode();
        }
        let each = start.elapsed() / calls;
        println!("sync_interface_mode, nothing changed: {each:?} a call (debug build)");
        assert_eq!(resolve_count_for_test(), steady, "no resolve");
        assert!(each < std::time::Duration::from_micros(100), "{each:?} a call");
        crate::settings::testing::reset(false);
        reset_in_memory();
    }
}
