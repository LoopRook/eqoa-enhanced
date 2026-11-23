// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A1: window sessions, "pass" and pad focus (docs/plans/eqoa-api/window-replacement-design.md, "The contract").
//!
//! A **session** is one opening of a window a mod shows: its own Bags, Spellbook or Journal (`open`), or a corpse,
//! merchant or bank the mod holds hidden (`window_takeover`, derived each frame by `sync_held`). Actions that need
//! the window (a confirm, a pad focus) belong to its session and end with it.
//!
//! **Pass** hands a held corpse, merchant or bank back to the game: its own window opens for that session as if no
//! mod held the kind (`fork_ext/a1_play.rs` does the opening). The three windows a mod opens itself hide nothing,
//! so there is nothing to pass there: `unsupported`.
//!
//! **Focus** lets an open mod window take the controller's d-pad, confirm and back while it is up (and hold the
//! sticks if it asks). It is built on the pad report the viewer already hands the fork (`prepare_controls`), and it
//! ends on close, error, disable, zoning, death and when the window loses the system's focus. It is suspended
//! (the buttons are the game's again) while any game window, the update question or Mods & Addons is up.
//!
//! Every end of a session is told to the mod that owned it, as `windows.session` with `phase = "closed"` and a
//! `reason`, except when the mod itself is gone (disabled, reloaded, crashed): there is nobody to tell.
use super::window_takeover::TakeoverKind;
use crate::input::bit;
use serde_json::json;
use std::cell::{Cell, RefCell};

/// The windows a mod may open for itself in A1 (a view: the game's window of that name stays where it is).
pub(crate) const OWN_KINDS: [&str; 5] = ["inventory", "spellbook", "quest_log", "trainer", "mastery"];
/// Most sessions one mod may hold at once.
const MAX_PER_VM: usize = 8;

/// The pad buttons a focused window gets, as `windows.key` events: the d-pad, confirm (cross) and back (triangle).
pub(crate) const FOCUS_BUTTONS: [(u16, &str); 6] = [
    (bit::UP, "up"),
    (bit::DOWN, "down"),
    (bit::LEFT, "left"),
    (bit::RIGHT, "right"),
    (bit::CROSS, "confirm"),
    (bit::TRIANGLE, "back"),
];
const FOCUS_MASK: u16 = bit::UP | bit::DOWN | bit::LEFT | bit::RIGHT | bit::CROSS | bit::TRIANGLE;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Session {
    pub id: String,
    pub kind: String,
    pub vm: u64,
    pub addon: String,
    /// `"mod"` (the mod opened its own window) or `"server"` (a corpse, merchant or bank the server opened).
    pub origin: &'static str,
    /// `"key"` or `"mod"` for a mod's own window, `"npc"` for what the server opened.
    pub opened_from: String,
    pub revision: u64,
    /// The takeover kind this session mirrors (held corpse, merchant or bank, or a dialog or offer of round A2).
    pub held: Option<TakeoverKind>,
    /// Round A2: what this one opening carries (a dialog's text and options, an offer's words); null for the rest.
    pub data: serde_json::Value,
}

impl Session {
    pub(crate) fn json(&self, focused: bool) -> serde_json::Value {
        let mut out = json!({
            "ref": self.id,
            "kind": self.kind,
            "title": super::window_kinds::title(&self.kind).unwrap_or(""),
            "origin": self.origin,
            "opened_from": self.opened_from,
            "revision": self.revision,
            "focused": focused,
        });
        out["data"] = self.data_json();
        out
    }

    /// `data` is always a table: the dialog's or offer's own fields, empty for every other window.
    pub(crate) fn data_json(&self) -> serde_json::Value {
        if self.data.is_null() { json!({}) } else { self.data.clone() }
    }
}

#[derive(Clone, Debug)]
struct Focus {
    id: String,
    vm: u64,
    movement: bool,
    /// The focus buttons held at the last report (so a press is told once, and a button held while the focus
    /// came or went is not a press).
    last: u16,
}

/// What the client says about play each frame (`fork_ext/a1_play.rs`): the default is the permissive one, so a
/// test or a build without a running game has no reason to refuse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Env {
    /// Play alone on the screen: no game window, box or map up, nothing typed into.
    pub plain: bool,
    pub dead: bool,
    /// Out of the world (loading, a world change, disconnected).
    pub away: bool,
    /// The game can open one of its own windows now (`pass`): plain play and no manual camera, which the game's
    /// windows do not open from.
    pub passable: bool,
}

impl Default for Env {
    fn default() -> Self {
        Self { plain: true, dead: false, away: false, passable: true }
    }
}

#[derive(Default)]
struct State {
    sessions: Vec<Session>,
    next_id: u64,
    next_revision: u64,
    focus: Option<Focus>,
    env: Env,
    manager_open: bool,
    passes: Vec<(u64, TakeoverKind)>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
    /// Whether any session or focus exists: the per-frame hooks cost one load when none does.
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
    static FOCUS_ON: Cell<bool> = const { Cell::new(false) };
    static OUTBOX: RefCell<Vec<(&'static str, serde_json::Value)>> = const { RefCell::new(Vec::new()) };
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let out = f(&mut s);
        ACTIVE.with(|a| a.set(!s.sessions.is_empty() || s.focus.is_some() || !s.passes.is_empty()));
        FOCUS_ON.with(|a| a.set(s.focus.is_some()));
        out
    })
}

pub(crate) fn is_active() -> bool {
    ACTIVE.with(Cell::get)
}

/// Events are queued where the registry changes (which can be inside the core's own borrow, a world reset) and
/// handed to the core at the next `flush_deferred`, the one point that is always safe.
pub(crate) fn queue_event(name: &'static str, data: serde_json::Value) {
    OUTBOX.with(|o| o.borrow_mut().push((name, data)));
}

/// Hands the queued events to the core (`native_core::flush_deferred` calls it first).
pub(crate) fn flush_outbox() {
    let items = OUTBOX.with(|o| std::mem::take(&mut *o.borrow_mut()));
    for (name, data) in items {
        super::native_core::defer(256, move |c| c.emit(name, data));
    }
}

fn emit_closed(s: &Session, reason: &str) {
    let event = json!({
        "owner": s.vm,
        "phase": "closed",
        "ref": s.id,
        "kind": s.kind,
        "origin": s.origin,
        "opened_from": s.opened_from,
        "revision": s.revision,
        "reason": reason,
        "data": s.data_json(),
    });
    super::window_sessions::queue_event("windows.session", event);
}

fn emit_opened(s: &Session) {
    let event = json!({
        "owner": s.vm,
        "phase": "opened",
        "ref": s.id,
        "kind": s.kind,
        "origin": s.origin,
        "opened_from": s.opened_from,
        "revision": s.revision,
        "reason": serde_json::Value::Null,
        "data": s.data_json(),
    });
    super::window_sessions::queue_event("windows.session", event);
}

/// A mod opens its own window of `kind`: one session per mod and kind, the same one again if it is still open.
pub(crate) fn open(
    vm: u64,
    addon: &str,
    kind: &str,
    opened_from: &str,
) -> std::result::Result<Session, (&'static str, String)> {
    if !super::window_kinds::is_window(kind) {
        return Err(("invalid_argument", format!("'{kind}' is not a window the game has")));
    }
    if !OWN_KINDS.contains(&kind) {
        return Err((
            if TakeoverKind::parse(kind).is_ok() { "invalid_argument" } else { "unsupported" },
            if TakeoverKind::parse(kind).is_ok() {
                format!("the '{kind}' window is opened by the server: hold it with eqoa.windows.takeover instead")
            } else {
                format!("a mod cannot open its own '{kind}' window in this version (only 'inventory', 'spellbook', 'quest_log', 'trainer' and 'mastery')")
            },
        ));
    }
    if super::window_takeover::use_game_windows() {
        return Err(("unavailable", "the player is using the game's windows: mod windows do not open".into()));
    }
    let env = with(|s| s.env);
    if env.dead || env.away {
        return Err(("not_ready", "no window opens while dead or out of the world".into()));
    }
    let opened_from = match opened_from {
        "key" | "mod" => opened_from,
        _ => "mod",
    };
    with(|s| {
        if let Some(existing) = s.sessions.iter().find(|x| x.vm == vm && x.kind == kind && x.held.is_none()) {
            return Ok(existing.clone());
        }
        if s.sessions.iter().filter(|x| x.vm == vm).count() >= MAX_PER_VM {
            return Err(("busy", "this mod has too many windows open".into()));
        }
        s.next_id += 1;
        s.next_revision += 1;
        let session = Session {
            id: format!("ws-{}", s.next_id),
            kind: kind.into(),
            vm,
            addon: addon.into(),
            origin: "mod",
            opened_from: opened_from.into(),
            revision: s.next_revision,
            held: None,
            data: serde_json::Value::Null,
        };
        s.sessions.push(session.clone());
        emit_opened(&session);
        Ok(session)
    })
}

/// `vm`'s session of `kind`, its own window or the corpse, merchant or bank it holds.
pub(crate) fn get(vm: u64, kind: &str) -> Option<Session> {
    if !is_active() {
        return None;
    }
    let found = with(|s| s.sessions.iter().find(|x| x.vm == vm && x.kind == kind).cloned());
    // round A2: a mod that asks for its dialog is showing it (the dialog's idle timer)
    if let Some(s) = &found
        && s.held.is_some_and(|k| TakeoverKind::DIALOGS.contains(&k))
    {
        super::dialog_sessions::touch(&s.id);
    }
    found
}

/// Some mod holds a session of `kind` (a window it opened for itself, or a held corpse, merchant or bank).
pub(crate) fn any_open(kind: &str) -> bool {
    if !is_active() {
        return false;
    }
    with(|s| s.sessions.iter().any(|x| x.kind == kind))
}

pub(crate) fn find(vm: u64, id: &str) -> Option<Session> {
    if !is_active() {
        return None;
    }
    with(|s| s.sessions.iter().find(|x| x.vm == vm && x.id == id).cloned())
}

pub(crate) fn is_focused(id: &str) -> bool {
    if !is_active() {
        return false;
    }
    with(|s| s.focus.as_ref().is_some_and(|f| f.id == id))
}

/// The mod closes its own window. A held corpse, merchant or bank closes through `release`, not here.
pub(crate) fn close(vm: u64, id: &str, reason: &str) -> std::result::Result<bool, (&'static str, String)> {
    let found = with(|s| s.sessions.iter().find(|x| x.vm == vm && x.id == id).cloned());
    let Some(session) = found else {
        return Ok(false);
    };
    if session.held.is_some() {
        return Err((
            "unsupported",
            "a held corpse, merchant or bank closes with eqoa.windows.release or eqoa.actions.close_window".into(),
        ));
    }
    end_where(reason, |x| x.id == id);
    Ok(true)
}

/// Ends every session `which` picks: told to its mod, its focus released and its question cancelled. Held
/// sessions are left (their takeover ends them); use `end_all` for the whole of them.
fn end_where(reason: &str, which: impl Fn(&Session) -> bool) -> usize {
    let ended: Vec<Session> = with(|s| {
        let (gone, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut s.sessions).into_iter().partition(|x| which(x));
        s.sessions = kept;
        if let Some(f) = &s.focus
            && gone.iter().any(|x| x.id == f.id)
        {
            s.focus = None;
        }
        gone
    });
    for session in &ended {
        super::confirm::cancel_for_session(&session.id, reason);
        emit_closed(session, reason);
        if session.held.is_some_and(|k| TakeoverKind::DIALOGS.contains(&k)) {
            super::dialog_sessions::session_ended(session, reason);
        }
    }
    ended.len()
}

/// The world changed under the mod (a zone, a new login): a held corpse, merchant or bank, a question and the
/// controller's focus end. A window the mod opened itself stays until the game says the character is out of the
/// world (`set_env`: `away`, a world change in progress), which it does for every real zone; a mod window is a view and
/// refreshes on `state.reset`.
pub(crate) fn world_changed() {
    if is_active() {
        end_where("zoning", |x| x.held.is_some());
        release_focus_all("zoning");
    }
    super::confirm::cancel_all("zoning");
}

/// Every mod window and every held session ends (death, leaving the world, the player's switch).
pub(crate) fn end_all(reason: &str) {
    if is_active() {
        end_where(reason, |_| true);
    }
    super::confirm::cancel_all(reason);
}

/// Only the windows a mod opened itself (the held corpse, merchant and bank are `window_takeover`'s to end).
pub(crate) fn end_own(reason: &str) {
    if is_active() {
        end_where(reason, |x| x.held.is_none());
    }
    super::confirm::cancel_all(reason);
}

/// No mod window can be shown or answered now (the addon runtime stopped on an error, play was left): every session
/// ends, a question is answered No and the controller is the game's again, so nothing is masked with no window to
/// use it.
pub(crate) fn mods_stopped(reason: &str) {
    end_all(reason);
    release_focus_all(reason);
}

/// `vm` is gone (disabled, reloaded, crashed): its sessions, its focus and its question go with it, told to no one.
pub(crate) fn end_vm(vm: u64) {
    if is_active() {
        with(|s| {
            s.sessions.retain(|x| x.vm != vm);
            s.passes.retain(|p| p.0 != vm);
            if s.focus.as_ref().is_some_and(|f| f.vm == vm) {
                s.focus = None;
            }
        });
    }
    super::dialog_sessions::vm_gone(vm);
    super::confirm::cancel_for_vm(vm);
}

/// The corpse, merchant and bank mods hold hidden, as sessions: told each frame by the client. `holders[kind]` is
/// the holder's `(vm, addon)` while its hidden session is up.
pub(crate) fn sync_held(holders: [Option<(u64, String)>; 3]) {
    if !is_active() && holders.iter().all(Option::is_none) {
        return;
    }
    for kind in TakeoverKind::ALL {
        let want = &holders[kind as usize];
        let have = with(|s| s.sessions.iter().find(|x| x.held == Some(kind)).cloned());
        match (have, want) {
            (Some(h), Some((vm, _))) if h.vm == *vm => {}
            (have, want) => {
                if let Some(h) = have {
                    let reason = take_pass_reason(h.vm, kind).unwrap_or("ended");
                    end_where(reason, |x| x.id == h.id);
                }
                if let Some((vm, addon)) = want {
                    with(|s| {
                        s.next_id += 1;
                        s.next_revision += 1;
                        let session = Session {
                            id: format!("ws-{}", s.next_id),
                            kind: kind.name().into(),
                            vm: *vm,
                            addon: addon.clone(),
                            origin: "server",
                            opened_from: "npc".into(),
                            revision: s.next_revision,
                            held: Some(kind),
                            data: serde_json::Value::Null,
                        };
                        s.sessions.push(session.clone());
                        emit_opened(&session);
                    });
                }
            }
        }
    }
}

/// Round A2: a dialog or offer the server sent, held for `vm` (`dialog_sessions` decides when). The session carries
/// `data`; the registry ends it with `end_dialog_session`.
pub(crate) fn start_dialog_session(vm: u64, addon: &str, kind: TakeoverKind, data: serde_json::Value) -> Session {
    with(|s| {
        s.next_id += 1;
        s.next_revision += 1;
        let session = Session {
            id: format!("ws-{}", s.next_id),
            kind: kind.name().into(),
            vm,
            addon: addon.into(),
            origin: "server",
            opened_from: "npc".into(),
            revision: s.next_revision,
            held: Some(kind),
            data,
        };
        s.sessions.push(session.clone());
        emit_opened(&session);
        session
    })
}

/// Round A2: the registry ends one of its dialog sessions (answered, passed, walked away, ...), told to the mod.
pub(crate) fn end_dialog_session(id: &str, reason: &str) {
    end_where(reason, |x| x.id == id);
}

fn take_pass_reason(vm: u64, kind: TakeoverKind) -> Option<&'static str> {
    with(|s| {
        let before = s.passes.len();
        s.passes.retain(|p| *p != (vm, kind));
        (s.passes.len() != before).then_some("passed")
    })
}

/// `eqoa.windows.pass`: the game's own window opens for this held session. Only where play is plain (the native
/// window needs the screen to itself); the client does the opening at the next step (`take_passes`).
pub(crate) fn pass(vm: u64, id: &str) -> std::result::Result<(), (&'static str, String)> {
    let Some(session) = find(vm, id) else {
        return Err(("stale_handle", "no such open window session".into()));
    };
    if session.held.is_some_and(|k| TakeoverKind::DIALOGS.contains(&k)) {
        // round A2: the game's own window shows this dialog or offer; the client does it at the next step
        return super::dialog_sessions::request_pass(&session.id);
    }
    let Some(kind) = session.held else {
        return Err((
            "unsupported",
            format!(
                "the '{}' window hides nothing of the game's, so there is nothing to pass back (only a corpse, merchant or bank the mod holds can be passed)",
                session.kind
            ),
        ));
    };
    with(|s| {
        if !s.env.plain || !s.env.passable {
            return Err(("busy", "the game's window cannot open now: another window or box is up, or the camera is manual".into()));
        }
        if !s.passes.contains(&(vm, kind)) {
            s.passes.push((vm, kind));
        }
        Ok(())
    })
}

/// The passes waiting for the client to open the game's window.
pub(crate) fn take_passes() -> Vec<(u64, TakeoverKind)> {
    if !is_active() {
        return Vec::new();
    }
    with(|s| s.passes.clone())
}

/// A pass the client could not carry out (a window of the game opened meanwhile): dropped, the session stays hidden.
pub(crate) fn cancel_pass(vm: u64, kind: TakeoverKind) {
    with(|s| s.passes.retain(|p| *p != (vm, kind)));
}

/// A pass the client has carried out (or found nothing left to pass): the session ends, told as `passed`.
pub(crate) fn finish_pass(vm: u64, kind: TakeoverKind) {
    let id = with(|s| s.sessions.iter().find(|x| x.vm == vm && x.held == Some(kind)).map(|x| x.id.clone()));
    with(|s| s.passes.retain(|p| *p != (vm, kind)));
    if let Some(id) = id {
        end_where("passed", |x| x.id == id);
    }
}

pub(crate) fn set_env(env: Env) {
    let before = with(|s| std::mem::replace(&mut s.env, env));
    if before == env {
        return;
    }
    if env.away {
        end_all("left_world");
    } else if env.dead {
        end_own("death");
        release_focus_all("death");
    }
}

pub(crate) fn env() -> Env {
    with(|s| s.env)
}

/// Mods & Addons up (the runtime says each frame).
pub(crate) fn set_manager_open(open: bool) {
    let changed = with(|s| std::mem::replace(&mut s.manager_open, open) != open);
    if changed && open {
        // the window the player opened over the mod's: its focus waits until it is closed
        with(|s| {
            if let Some(f) = s.focus.as_mut() {
                f.last = 0;
            }
        });
    }
}

/// `eqoa.windows.focus(ref, {movement})`.
pub(crate) fn focus(vm: u64, id: &str, movement: bool) -> std::result::Result<(), (&'static str, String)> {
    let Some(session) = find(vm, id) else {
        return Err(("stale_handle", "no such open window session".into()));
    };
    with(|s| {
        if !s.env.plain {
            return Err(("not_ready", "a window or box of the game is up: the controller is the game's".into()));
        }
        match &s.focus {
            Some(f) if f.id != session.id => Err((
                "busy",
                "another window already holds the controller".into(),
            )),
            _ => {
                s.focus = Some(Focus { id: session.id.clone(), vm, movement, last: 0 });
                Ok(())
            }
        }
    })
}

/// A mod gives the controller back; true when this session held it.
pub(crate) fn release_focus(vm: u64, id: &str) -> bool {
    with(|s| {
        if s.focus.as_ref().is_some_and(|f| f.vm == vm && f.id == id) {
            s.focus = None;
            true
        } else {
            false
        }
    })
}

fn release_focus_all(reason: &str) {
    let held = with(|s| s.focus.take());
    if let Some(f) = held {
        let event = json!({
            "owner": f.vm, "phase": "changed", "ref": f.id, "kind": serde_json::Value::Null, "origin": serde_json::Value::Null,
            "opened_from": serde_json::Value::Null, "revision": serde_json::Value::Null, "reason": reason,
        });
        super::window_sessions::queue_event("windows.session", event);
    }
}

/// The system took the window's focus away: the controller and held buttons are the game's again.
pub(crate) fn focus_lost() {
    if FOCUS_ON.with(Cell::get) {
        release_focus_all("focus_lost");
    }
}

/// The pad report the viewer hands the fork each frame: while a window holds the focus (and play is plain) its
/// buttons are told to it and taken out of what the game sees.
pub(crate) fn apply_pad(report: &mut crate::input::Report) {
    if !FOCUS_ON.with(Cell::get) {
        return;
    }
    let suspended = with(|s| !s.env.plain || s.manager_open) || crate::update::offer_shown();
    let events = with(|s| {
        let Some(f) = s.focus.as_mut() else {
            return Vec::new();
        };
        if suspended {
            // the game's windows and boxes have the buttons now; a button still down when they go is no press
            f.last = report.buttons & FOCUS_MASK;
            return Vec::new();
        }
        let now = report.buttons & FOCUS_MASK;
        let changed = now ^ f.last;
        f.last = now;
        report.buttons &= !FOCUS_MASK;
        if !f.movement {
            report.sticks = [0.0; 4];
        }
        FOCUS_BUTTONS
            .iter()
            .filter(|(b, _)| changed & b != 0)
            .map(|(b, name)| (f.vm, f.id.clone(), *name, now & b != 0))
            .collect()
    });
    for (vm, id, key, down) in events {
        let event = json!({"owner": vm, "ref": id, "key": key, "down": down});
        super::window_sessions::queue_event("windows.key", event);
    }
}

#[cfg(test)]
pub(crate) fn reset_for_test() {
    STATE.with(|s| *s.borrow_mut() = State::default());
    ACTIVE.with(|a| a.set(false));
    FOCUS_ON.with(|a| a.set(false));
    OUTBOX.with(|o| o.borrow_mut().clear());
    super::confirm::reset_for_test();
    super::dialog_sessions::reset_for_test();
}

#[cfg(test)]
pub(crate) fn sessions_for_test() -> Vec<Session> {
    with(|s| s.sessions.clone())
}
