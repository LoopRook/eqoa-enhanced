// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A1: the question a mod's destroy and drop-quest ask the player first.
//!
//! The owner's rule (2026-11-19/22): an irreversible step keeps its "are you sure?", starting on No, and a mod can
//! never skip it. `eqoa.actions.destroy` and `eqoa.actions.drop_quest` therefore never act: they open ONE question
//! here, drawn by the client over play, and the step is carried out only when the player answers Yes with their own
//! key press (Right or Tab to Yes, then Enter) or their own click on the Yes button. A Yes comes from nowhere else:
//! there is no function a mod can call to answer, to move the choice or to extend the time; a mod's synthetic input
//! is not delivered here (`Runtime::route_key` skips synthetic key events, a click is the window system's pointer
//! event on the client's own button). Escape, No, the time running out (`TIMEOUT`), the mod's window closing, zoning,
//! death and the mod going away all end it as No. One question at a time; a second is refused `busy`.
//!
//! What a Yes does is the client's own, at the next step (`fork_ext/a1_play.rs`), re-checking that the item or quest
//! is still what the question named.
use dear_imgui_rs::{Condition, Ui, WindowFlags};
use serde_json::json;
use std::cell::RefCell;
use std::time::{Duration, Instant};

/// How long a question waits for its answer.
pub(crate) const TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Act {
    /// `inv_index` is the game's slot for the item (what the destroy message names), `count` how many.
    Destroy { inv_index: u32, name: String, count: i64 },
    DropQuest { index: usize, title: String, stamp: u32, revision: u64 },
}

impl Act {
    pub(crate) const fn kind(&self) -> &'static str {
        match self {
            Self::Destroy { .. } => "destroy",
            Self::DropQuest { .. } => "drop_quest",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Pending {
    pub id: u64,
    pub vm: u64,
    pub addon: String,
    /// The mod window (session) the question belongs to, when the mod has one open.
    pub session: Option<String>,
    pub act: Act,
    pub text: String,
    pub opened: Instant,
    /// The highlighted choice. Always starts false (No).
    pub yes: bool,
    pub ask_tick: u64,
}

#[derive(Default)]
struct State {
    current: Option<Pending>,
    next_id: u64,
    /// Yes answers waiting for the client to carry them out.
    answered: Vec<Pending>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn emit_closed(p: &Pending, answer: &str, detail: serde_json::Value) {
    let event = json!({
        "owner": p.vm,
        "confirm": p.id,
        "kind": p.act.kind(),
        "answer": answer,
        "detail": detail,
    });
    super::window_sessions::queue_event("confirm.closed", event);
}

/// A mod's request to ask. Refused `busy` while another question is up, `not_ready` where play is not plain.
pub(crate) fn open(
    vm: u64,
    addon: &str,
    session: Option<String>,
    act: Act,
    text: String,
    ask_tick: u64,
) -> std::result::Result<u64, (&'static str, String)> {
    let env = super::window_sessions::env();
    if env.dead || env.away || !env.plain {
        return Err(("not_ready", "the question can only be asked in plain play".into()));
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.current.is_some() {
            return Err(("busy", "another question is already waiting for the player".into()));
        }
        s.next_id += 1;
        let id = s.next_id;
        s.current = Some(Pending {
            id,
            vm,
            addon: addon.into(),
            session,
            act,
            text,
            opened: Instant::now(),
            yes: false,
            ask_tick,
        });
        Ok(id)
    })
}

pub(crate) fn is_open() -> bool {
    STATE.with(|s| s.borrow().current.is_some())
}

/// The question `vm` has up, if any.
pub(crate) fn current_for(vm: u64) -> Option<Pending> {
    STATE.with(|s| s.borrow().current.clone().filter(|p| p.vm == vm))
}

/// The answer ends the question: Yes goes to the client's work list, anything else is told to the mod.
fn finish(yes: bool, reason: &str) {
    let taken = STATE.with(|s| s.borrow_mut().current.take());
    let Some(p) = taken else { return };
    if yes {
        STATE.with(|s| s.borrow_mut().answered.push(p));
    } else {
        emit_closed(&p, reason, serde_json::Value::Null);
    }
}

/// The player's answer by the mouse: a click on Yes or No.
pub(crate) fn click(yes: bool) {
    if is_open() {
        finish(yes, "no");
    }
}

/// A key the player pressed while the question is up; true when the question took it (so the game does not).
/// Left, Right and Tab move the choice, Enter answers what is chosen, Escape answers No. Every other key is
/// the game's: the question is not a game window and never holds the walk.
pub(crate) fn key(key: &winit::keyboard::Key) -> bool {
    use winit::keyboard::{Key, NamedKey};
    let Key::Named(name) = key else { return false };
    let handled = STATE.with(|s| {
        let mut s = s.borrow_mut();
        let Some(p) = s.current.as_mut() else { return None };
        match name {
            NamedKey::ArrowLeft => p.yes = false,
            NamedKey::ArrowRight => p.yes = true,
            NamedKey::Tab => p.yes = !p.yes,
            NamedKey::Enter => return Some(Some(p.yes)),
            NamedKey::Escape => return Some(Some(false)),
            _ => return None,
        }
        Some(None)
    });
    match handled {
        None => false,
        Some(None) => true,
        Some(Some(yes)) => {
            finish(yes, "no");
            true
        }
    }
}

/// Time running out ends the question as No; returns whether one expired.
pub(crate) fn expire() -> bool {
    let over = STATE.with(|s| s.borrow().current.as_ref().is_some_and(|p| p.opened.elapsed() >= TIMEOUT));
    if over {
        finish(false, "expired");
    }
    over
}

/// Takes the Yes answers for the client to carry out.
pub(crate) fn take_answered() -> Vec<Pending> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().answered))
}

/// The client carried a Yes out (or could not): told to the mod.
pub(crate) fn report_done(p: &Pending, answer: &str, detail: serde_json::Value) {
    emit_closed(p, answer, detail);
}

pub(crate) fn cancel_for_session(session: &str, reason: &str) {
    let hit = STATE.with(|s| s.borrow().current.as_ref().is_some_and(|p| p.session.as_deref() == Some(session)));
    if hit {
        finish(false, reason);
    }
    STATE.with(|s| s.borrow_mut().answered.retain(|p| p.session.as_deref() != Some(session)));
}

pub(crate) fn cancel_for_vm(vm: u64) {
    // a mod that is gone is told nothing; its question and any Yes not yet carried out go with it
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.current.as_ref().is_some_and(|p| p.vm == vm) {
            s.current = None;
        }
        s.answered.retain(|p| p.vm != vm);
    });
}

pub(crate) fn cancel_all(reason: &str) {
    if is_open() {
        finish(false, reason);
    }
    // a Yes the player gave a moment before the world changed is not carried into the next one
    let stale = STATE.with(|s| std::mem::take(&mut s.borrow_mut().answered));
    for p in stale {
        emit_closed(&p, reason, serde_json::Value::Null);
    }
}

/// Draws the question over play (the runtime's overlay frame): the text, then No and Yes, the chosen one marked.
pub(crate) fn draw(ui: &Ui) {
    let Some(p) = STATE.with(|s| s.borrow().current.clone()) else { return };
    let [w, h] = ui.io().display_size();
    let mut answer: Option<bool> = None;
    let width = (w * 0.6).clamp(280.0, 420.0);
    ui.window("Are you sure?")
        .position([(w - width) * 0.5, h * 0.35], Condition::Always)
        .size([width, 0.0], Condition::Always)
        .flags(WindowFlags::NO_COLLAPSE | WindowFlags::NO_MOVE | WindowFlags::NO_SAVED_SETTINGS | WindowFlags::ALWAYS_AUTO_RESIZE)
        .build(|| {
            ui.text_wrapped(&p.text);
            ui.text_disabled(format!("{} asks this", p.addon));
            ui.separator();
            let mark = |chosen: bool, label: &str| if chosen { format!("> {label} <") } else { label.to_string() };
            if ui.button(format!("{}##confirm-no", mark(!p.yes, "No"))) {
                answer = Some(false);
            }
            ui.same_line();
            if ui.button(format!("{}##confirm-yes", mark(p.yes, "Yes"))) {
                answer = Some(true);
            }
            ui.text_disabled("Left or Right to choose, Enter to answer, Escape for No");
        });
    if let Some(yes) = answer {
        click(yes);
    }
}

/// The text of the question for an item (words only; the game's own is longer and in its own language table).
pub(crate) fn destroy_text(name: &str, count: i64, worn: bool, quick_slot: bool, no_trade: bool) -> String {
    let what = if count > 1 { format!("{count} x \"{name}\"") } else { format!("\"{name}\"") };
    let mut text = format!("Destroy {what}? It cannot be undone.");
    if worn {
        text.push_str(" It is worn.");
    }
    if quick_slot {
        text.push_str(" It is on a quick slot.");
    }
    if no_trade {
        text.push_str(" It is NO TRADE: it cannot be given back.");
    }
    text
}

pub(crate) fn drop_quest_text(title: &str) -> String {
    format!("Drop the quest \"{title}\"? Its progress is lost.")
}

#[cfg(test)]
pub(crate) fn reset_for_test() {
    STATE.with(|s| *s.borrow_mut() = State::default());
}

/// Makes the open question `by` older (the time running out, without waiting for it).
#[cfg(test)]
pub(crate) fn age_for_test(by: Duration) {
    STATE.with(|s| {
        if let Some(p) = s.borrow_mut().current.as_mut() {
            p.opened -= by;
        }
    });
}

#[cfg(test)]
pub(crate) fn current_for_test() -> Option<Pending> {
    STATE.with(|s| s.borrow().current.clone())
}
