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
//! death and the mod going away all end it as No. One question at a time; a second is refused `busy`. It is modal:
//! it comes over a game window too, and never to a character who is dead or out of the world.
//!
//! What a Yes does is the client's own, at the next step (`fork_ext/a1_play.rs`), re-checking that the item or quest
//! is still what the question named.
use dear_imgui_rs::{Condition, Ui, WindowFlags};
use serde_json::json;
use std::cell::RefCell;
use std::time::{Duration, Instant};

/// The question's window. After `###` is its id: a mod's own window can never take it (`reserved`).
pub(crate) const WINDOW: &str = "Are you sure?###eqoa-client-confirm";
const RESERVED_ID: &str = "###eqoa-client-";

/// A mod's window name that would be the client's question's (or any `eqoa-client` window's) id: refused, so a mod
/// cannot append to the question or take its place.
pub(crate) fn reserved(label: &str) -> bool {
    label.contains(RESERVED_ID)
}

/// How long a question waits for its answer.
pub(crate) const TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Act {
    /// `inv_index` is the game's slot for the item (what the destroy message names), `count` how many.
    Destroy { inv_index: u32, name: String, count: i64 },
    /// Bag Viewer multi-select: several whole stacks, asked about once. Each is (the game's slot, name, count). A Yes
    /// destroys them one at a time, each waiting for the server's answer to the last (`Run`), re-checking the slot.
    DestroyMany { items: Vec<(u32, String, i64)> },
    DropQuest { index: usize, title: String, stamp: u32, revision: u64 },
    /// Round A7: guild kick, accepting a friend request, accepting a guild invitation.
    GuildKick { member_id: i32, name: String },
    FriendAccept { request_id: u32, name: String },
    GuildAccept { inviter_id: u32, inviter: String, guild: String },
    /// Round A9: accepting a trade (`duel` false) or a duel's wager (`duel` true). `digest` stands for both sides'
    /// offers as the question showed them: the Yes is carried out only if they are still exactly those.
    TradeAccept { duel: bool, digest: u64 },
    /// Round A2: accepting a summons or a resurrection the game offered (`dialog_sessions`); `session` is the offer's.
    AcceptOffer { session: String, summons: bool },
    /// Round A5: spend training points, by stat in the game's order (`native_a5::STAT_NAMES`).
    TrainStats { points: [i32; 7] },
    /// Round A5: buy (`refund` false) or switch off (`refund` true) a Class Mastery skill. `class` and `parent` are
    /// the inspected skill's category and rank below, which the Master Class rule is judged by again at the Yes.
    Mastery { skill_id: i32, name: String, refund: bool, class: u8, parent: i32 },
    /// Round A10: delete a mail (`subject` is what the question named, checked again at the Yes).
    MailDelete { id: u32, subject: String },
    /// Round A10: send a mail that carries tunar or an item: (the game's pack key, the item's id, how many, its name).
    MailSend { to: String, subject: String, body: String, tunar: u32, item: Option<(u32, i64, i64, String)> },
    /// Round A10: a bid (`buyout` false) or a buyout (true) on `lot` at `price` each for `count`.
    AuctionBid { lot: i64, price: i32, count: i32, buyout: bool, name: String },
    /// Round A10: put a bank item up: its bank record key, its id, how many, the starting price, the buyout, and
    /// the time in minutes.
    AuctionSell { key: u32, id: i64, name: String, count: i32, price: i32, buyout: i32, minutes: i32 },
}

impl Act {
    pub(crate) const fn kind(&self) -> &'static str {
        match self {
            Self::Destroy { .. } => "destroy",
            Self::DestroyMany { .. } => "destroy_many",
            Self::DropQuest { .. } => "drop_quest",
            Self::GuildKick { .. } => "guild_kick",
            Self::FriendAccept { .. } => "friend_accept",
            Self::GuildAccept { .. } => "guild_accept",
            Self::TradeAccept { duel: false, .. } => "trade_accept",
            Self::TradeAccept { duel: true, .. } => "duel_accept",
            Self::AcceptOffer { summons: true, .. } => "summon_offer",
            Self::AcceptOffer { summons: false, .. } => "revive_offer",
            Self::TrainStats { .. } => "train_stats",
            Self::Mastery { refund: false, .. } => "mastery_buy",
            Self::Mastery { refund: true, .. } => "mastery_refund",
            Self::MailDelete { .. } => "mail_delete",
            Self::MailSend { .. } => "mail_send",
            Self::AuctionBid { buyout: false, .. } => "auction_bid",
            Self::AuctionBid { buyout: true, .. } => "auction_buyout",
            Self::AuctionSell { .. } => "auction_sell",
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
    /// Group destroys the player said Yes to, being carried out one stack at a time (`a1_play::step_runs`).
    runs: Vec<Run>,
}

/// A group destroy under way: what is left to destroy, and what has been done.
pub(crate) struct Run {
    pub p: Pending,
    pub queue: std::collections::VecDeque<(u32, String, i64)>,
    pub total: usize,
    pub sent: u32,
    /// Names of the stacks not destroyed (gone, changed or smaller by the time their turn came).
    pub skipped: Vec<String>,
    /// Since when the server has owed an answer to the last destroy.
    pub waiting_since: Option<Instant>,
}

impl Run {
    pub(crate) fn detail(&self, reason: Option<&str>) -> serde_json::Value {
        json!({
            "sent": self.sent > 0,
            "sent_count": self.sent,
            "total": self.total,
            "skipped": self.skipped,
            "reason": reason,
        })
    }
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
    open_with(vm, addon, session, act, text, ask_tick, false)
}

/// Round A2: the question for accepting a resurrection, which is offered to a character who is dead (so a dead
/// character may be asked; one out of the world still may not).
pub(crate) fn open_for_offer(
    vm: u64,
    addon: &str,
    session: Option<String>,
    act: Act,
    text: String,
) -> std::result::Result<u64, (&'static str, String)> {
    open_with(vm, addon, session, act, text, 0, true)
}

fn open_with(
    vm: u64,
    addon: &str,
    session: Option<String>,
    act: Act,
    text: String,
    ask_tick: u64,
    allow_dead: bool,
) -> std::result::Result<u64, (&'static str, String)> {
    // modal over whatever is up (a game window under it keeps its own keys until the player answers): never to a
    // character who is dead (unless it is the offer of a resurrection) or out of the world
    let env = super::window_sessions::env();
    if (env.dead && !allow_dead) || env.away {
        return Err(("not_ready", "the question cannot be asked while dead or out of the world".into()));
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

/// A question is up, or a Yes waits for the client to carry it out: the per-step work has something to do.
pub(crate) fn has_work() -> bool {
    STATE.with(|s| {
        let s = s.borrow();
        s.current.is_some() || !s.answered.is_empty() || !s.runs.is_empty()
    })
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

/// A Yes to a group destroy starts its run (`Act::DestroyMany` only).
pub(crate) fn begin_run(p: Pending) {
    let Act::DestroyMany { items } = &p.act else { return };
    let queue: std::collections::VecDeque<_> = items.iter().cloned().collect();
    let run = Run { total: queue.len(), queue, sent: 0, skipped: Vec::new(), waiting_since: None, p };
    STATE.with(|s| s.borrow_mut().runs.push(run));
}

/// The runs, for the client to advance one step; `put_runs` gives back the ones still going.
pub(crate) fn take_runs() -> Vec<Run> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().runs))
}

pub(crate) fn put_runs(runs: Vec<Run>) {
    STATE.with(|s| s.borrow_mut().runs.extend(runs));
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
    // a group destroy under way stops with its window: what is left is not destroyed
    let (stopped, kept): (Vec<Run>, Vec<Run>) =
        take_runs().into_iter().partition(|r| r.p.session.as_deref() == Some(session));
    put_runs(kept);
    for r in stopped {
        emit_closed(&r.p, reason, r.detail(Some(reason)));
    }
}

/// Cancels any open or queued confirm of `kind` (e.g. a 45 s request timeout cancelling a pending accept prompt).
pub(crate) fn cancel_kind(kind: &str, reason: &str) {
    let hit = STATE.with(|s| s.borrow().current.as_ref().is_some_and(|p| p.act.kind() == kind));
    if hit {
        finish(false, reason);
    }
    STATE.with(|s| s.borrow_mut().answered.retain(|p| p.act.kind() != kind));
}

pub(crate) fn cancel_for_vm(vm: u64) {
    // a mod that is gone is told nothing; its question and any Yes not yet carried out go with it
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.current.as_ref().is_some_and(|p| p.vm == vm) {
            s.current = None;
        }
        s.answered.retain(|p| p.vm != vm);
        s.runs.retain(|r| r.p.vm != vm);
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
    // a group destroy under way ends with the world: what is left is not destroyed, and the mod is told how far it got
    for r in take_runs() {
        emit_closed(&r.p, reason, r.detail(Some(reason)));
    }
}

/// Draws the question over play (the runtime's overlay frame): the text, then No and Yes, the chosen one marked.
pub(crate) fn draw(ui: &Ui) {
    let Some(p) = STATE.with(|s| s.borrow().current.clone()) else { return };
    let [w, h] = ui.io().display_size();
    let mut answer: Option<bool> = None;
    let width = (w * 0.6).clamp(280.0, 420.0);
    ui.window(WINDOW)
        // in front of every mod window, every frame it is shown (an earlier-drawn window never covers it)
        .focused(true)
        // a long list starts near the top so the buttons below it stay on the screen
        .position([(w - width) * 0.5, if p.text.lines().count() > 6 { h * 0.08 } else { h * 0.35 }], Condition::Always)
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

/// The question for a group destroy: how many stacks, then one line for each, as `destroy_text` names one.
pub(crate) fn destroy_many_text(items: &[(String, i64, bool, bool, bool)]) -> String {
    let noun = if items.len() == 1 { "item" } else { "items" };
    let mut text = format!("Destroy these {} {noun}? It cannot be undone.", items.len());
    // the window must keep Yes and No on the screen: past this many lines the rest is only counted
    for (name, count, worn, quick_slot, no_trade) in items.iter().take(MAX_LISTED) {
        text.push_str(&format!("\n- {}", if *count > 1 { format!("{count} x \"{name}\"") } else { format!("\"{name}\"") }));
        if *worn {
            text.push_str(" (worn)");
        }
        if *quick_slot {
            text.push_str(" (quick slot)");
        }
        if *no_trade {
            text.push_str(" (NO TRADE)");
        }
    }
    if items.len() > MAX_LISTED {
        text.push_str(&format!("\n...and {} more", items.len() - MAX_LISTED));
    }
    text
}

/// How many stacks a group question names one by one (the rest are counted): 12 lines with the heading and the buttons
/// fit a 720p window from the top of the screen.
pub(crate) const MAX_LISTED: usize = 12;

pub(crate) fn drop_quest_text(title: &str) -> String {
    format!("Drop the quest \"{title}\"? Its progress is lost.")
}

pub(crate) fn guild_kick_text(name: &str) -> String {
    format!("Kick \"{name}\" from the guild?")
}

pub(crate) fn friend_accept_text(name: &str) -> String {
    format!("Accept friend request from \"{name}\"?")
}

/// The question for accepting a trade or a duel's wager: what each side gives, as the window shows it now.
pub(crate) fn trade_accept_text(duel: bool, _partner: &str, gives: &[String], gets: &[String]) -> String {
    let list = |lines: &[String]| {
        if lines.is_empty() { "nothing".to_string() } else { lines.join(", ") }
    };
    format!("You give {}, you get {}. {}", list(gives), list(gets), if duel { "Start the duel?" } else { "Trade?" })
}

pub(crate) fn guild_accept_text(inviter: &str, guild: &str) -> String {
    format!("Join the guild \"{guild}\" (invited by \"{inviter}\")?")
}

/// The question for deleting a mail: what it is, and that what it carries goes with it.
pub(crate) fn mail_delete_text(from: &str, subject: &str, carries: Option<&str>) -> String {
    let mut text = format!("Delete the mail \"{subject}\" from {from}? It cannot be undone.");
    if let Some(c) = carries {
        text.push_str(&format!(" It still carries {c}."));
    }
    text
}

/// The question for a mail that carries tunar or an item.
pub(crate) fn mail_send_text(to: &str, subject: &str, tunar: u32, item: Option<(&str, i64)>, postage: u32) -> String {
    let mut parts = Vec::new();
    if tunar > 0 {
        parts.push(format!("{tunar} tunar"));
    }
    if let Some((name, count)) = item {
        parts.push(if count > 1 { format!("{count} x \"{name}\"") } else { format!("\"{name}\"") });
    }
    let subject = subject.replace('\n', " ");
    let mut text = format!("Send {} to {to} in the mail \"{subject}\"?", parts.join(" and "));
    if postage > 0 {
        text.push_str(&format!(" The postage is {postage} tunar."));
    }
    text
}

/// The question for a bid or a buyout.
pub(crate) fn auction_bid_text(name: &str, price: i32, count: i32, buyout: bool, lot_buyout: i64) -> String {
    let what = if count > 1 { format!("{count} x \"{name}\"") } else { format!("\"{name}\"") };
    if buyout {
        format!("Buy out {what} for {price} tunar each? The tunar leaves you at once.")
    } else {
        let total = if count > 1 { format!(" ({} in all)", i64::from(price) * i64::from(count)) } else { String::new() };
        let buy = if lot_buyout > 0 { format!(" Its buyout is {lot_buyout} tunar each.") } else { String::new() };
        format!("Bid {price} tunar each on {what}{total}? The tunar is held until you are outbid or win.{buy}")
    }
}

/// The question for putting an item up at the auction.
pub(crate) fn auction_sell_text(name: &str, count: i32, price: i32, buyout: i32, minutes: i32) -> String {
    let what = if count > 1 { format!("{count} x \"{name}\"") } else { format!("\"{name}\"") };
    let each = if count > 1 { " each" } else { "" };
    format!("Put {what} up at the auction from {price} tunar{each}, buyout {buyout} tunar{each}, for {} hours? It leaves your bank.", minutes / 60)
}

/// The question for spending training points: how many, and on which stats.
pub(crate) fn train_text(points: &[i32; 7]) -> String {
    let total: i32 = points.iter().sum();
    let parts: Vec<String> = points
        .iter()
        .zip(super::native_a5::STAT_NAMES)
        .filter(|(p, _)| **p > 0)
        .map(|(p, name)| format!("{name} +{p}"))
        .collect();
    let noun = if total == 1 { "point" } else { "points" };
    format!("Spend {total} training {noun} ({})? They cannot be taken back.", parts.join(", "))
}

/// The question for a Class Mastery skill: buying it (with its cost), or switching it off.
pub(crate) fn mastery_text(name: &str, cost: i32, refund: bool, master_class: bool) -> String {
    if refund {
        return format!("Switch off the Class Mastery skill \"{name}\"? The server decides what comes back, and the client cannot undo it.");
    }
    let noun = if cost == 1 { "point" } else { "points" };
    let mut text = format!("Buy the Class Mastery skill \"{name}\" for {cost} {noun}? It cannot be undone.");
    if master_class {
        text.push_str(" It is a Master Class: only one can be owned.");
    }
    text
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
