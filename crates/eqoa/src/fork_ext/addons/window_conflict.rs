// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Two mods that want the same game window: the player is told, in plain words.
//!
//! `eqoa.windows.takeover` gives a window to the first mod that asks; the second is refused `busy` (that result, and
//! the log line, are unchanged). This module remembers who was refused and who holds the window, so that
//!
//! * a small box says so once ("Dialog window: A is using it, so B's is not shown.", with OK and Open Mods & Addons),
//! * Mods & Addons shows the mod that lost a yellow line under its name, and
//! * turning the holder off clears both (`clear_vm`) and gives the loser its turn at once (`losers_of`: the manager
//!   reloads each, so its own start-up asks again; no new event, nothing the mod must be written for).
//!
//! "Once": a pair (window, holder, loser) brings the box up one time until something changes (a mod turned on or off,
//! mods reloaded, rescanned: `changed`; or a new session). A mod that asks every frame, or every time a window opens,
//! is the same conflict and adds nothing. The client never turns a mod off. Display names are looked up by the caller
//! (`name_of`), never the ids a mod is stored under.
use super::window_takeover::{self, TakeoverKind};
use dear_imgui_rs::{Condition, Ui, WindowFlags};
use std::cell::RefCell;
use std::collections::HashSet;

/// The box's window. After `###` is its id: a mod's own window can never take it (`confirm::reserved`).
pub(crate) const WINDOW: &str = "Two mods want one window###eqoa-client-window-conflict";

#[derive(Clone, Debug, PartialEq, Eq)]
struct Conflict {
    kind: TakeoverKind,
    holder_vm: u64,
    holder_id: String,
    loser_vm: u64,
    loser_id: String,
}

/// What a pair is, for "once".
type PairKey = (TakeoverKind, String, String);

#[derive(Default)]
struct State {
    /// Every refusal that still stands (the holder still holds the window).
    conflicts: Vec<Conflict>,
    /// Pairs the box has already been shown for since something last changed.
    announced: HashSet<PairKey>,
    /// What the box shows now: the conflicts it was raised for, until OK, Escape, or they no longer stand.
    boxed: Vec<Conflict>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// The player-facing name of a window: its game title and "window" ("Dialog window", "Loot window").
pub(crate) fn window_name(kind: TakeoverKind) -> String {
    match super::window_kinds::title(kind.name()) {
        Some(title) => format!("{title} window"),
        None => format!("{} window", kind.name().replace('_', " ")),
    }
}

/// A refusal: `loser` asked for `kind`, `holder` has it. The first time for a pair raises the box.
pub(crate) fn record(kind: TakeoverKind, holder: (u64, &str), loser: (u64, &str)) {
    let conflict = Conflict {
        kind,
        holder_vm: holder.0,
        holder_id: holder.1.to_string(),
        loser_vm: loser.0,
        loser_id: loser.1.to_string(),
    };
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        prune(&mut s);
        if s.conflicts.contains(&conflict) {
            return;
        }
        let key = (kind, conflict.holder_id.clone(), conflict.loser_id.clone());
        if s.announced.insert(key) {
            s.boxed.push(conflict.clone());
        }
        s.conflicts.push(conflict);
    });
}

/// Drops what no longer stands: a holder that let go (or lost the window to someone else).
fn prune(s: &mut State) {
    let stands = |c: &Conflict| window_takeover::holder_vm(c.kind) == Some(c.holder_vm);
    s.conflicts.retain(stands);
    let conflicts = std::mem::take(&mut s.conflicts);
    s.boxed.retain(|c| conflicts.contains(c));
    s.conflicts = conflicts;
}

fn live<R>(f: impl FnOnce(&State) -> R) -> R {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        prune(&mut s);
        f(&s)
    })
}

/// The box is up.
pub(crate) fn is_open() -> bool {
    live(|s| !s.boxed.is_empty())
}

/// Something changed (a mod turned on or off, mods reloaded or rescanned): every pair may be told again.
pub(crate) fn changed() {
    STATE.with(|s| s.borrow_mut().announced.clear());
}

/// A mod's machine ended (turned off, reloaded, failed): every conflict it was in, as holder or as loser, is over.
pub(crate) fn clear_vm(vm: u64) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.conflicts.retain(|c| c.holder_vm != vm && c.loser_vm != vm);
        s.boxed.retain(|c| c.holder_vm != vm && c.loser_vm != vm);
    });
}

/// The mods (by id, each once) that were refused a window `holder_id` has now: when that mod is turned off, these are
/// the ones that get their turn.
pub(crate) fn losers_of(holder_id: &str) -> Vec<String> {
    live(|s| {
        let mut out: Vec<String> = Vec::new();
        for c in s.conflicts.iter().filter(|c| c.holder_id == holder_id) {
            if !out.contains(&c.loser_id) {
                out.push(c.loser_id.clone());
            }
        }
        out
    })
}

/// The box's lines, one per conflict, by display name.
pub(crate) fn box_lines(name_of: &dyn Fn(&str) -> String) -> Vec<String> {
    live(|s| {
        s.boxed
            .iter()
            .map(|c| {
                format!(
                    "{}: {} is using it, so {}'s is not shown.",
                    window_name(c.kind),
                    name_of(&c.holder_id),
                    name_of(&c.loser_id)
                )
            })
            .collect()
    })
}

/// The yellow lines under a mod in Mods & Addons: each window it did not get and who has it.
pub(crate) fn unmet_lines(loser_id: &str, name_of: &dyn Fn(&str) -> String) -> Vec<String> {
    live(|s| {
        s.conflicts
            .iter()
            .filter(|c| c.loser_id == loser_id)
            .map(|c| format!("Did not get the {}: {} is using it.", window_name(c.kind), name_of(&c.holder_id)))
            .collect()
    })
}

/// The player closed the box (OK, or Escape). The conflicts themselves stand until a mod lets go.
pub(crate) fn dismiss() {
    STATE.with(|s| s.borrow_mut().boxed.clear());
}

/// A key the player pressed while the box is up; true when the box took it. Only Escape: it closes the box and
/// nothing else happens (every other key is the game's or the mods').
pub(crate) fn key(key: &winit::keyboard::Key) -> bool {
    use winit::keyboard::{Key, NamedKey};
    if !matches!(key, Key::Named(NamedKey::Escape)) || !is_open() {
        return false;
    }
    dismiss();
    true
}

/// Draws the box over play when it is up; returns true when the player chose Open Mods & Addons.
pub(crate) fn draw(ui: &Ui, name_of: &dyn Fn(&str) -> String) -> bool {
    let lines = box_lines(name_of);
    if lines.is_empty() {
        return false;
    }
    let [w, h] = ui.io().display_size();
    let width = (w * 0.6).clamp(280.0, 460.0);
    let (mut ok, mut open_manager) = (false, false);
    ui.window(WINDOW)
        // in front of every mod window, every frame it is shown (an earlier-drawn window never covers it)
        .focused(true)
        .position([(w - width) * 0.5, h * 0.3], Condition::Always)
        .size([width, 0.0], Condition::Always)
        .flags(WindowFlags::NO_COLLAPSE | WindowFlags::NO_MOVE | WindowFlags::NO_SAVED_SETTINGS | WindowFlags::ALWAYS_AUTO_RESIZE)
        .build(|| {
            for line in &lines {
                ui.text_wrapped(line);
            }
            ui.separator();
            ok = ui.button("OK##conflict-ok");
            ui.same_line();
            open_manager = ui.button("Open Mods & Addons##conflict-open");
            ui.text_disabled("Escape closes this");
        });
    if ok || open_manager {
        dismiss();
    }
    open_manager
}

#[cfg(test)]
pub(crate) fn reset_for_test() {
    STATE.with(|s| *s.borrow_mut() = State::default());
}
