// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A2: NPC dialogs and the game's yes/no offers a mod holds (docs/plans/eqoa-api/window-replacement-design.md,
//! "NPC dialogs" and "Invitations and offers").
//!
//! A mod that holds `text_dialog`, `response_dialog`, `group_invite`, `revive_offer` or `summon_offer` (scope `all`
//! only: the wire names no speaker, so a dialog cannot be proven to answer the mod's own `talk`) gets each such
//! dialog or offer as a **session** (`window_sessions`) instead of the game's window. This registry is the fork's
//! own queue behind them, with the game's limits: plain text queues up to eleven behind the one showing; an answerable
//! dialog has one waiting place and a further one is refused to the server; invitations wait in turn. One dialog
//! session is up at a time (the game's one window mode), and the next starts when it ends and play is plain (the game's
//! `UpdateGame`). An offer (summons, resurrection) comes at once, over whatever was up, as the game's own prompt does.
//!
//! Nothing here reads or writes the app: `fork_ext/a2_play.rs` feeds it what the game received, applies what it asks
//! (give a dialog back to the game's window, answer with the game's own keys) and ends it on walk-away. A dialog that
//! ends with no answer is **given back** to the game's window (`take_give_back`), never dropped; only the old world's
//! (zoning, leaving the world) is dropped, as the game does.
use super::{
    window_sessions,
    window_takeover::{self, TakeoverKind},
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    time::{Duration, Instant},
};

/// Plain text waiting behind the one showing (`Play::show_dialog_b`: `dialogs.len() < 11`).
pub(crate) const MAX_WAITING_TEXTS: usize = 11;
/// A dialog session its mod has not touched for this long goes back to the game's window (the mod stopped showing
/// it): `eqoa.windows.session(kind)`, `eqoa.windows.takeover(kind)` again and an answer all touch it.
pub(crate) const IDLE: Duration = Duration::from_secs(120);

/// The entity the player had targeted when the words arrived: a best guess at the speaker, never proven.
pub(crate) type TalkedTo = Option<(u32, String)>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Item {
    /// A 0x0046 text (`VIWndPrivChat`), already expanded as the game does.
    Text { text: Vec<u16>, talked_to: TalkedTo },
    /// A 0x0034 dialog with answers (`VIWndResponse`): every option, not only the first page.
    Box { id: u32, text: Vec<u16>, options: Vec<Vec<u16>>, talked_to: TalkedTo },
    /// A group invitation the game parked out of its own list (`Play::group.invites`).
    Invite { id: u32, name: String, text: String },
    /// A summons (`summons`) or a resurrection offered by `name` (`Play::summon_prompt`).
    Offer { summons: bool, name: String, text: Vec<u16> },
    /// Round A11: a window the server opened (a mercenary's inventory or spells, the dye picker, a fishing session): its
    /// kind and the session's `data` (`a11_play`).
    Window { kind: TakeoverKind, data: Value },
}

fn plain(text: &[u16]) -> String {
    let end = text.iter().position(|&c| c == 0).unwrap_or(text.len());
    String::from_utf16_lossy(&text[..end])
}

impl Item {
    pub(crate) fn kind(&self) -> TakeoverKind {
        match self {
            Self::Text { .. } => TakeoverKind::TextDialog,
            Self::Box { .. } => TakeoverKind::ResponseDialog,
            Self::Invite { .. } => TakeoverKind::GroupInvite,
            Self::Offer { summons: true, .. } => TakeoverKind::SummonOffer,
            Self::Offer { summons: false, .. } => TakeoverKind::ReviveOffer,
            Self::Window { kind, .. } => *kind,
        }
    }

    /// The session's `data` (spec §7, "NPC dialogs and offers").
    pub(crate) fn data(&self) -> Value {
        let talked = |t: &TalkedTo| match t {
            Some((id, name)) => json!({"id": id, "name": name}),
            None => Value::Null,
        };
        match self {
            Self::Text { text, talked_to } => json!({"text": plain(text), "talked_to": talked(talked_to)}),
            Self::Box { text, options, talked_to, .. } => json!({
                "text": plain(text),
                "options": options.iter().map(|o| plain(o)).collect::<Vec<_>>(),
                "talked_to": talked(talked_to),
            }),
            Self::Invite { name, text, .. } => json!({"name": name, "text": text}),
            Self::Offer { name, text, .. } => json!({"from": name, "text": plain(text)}),
            Self::Window { data, .. } => data.clone(),
        }
    }

    /// How many answers `dialog_answer` accepts: a text's Done, a box's options, an offer's Yes and No; `None` for
    /// A11 server windows, which have no numbered answers (`dialog_answer` returns `unsupported`).
    pub(crate) fn answers(&self) -> Option<usize> {
        match self {
            Self::Text { .. } => Some(1),
            Self::Box { options, .. } => Some(options.len()),
            Self::Invite { .. } | Self::Offer { .. } => Some(2),
            Self::Window { .. } => None,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Active {
    pub id: String,
    pub vm: u64,
    pub item: Item,
    /// Where the player stood when it opened (walk-away is measured from here).
    pub pos: Option<[f32; 3]>,
    touched: Instant,
}

#[derive(Default)]
struct State {
    /// The text, dialog or invitation the mod is showing (the game's one window mode).
    slot: Option<Active>,
    /// The summons or resurrection offer showing (the game's message box).
    offer: Option<Active>,
    /// Round A11: the server windows held, at most one of each kind.
    wins: Vec<Active>,
    texts: VecDeque<Item>,
    boxed: Option<Item>,
    invites: VecDeque<Item>,
    give_back: Vec<Item>,
    passes: Vec<String>,
}

impl State {
    fn is_busy(&self) -> bool {
        self.slot.is_some()
            || self.offer.is_some()
            || !self.wins.is_empty()
            || !self.texts.is_empty()
            || self.boxed.is_some()
            || !self.invites.is_empty()
            || !self.give_back.is_empty()
            || !self.passes.is_empty()
    }
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
    static BUSY: Cell<bool> = const { Cell::new(false) };
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let out = f(&mut s);
        BUSY.with(|b| b.set(s.is_busy()));
        out
    })
}

/// Anything held or waiting: the per-step work has something to do.
pub(crate) fn is_busy() -> bool {
    BUSY.with(Cell::get)
}

/// Whether any mod holds a dialog or offer kind (the client's cheap test before it reads a packet's dialog).
pub(crate) fn any_holder() -> bool {
    window_takeover::has_activity() && TakeoverKind::DIALOGS.iter().any(|k| window_takeover::is_taken(*k))
}

fn holder(kind: TakeoverKind) -> Option<(u64, String)> {
    Some((window_takeover::holder_vm(kind)?, window_takeover::holder_of(kind)?))
}

/// What happens to a dialog that arrives while a mod holds its kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Intake {
    /// Nobody holds the kind: the game shows it.
    NotHeld,
    /// Held (queued or started).
    Held,
    /// Held, but the game's limit refuses it: an answerable dialog while one already waits (the client tells the
    /// server, as the game does).
    Refused,
    /// Held, and the queue is full: dropped, as the game drops a twelfth waiting text.
    Dropped,
}

/// A dialog or offer arrived. Offers start at once; the rest wait their turn (`start_next`).
pub(crate) fn hold(item: Item, pos: Option<[f32; 3]>) -> Intake {
    let kind = item.kind();
    let Some((vm, addon)) = holder(kind) else {
        return Intake::NotHeld;
    };
    match &item {
        Item::Text { .. } => {
            if with(|s| s.texts.len() >= MAX_WAITING_TEXTS) {
                return Intake::Dropped;
            }
            with(|s| s.texts.push_back(item));
        }
        Item::Box { .. } => {
            if with(|s| s.boxed.is_some()) {
                return Intake::Refused;
            }
            with(|s| s.boxed = Some(item));
        }
        Item::Invite { id, .. } => {
            let id = *id;
            with(|s| {
                let known = |i: &Item| matches!(i, Item::Invite { id: other, .. } if *other == id);
                if !s.invites.iter().any(known) && !s.slot.as_ref().is_some_and(|a| known(&a.item)) {
                    s.invites.push_back(item);
                }
            });
        }
        Item::Window { .. } => {
            // a window the server opens again while the mod holds the first: the same session, new data
            let existing = with(|s| s.wins.iter().find(|a| a.item.kind() == kind).map(|a| a.id.clone()));
            if let Some(id) = existing {
                with(|s| {
                    if let Some(a) = s.wins.iter_mut().find(|a| a.id == id) {
                        a.item = item.clone();
                        a.touched = Instant::now();
                    }
                });
                window_sessions::set_data(&id, item.data());
            } else {
                let session = window_sessions::start_dialog_session(vm, &addon, kind, item.data());
                with(|s| s.wins.push(Active { id: session.id, vm, item, pos, touched: Instant::now() }));
            }
        }
        Item::Offer { .. } => {
            // the game's own prompt replaces the one before it
            let old = with(|s| s.offer.take());
            if let Some(old) = old {
                window_sessions::end_dialog_session(&old.id, "ended");
            }
            let session = window_sessions::start_dialog_session(vm, &addon, kind, item.data());
            with(|s| s.offer = Some(Active { id: session.id, vm, item, pos, touched: Instant::now() }));
        }
    }
    Intake::Held
}

/// The next waiting dialog becomes a session when none is up and play is plain (`ready`), in the game's order: the
/// texts, then the answerable dialog, then the invitations. Returns the session's kind when one started.
pub(crate) fn start_next(ready: bool, pos: Option<[f32; 3]>) -> Option<TakeoverKind> {
    if !ready || with(|s| s.slot.is_some()) {
        return None;
    }
    let next = with(|s| {
        if holder(TakeoverKind::TextDialog).is_some()
            && let Some(i) = s.texts.pop_front()
        {
            return Some(i);
        }
        if holder(TakeoverKind::ResponseDialog).is_some()
            && let Some(i) = s.boxed.take()
        {
            return Some(i);
        }
        if holder(TakeoverKind::GroupInvite).is_some()
            && let Some(i) = s.invites.pop_front()
        {
            return Some(i);
        }
        None
    })?;
    let kind = next.kind();
    let (vm, addon) = holder(kind)?;
    let session = window_sessions::start_dialog_session(vm, &addon, kind, next.data());
    with(|s| s.slot = Some(Active { id: session.id, vm, item: next, pos, touched: Instant::now() }));
    Some(kind)
}

/// The dialog or offer a session id names, if it is still the one showing.
pub(crate) fn find(id: &str) -> Option<Active> {
    if !is_busy() {
        return None;
    }
    with(|s| {
        s.slot
            .iter()
            .chain(s.offer.iter())
            .chain(s.wins.iter())
            .find(|a| a.id == id)
            .cloned()
    })
}

/// The mod showing a session asked about it: not idle.
pub(crate) fn touch(id: &str) {
    if !is_busy() {
        return;
    }
    with(|s| {
        for a in s.slot.iter_mut().chain(s.offer.iter_mut()).chain(s.wins.iter_mut()) {
            if a.id == id {
                a.touched = Instant::now();
            }
        }
    });
}

/// The holder of a kind claimed it again: its sessions are shown.
pub(crate) fn touch_vm(vm: u64) {
    if !is_busy() {
        return;
    }
    with(|s| {
        for a in s.slot.iter_mut().chain(s.offer.iter_mut()).chain(s.wins.iter_mut()) {
            if a.vm == vm {
                a.touched = Instant::now();
            }
        }
    });
}

/// `eqoa.windows.pass(ref)`: the client puts the dialog in the game's window at the next step.
pub(crate) fn request_pass(id: &str) -> Result<(), (&'static str, String)> {
    if find(id).is_none() {
        return Err(("stale_handle", "no such open window session".into()));
    }
    with(|s| {
        if !s.passes.iter().any(|p| p == id) {
            s.passes.push(id.to_string());
        }
    });
    Ok(())
}

/// The sessions to pass: taken out of the registry, for the client to give to the game's window (then
/// `window_sessions::end_dialog_session(id, "passed")`).
pub(crate) fn take_passes() -> Vec<Active> {
    if !is_busy() {
        return Vec::new();
    }
    with(|s| {
        let ids = std::mem::take(&mut s.passes);
        let mut out = Vec::new();
        for id in ids {
            if s.slot.as_ref().is_some_and(|a| a.id == id) {
                out.extend(s.slot.take());
            } else if s.offer.as_ref().is_some_and(|a| a.id == id) {
                out.extend(s.offer.take());
            } else if let Some(i) = s.wins.iter().position(|a| a.id == id) {
                out.push(s.wins.remove(i));
            }
        }
        out
    })
}

/// The client answered (or walked away from) the session: it is over, told to its mod with `reason`.
pub(crate) fn finish(id: &str, reason: &str) {
    let gone = with(|s| {
        if s.slot.as_ref().is_some_and(|a| a.id == id) {
            s.slot.take().is_some()
        } else if s.offer.as_ref().is_some_and(|a| a.id == id) {
            s.offer.take().is_some()
        } else if let Some(i) = s.wins.iter().position(|a| a.id == id) {
            s.wins.remove(i);
            true
        } else {
            false
        }
    });
    if gone {
        window_sessions::end_dialog_session(id, reason);
    }
}

/// Called when `window_sessions` ended a dialog session on its own (zoning, leaving the world, the player's switch,
/// a stop on error...). The old world's is dropped; any other end gives the dialog back to the game's window.
pub(crate) fn session_ended(session: &window_sessions::Session, reason: &str) {
    let taken = with(|s| {
        if s.slot.as_ref().is_some_and(|a| a.id == session.id) {
            s.slot.take()
        } else if s.offer.as_ref().is_some_and(|a| a.id == session.id) {
            s.offer.take()
        } else if let Some(i) = s.wins.iter().position(|a| a.id == session.id) {
            Some(s.wins.remove(i))
        } else {
            None
        }
    });
    let Some(active) = taken else { return };
    if matches!(reason, "zoning" | "left_world") {
        return;
    }
    with(|s| s.give_back.push(active.item));
}

/// A mod is gone (disabled, reloaded, stopped by an error): what it was showing goes back to the game's window.
pub(crate) fn vm_gone(vm: u64) {
    if !is_busy() {
        return;
    }
    with(|s| {
        for a in [s.slot.take_if(|a| a.vm == vm), s.offer.take_if(|a| a.vm == vm)].into_iter().flatten() {
            s.give_back.push(a.item);
        }
        for a in std::mem::take(&mut s.wins) {
            if a.vm == vm {
                s.give_back.push(a.item);
            } else {
                s.wins.push(a);
            }
        }
    });
}

/// The character died. The game closes an open dialog or box without answering it (`Play::msg_005e` ends in
/// `set_mode_0`) and refuses every invitation with 3: the same here. Everything held or waiting is dropped, each
/// session ended `death`. Returns the invitations to refuse (shown, waiting).
pub(crate) fn death() -> Vec<u32> {
    if !is_busy() {
        return Vec::new();
    }
    let (ended, invites) = with(|s| {
        let mut ids = Vec::new();
        let mut refuse = Vec::new();
        for a in [s.slot.take(), s.offer.take()].into_iter().flatten() {
            if let Item::Invite { id, .. } = &a.item {
                refuse.push(*id);
            }
            ids.push(a.id);
        }
        for i in s.invites.drain(..).chain(s.give_back.drain(..).filter(|i| matches!(i, Item::Invite { .. }))) {
            if let Item::Invite { id, .. } = i {
                refuse.push(id);
            }
        }
        s.texts.clear();
        s.boxed = None;
        s.give_back.clear();
        s.passes.clear();
        (ids, refuse)
    });
    for id in ended {
        window_sessions::end_dialog_session(&id, "death");
    }
    invites
}

/// The world changed: the old world's dialogs and offers are gone (the game does not carry them over).
pub(crate) fn world_changed() {
    if !is_busy() {
        return;
    }
    with(|s| *s = State::default());
}

/// Per step: what a released claim, a disabled mod or the idle limit ends goes back to the game's window.
pub(crate) fn reconcile() {
    if !is_busy() {
        return;
    }
    let mut ended: Vec<(String, &'static str)> = Vec::new();
    with(|s| {
        for slot in [&mut s.slot, &mut s.offer] {
            let Some(a) = slot.as_ref() else { continue };
            let reason = if window_takeover::holder_vm(a.item.kind()) != Some(a.vm) {
                Some("ended")
            } else if a.touched.elapsed() >= IDLE {
                Some("idle")
            } else {
                None
            };
            if let Some(reason) = reason
                && let Some(a) = slot.take()
            {
                ended.push((a.id.clone(), reason));
                s.give_back.push(a.item);
            }
        }
        for a in std::mem::take(&mut s.wins) {
            let reason = if window_takeover::holder_vm(a.item.kind()) != Some(a.vm) {
                Some("ended")
            } else if a.touched.elapsed() >= IDLE {
                Some("idle")
            } else {
                None
            };
            match reason {
                Some(reason) => {
                    ended.push((a.id.clone(), reason));
                    s.give_back.push(a.item);
                }
                None => s.wins.push(a),
            }
        }
        // waiting ones whose kind nobody holds any more
        let mut keep = VecDeque::new();
        for i in std::mem::take(&mut s.texts) {
            if holder(TakeoverKind::TextDialog).is_some() {
                keep.push_back(i);
            } else {
                s.give_back.push(i);
            }
        }
        s.texts = keep;
        if s.boxed.is_some() && holder(TakeoverKind::ResponseDialog).is_none() {
            let i = s.boxed.take().unwrap();
            s.give_back.push(i);
        }
        let mut keep = VecDeque::new();
        for i in std::mem::take(&mut s.invites) {
            if holder(TakeoverKind::GroupInvite).is_some() {
                keep.push_back(i);
            } else {
                s.give_back.push(i);
            }
        }
        s.invites = keep;
    });
    for (id, reason) in ended {
        window_sessions::end_dialog_session(&id, reason);
    }
}

/// What the game's window is to be given back (the client does it in its own order).
pub(crate) fn take_give_back() -> Vec<Item> {
    if !is_busy() {
        return Vec::new();
    }
    with(|s| std::mem::take(&mut s.give_back))
}

/// The text or dialog session showing (what the player's walking away from the NPC ends).
pub(crate) fn walkers() -> Vec<Active> {
    if !is_busy() {
        return Vec::new();
    }
    with(|s| {
        s.slot
            .iter()
            .filter(|a| matches!(a.item, Item::Text { .. } | Item::Box { .. }))
            .cloned()
            .collect()
    })
}

/// New data for a held server window: told to its mod as a change.
pub(crate) fn refresh_window(id: &str, data: Value) {
    with(|s| {
        if let Some(a) = s.wins.iter_mut().find(|a| a.id == id)
            && let Item::Window { data: old, .. } = &mut a.item
        {
            *old = data.clone();
        }
    });
    window_sessions::set_data(id, data);
}

/// The held server windows (a11 reads and ends them).
pub(crate) fn windows() -> Vec<Active> {
    if !is_busy() {
        return Vec::new();
    }
    with(|s| s.wins.clone())
}

/// The invitation `id` was withdrawn by the server while parked or showing.
pub(crate) fn invite_gone(id: u32) {
    if !is_busy() {
        return;
    }
    let showing = with(|s| {
        s.invites.retain(|i| !matches!(i, Item::Invite { id: other, .. } if *other == id));
        s.slot
            .as_ref()
            .filter(|a| matches!(&a.item, Item::Invite { id: other, .. } if *other == id))
            .map(|a| a.id.clone())
    });
    if let Some(sid) = showing {
        finish(&sid, "withdrawn");
    }
}

/// The invitations still parked (the client refuses them when one is accepted).
pub(crate) fn take_parked_invites() -> Vec<u32> {
    if !is_busy() {
        return Vec::new();
    }
    with(|s| {
        s.invites
            .drain(..)
            .filter_map(|i| match i {
                Item::Invite { id, .. } => Some(id),
                _ => None,
            })
            .collect()
    })
}

#[cfg(test)]
pub(crate) fn reset_for_test() {
    STATE.with(|s| *s.borrow_mut() = State::default());
    BUSY.with(|b| b.set(false));
}

#[cfg(test)]
pub(crate) fn waiting_for_test() -> (usize, bool, usize) {
    with(|s| (s.texts.len(), s.boxed.is_some(), s.invites.len()))
}

#[cfg(test)]
pub(crate) fn age_for_test(by: Duration) {
    with(|s| {
        for a in s.slot.iter_mut().chain(s.offer.iter_mut()).chain(s.wins.iter_mut()) {
            a.touched -= by;
        }
    });
}
