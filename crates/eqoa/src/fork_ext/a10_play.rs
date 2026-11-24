// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A10 on `Play`: mail and the auction for a mod window (`addons::native_a10`). A child of the Play adapter, so
//! it reads `Play`'s private state (`mail`, `bank`, `dump`); it adds one line to `bankplay.rs` (the auction replies'
//! "the search window is up" test also holds while a mod holds its own auction window).
//!
//! Mail needs no line: the game's gram client reads its replies (0x0D41) in every mode, and its state machine
//! (`Grams::state`) is the one pending request. The auction does not: a search page that arrives while the game's
//! search window (mode 39) is not up is cleared at once (`Auction::recv_results`), so a mod's window, which leaves
//! the mode at 0, would never see a page. While a mod holds an `auction_search` window the test is true.
//!
//! The actions are the game's own requests: `Grams`' and `Auction`'s builders, one request at a time as the game has
//! it. Nothing is ever resent: an action that finds one still out is refused `busy`, and a request the server never
//! answers is given up on after `GIVE_UP` (told to the mod as `uncertain`, never replayed). Every step that moves
//! money, items or mail that cannot be had back asks the player first (`confirm.rs`) and is carried out only on
//! their Yes, after checking again that the mail, lot or item is still what the question named.
use super::Play;
use crate::auction::{Auction, Query};
use crate::fork_ext::addons::{
    action_broker::{self, DispatchOutcome},
    confirm::{self, Act},
    native_a10::{self, A10Action, MailItem, SearchArgs},
    window_sessions, window_takeover,
};
use serde_json::{Value as Json, json};
use std::cell::RefCell;
use std::time::{Duration, Instant};

const MAIL_KINDS: [&str; 2] = ["mail_inbox", "mail_compose"];
const AUCTION_KIND: &str = "auction_search";

/// How long a request waits for its answer before the fork gives up on it (never resends): the game's own windows
/// wait for ever, a mod's must not.
const GIVE_UP: Duration = Duration::from_secs(15);

/// What the client keeps of the auction request a mod made: which list the page is of, and the request out.
#[derive(Default)]
struct Local {
    mail_open: bool,
    auction_open: bool,
    /// the auction window's list: "search", "lots" or "bids"
    mode: &'static str,
    /// a page request is out (search, page, lots, bids): when it was made
    awaiting: Option<Instant>,
    /// a sell, bid or bidders request is out: the game's `bank.state` it set, and when
    out: Option<(i32, Instant)>,
    /// the lot and the bank record a request out is about (a bid, a sale)
    out_target: (Option<i64>, Option<u32>),
    /// a request was given up on after `GIVE_UP`: it may or may not have happened
    uncertain: Option<Uncertain>,
    /// a Take of a mail's contents is out: which mail, and when
    taking: Option<(u32, Instant)>,
}

/// A request the fork gave up on. It stays until a fresh page shows the lots again, and until then the same lot is
/// not bid on again and the same bank record is not put up again.
#[derive(Clone)]
struct Uncertain {
    kind: &'static str,
    lot: Option<i64>,
    key: Option<u32>,
    /// the game's page counter when it was given up on: a later, answered page is a fresh read
    page: i32,
}

thread_local! {
    static LOCAL: RefCell<Local> = RefCell::new(Local::default());
}

fn refuse(code: &'static str, reason: impl Into<String>) -> DispatchOutcome {
    DispatchOutcome::Refused { code, reason: reason.into() }
}

fn sent() -> DispatchOutcome {
    DispatchOutcome::Completed { detail: Some(json!({"sent": true, "confirmed": false})) }
}

/// A mod holds its own auction window: the replies are read as the game's search window would (the one line in
/// `bankplay.rs`).
pub(crate) fn auction_reading() -> bool {
    window_sessions::any_open_of(&[AUCTION_KIND])
}

fn mail_window_open() -> bool {
    window_sessions::any_open_of(&MAIL_KINDS)
}

/// The game's own mail or tell window is up (the gram client is shared with it).
fn game_mail_up(play: &Play) -> bool {
    play.mail_owns()
}

/// The game's own auction search or bank window is up.
fn game_auction_up(play: &Play) -> bool {
    play.mode == 39 || play.mode == 11
}

fn caller_holds(kind: &str) -> bool {
    action_broker::current_dispatch_vm().is_some_and(|vm| window_sessions::get(vm, kind).is_some())
}

fn offered(play: &Play) -> bool {
    crate::live::features().auction(play.dump.bank.p78cc)
}

fn live() -> bool {
    crate::live::features().live()
}

fn utf16(s: &[u16]) -> String {
    String::from_utf16_lossy(&s[..s.iter().position(|&c| c == 0).unwrap_or(s.len())])
}

fn bytes8(s: &[u8]) -> String {
    String::from_utf8_lossy(&s[..s.iter().position(|&c| c == 0).unwrap_or(s.len())]).into_owned()
}

#[cfg(test)]
thread_local! {
    /// Every request the actions sent, as the bytes the session was handed (the tests compare them).
    pub(super) static SENT: RefCell<Vec<Vec<u8>>> = const { RefCell::new(Vec::new()) };
}

fn send(play: &mut Play, m: Vec<u8>) {
    #[cfg(test)]
    SENT.with(|s| s.borrow_mut().push(m.clone()));
    play.session.send(&m);
}

/// What the gram client queued, handed to the server now (the game's own `run_mail` does the same each frame).
fn flush_mail(play: &mut Play) {
    for m in std::mem::take(&mut play.mail.grams.out) {
        send(play, m);
    }
}

// ---- mail ----

fn status_name(state: i32) -> &'static str {
    match state {
        0 => "idle",
        1 => "waiting_inbox",
        2 => "waiting_message",
        3 => "waiting_send",
        4 => "waiting_delete",
        -1 => "send_unknown_recipient",
        -2 => "send_mailbox_full",
        -3 => "send_too_many_unread",
        -4 => "send_mail_busy",
        -5 => "send_failed",
        -6 => "inbox_failed",
        -7 => "read_failed",
        -8 => "delete_failed",
        -9 => "send_not_delivered",
        s if s > 0 => "waiting",
        _ => "failed",
    }
}

/// The items of the pack a mail may carry, in pack order (`mailattach::mailable`): a mod's `pack` rows.
fn pack_rows(play: &Play) -> Vec<&net::dump::NetItem> {
    play.dump.inventory.items.iter().filter(|it| crate::mailattach::mailable(it)).collect()
}

fn mail_snapshot(play: &Play) -> Json {
    let g = &play.mail.grams;
    let n = g.count.clamp(0, 100) as usize;
    let entries: Vec<Json> = g
        .headers
        .iter()
        .take(n)
        .map(|h| {
            json!({
                "id": h.id,
                "from": bytes8(&h.from),
                "subject": utf16(&h.subject),
                "date": h.date.format(),
                "read": h.read != 0,
                "carries": g.attach.marked(h.id),
            })
        })
        .collect();
    let message = (g.message.id != 0 && !g.text.is_empty() && g.headers.iter().take(n).any(|h| h.id == g.message.id)).then(|| {
        let carried = g.attach.carried_by(g.message.id);
        json!({
            "id": g.message.id,
            "from": bytes8(&g.message.from),
            "subject": utf16(&g.message.subject),
            "date": g.message.date.format(),
            "text": utf16(&g.text),
            "to": utf16(&g.to),
            "tunar": carried.map_or(0, |c| c.tunar),
            "items": carried.map(|c| c.items.iter().map(|i| if i.quantity > 1 { format!("{} {}", i.quantity, i.item.name) } else { i.item.name.clone() }).collect::<Vec<_>>()).unwrap_or_default(),
            "returned": carried.is_some_and(|c| c.returned),
        })
    });
    let pack: Vec<Json> = pack_rows(play)
        .iter()
        .enumerate()
        .map(|(i, it)| json!({"slot": i + 1, "id": it.item.id, "name": it.item.name, "quantity": it.quantity.max(1)}))
        .collect();
    let offer = if g.live { g.attach.offer } else { None };
    json!({
        "revision": 0,
        "offered": true,
        "status": status_name(g.state),
        "count": g.count.max(0),
        "truncated": g.count > 100,
        "new": g.x08 != 0,
        "entries": entries,
        "message": message,
        "pack": pack,
        "carries_offer": offer.is_some(),
        "postage": offer.map_or(0, |o| o.postage),
        "max_items": offer.map_or(0, |o| o.items),
        "tunar": play.dump.player.p34,
        "names": g.names.iter().map(|n| bytes8(n)).collect::<Vec<_>>(),
        "delivered": g.delivered,
        "refusal": g.attach.refusal,
    })
}

/// The mail window's opening, as the game's inbox does: the inbox asked for when nothing is waited for.
fn mail_on_open(play: &mut Play) {
    if play.mail.grams.state <= 0 {
        play.mail.grams.request_inbox();
        flush_mail(play);
    }
}

// ---- auction ----

fn state_name(state: i32) -> &'static str {
    match state {
        0 => "none",
        1 => "money",
        2 => "move",
        3 => "sell",
        4 => "bid",
        5 => "search",
        6 => "bidders",
        _ => "other",
    }
}

/// Minutes left of a lot on the server's clock, when a page has set it.
fn minutes_left(a: &Auction, lot_end: i64, now: i64) -> Option<i64> {
    (a.arrived >= 0).then(|| lot_end - a.get_minutes(now))
}

fn auction_snapshot(play: &Play) -> Json {
    let a = &play.bank.auction;
    let now = play.session.now() as i64;
    let live = live();
    let (mode, awaiting, uncertain) = LOCAL.with(|l| {
        let l = l.borrow();
        (l.mode, l.awaiting.is_some(), l.uncertain.as_ref().map(|u| u.kind))
    });
    let pending = if awaiting { mode_request(mode) } else { state_name(play.bank.state) };
    let mine = |lot: i64| play.dump.bank.lots.iter().any(|l| l.lot_id == lot);
    let results: Vec<Json> = a
        .results
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let col = |k: usize| a.cols[k].get(i).copied().unwrap_or(0);
            json!({
                "lot": l.lot_id,
                "name": l.item.name,
                "quantity": l.quantity,
                "seller": l.seller,
                "min_bid": l.p2f4,
                "buyout": l.p2f8,
                "minutes_left": minutes_left(a, l.p2fc, now),
                "high_bid": (col(1) != -1).then(|| col(1)),
                "leader": (!live).then(|| col(0)),
                "bids": (!live).then(|| col(2)),
                "next_bid": live.then(|| col(2)),
                "mine": mine(l.lot_id),
            })
        })
        .collect();
    let lots: Vec<Json> = play
        .dump
        .bank
        .lots
        .iter()
        .map(|l| json!({"lot": l.lot_id, "name": l.item.name, "quantity": l.quantity, "buyout": l.p2f8, "minutes_left": minutes_left(a, l.p2fc, now)}))
        .collect();
    let bids: Vec<Json> = play
        .dump
        .bank
        .bids
        .iter()
        .map(|b| json!({"lot": b.auction_id, "price": b.x08, "count": b.x0c, "escrow": b.x10}))
        .collect();
    let bidders: Vec<Json> = a.bidders.iter().map(|b| json!({"name": b.name, "bid": b.x24})).collect();
    // what can be put up: the bank's rows, only while a mod holds the bank (selling is a page of the bank window)
    let sellable: Vec<Json> = if window_takeover::hidden_bank_session().is_some() {
        play.dump
            .bank
            .items
            .items
            .iter()
            .enumerate()
            .filter(|(_, it)| crate::auction::can_sell(it))
            .map(|(i, it)| json!({"slot": i + 1, "id": it.item.id, "name": it.item.name, "quantity": it.quantity.max(1), "value": it.item.value}))
            .collect()
    } else {
        Vec::new()
    };
    json!({
        "revision": 0,
        "offered": offered(play),
        "mode": mode,
        "pending": pending,
        "page": a.page,
        "results": results,
        "minutes": (a.arrived >= 0).then(|| a.get_minutes(now)),
        "lots": lots,
        "bids": bids,
        "bidders_lot": (!a.bidders.is_empty() || a.p78bc >= 0).then_some(a.p78bc),
        "bidders": bidders,
        "sellable": sellable,
        "tunar": play.dump.player.p34,
        "banked": play.dump.player.p38,
        "uncertain": uncertain,
    })
}

fn mode_request(mode: &str) -> &'static str {
    match mode {
        "lots" => "lots",
        "bids" => "bids",
        _ => "search",
    }
}

fn auction_on_open(play: &mut Play) {
    // `SearchWnd::open`: the page and the one before it emptied
    play.bank.auction.clear_results();
    play.bank.auction.clear_saved();
    LOCAL.with(|l| {
        let mut l = l.borrow_mut();
        l.mode = "search";
        l.awaiting = None;
        // a sale or bid still out is the bank's, not this window's: opening does not forget it
    });
}

fn auction_on_close(play: &mut Play) {
    // `CloseAuctionSearch` without its start menu. The game's own window, when it is up, owns the page: the mod's
    // window going (closed, disabled, a Lua error, death) never empties what the player is looking at
    if !game_auction_up(play) {
        play.bank.auction.clear_results();
        play.bank.auction.clear_saved();
    }
    // a sale or bid still out stays waited for (`out`): it is the bank's, not this window's
    LOCAL.with(|l| l.borrow_mut().awaiting = None);
    native_a10::clear_auction();
}

/// A request that has had no answer for `GIVE_UP` is given up on: told to the mod, never sent again.
fn give_up_stale(play: &mut Play) {
    let (page_stale, out_stale) = LOCAL.with(|l| {
        let l = l.borrow();
        (l.awaiting.is_some_and(|t| t.elapsed() >= GIVE_UP), l.out.is_some_and(|(_, t)| t.elapsed() >= GIVE_UP))
    });
    if page_stale || out_stale {
        // the game's own reset for a request that will not be answered (`bank_reset`), unless its own window is up:
        // that window's request is not the mod's to forget
        if !game_auction_up(play) {
            play.bank.state = 0;
        }
        let page = play.bank.auction.page;
        LOCAL.with(|l| {
            let mut l = l.borrow_mut();
            let kind = if out_stale { l.out.map_or("request", |(s, _)| state_name(s)) } else { mode_request(l.mode) };
            let (lot, key) = if out_stale { l.out_target } else { (None, None) };
            l.uncertain = Some(Uncertain { kind, lot, key, page });
            l.awaiting = None;
            l.out = None;
        });
    }
}

/// Each game step's work: windows opening and closing, requests answered or given up on, and the snapshots a mod
/// reads.
pub(crate) fn tick(play: &mut Play, game_step: bool) {
    let mail = mail_window_open();
    let auction = auction_reading();
    let (was_mail, was_auction) = LOCAL.with(|l| {
        let l = l.borrow();
        (l.mail_open, l.auction_open)
    });
    let out_pending = LOCAL.with(|l| {
        let l = l.borrow();
        l.awaiting.is_some() || l.out.is_some()
    });
    if !mail && !auction && !was_mail && !was_auction && !out_pending {
        return;
    }
    if mail && !was_mail && !game_mail_up(play) {
        mail_on_open(play);
    }
    if !mail && was_mail {
        native_a10::clear_mail();
    }
    if auction && !was_auction && !game_auction_up(play) {
        auction_on_open(play);
    }
    if !auction && was_auction {
        auction_on_close(play);
    }
    LOCAL.with(|l| {
        let mut l = l.borrow_mut();
        l.mail_open = mail;
        l.auction_open = auction;
    });
    // a fresh page (a new counter, answered) is a fresh read of the lots: what was uncertain is known again
    if play.bank.auction.p7860 == 0 {
        let page = play.bank.auction.page;
        LOCAL.with(|l| {
            let mut l = l.borrow_mut();
            if l.uncertain.as_ref().is_some_and(|u| u.page != page) {
                l.uncertain = None;
            }
        });
    }
    // an answer ends the wait: a page arrived, or the game's own request state moved on. A sale or a bid is waited
    // for whether or not the auction window is still open (the bank's sell is a page of the bank window)
    if play.bank.auction.p7860 == 0 {
        LOCAL.with(|l| l.borrow_mut().awaiting = None);
    }
    LOCAL.with(|l| {
        let mut l = l.borrow_mut();
        if l.out.is_some_and(|(s, _)| s != play.bank.state) {
            l.out = None;
        }
    });
    give_up_stale(play);
    if mail && !game_mail_up(play) && (game_step || native_a10::current_mail().is_none()) {
        native_a10::publish_mail(mail_snapshot(play));
    }
    if auction && !game_auction_up(play) && (game_step || native_a10::current_auction().is_none()) {
        native_a10::publish_auction(auction_snapshot(play));
    }
}

/// The world is let go: nothing of a mail or auction window is left to read.
pub(crate) fn dropped() {
    LOCAL.with(|l| *l.borrow_mut() = Local::default());
    native_a10::clear_mail();
    native_a10::clear_auction();
}

pub(crate) fn dispatch(play: &mut Play, action: &A10Action) -> DispatchOutcome {
    let out = dispatch_inner(play, action);
    if mail_window_open() && !game_mail_up(play) {
        native_a10::publish_mail(mail_snapshot(play));
    }
    if auction_reading() && !game_auction_up(play) {
        native_a10::publish_auction(auction_snapshot(play));
    }
    out
}

fn dispatch_inner(play: &mut Play, action: &A10Action) -> DispatchOutcome {
    if play.is_dead() {
        return refuse("not_ready", "cannot do this while dead");
    }
    match action {
        A10Action::MailRefresh
        | A10Action::MailRead { .. }
        | A10Action::MailDelete { .. }
        | A10Action::MailTake { .. } => mail_inbox_action(play, action),
        A10Action::MailSend { .. } => mail_send(play, action),
        A10Action::AuctionSell { .. } => auction_sell(play, action),
        _ => auction_action(play, action),
    }
}

// ---- mail actions ----

fn mail_ready(play: &Play, kind: &str) -> Result<(), DispatchOutcome> {
    if !caller_holds(kind) {
        return Err(refuse(
            "not_ready",
            format!("open the mail window first (eqoa.windows.open_session '{kind}'): the gram client is read only while one is open"),
        ));
    }
    if game_mail_up(play) {
        return Err(refuse("busy", "the game's own mail window is up"));
    }
    if play.mail.grams.state > 0 {
        return Err(refuse("busy", "the last mail request has not been answered yet"));
    }
    Ok(())
}

fn listed(play: &Play, id: u32) -> Option<&crate::gram::Header> {
    let g = &play.mail.grams;
    g.headers.iter().take(g.count.clamp(0, 100) as usize).find(|h| h.id == id)
}

fn mail_inbox_action(play: &mut Play, action: &A10Action) -> DispatchOutcome {
    if let Err(e) = mail_ready(play, "mail_inbox") {
        return e;
    }
    match action {
        A10Action::MailRefresh => {
            play.mail.grams.request_inbox();
            flush_mail(play);
            sent()
        }
        A10Action::MailRead { id } => {
            if listed(play, *id).is_none() {
                return refuse("stale_handle", "that mail is not in the inbox now");
            }
            play.mail.grams.request_message(*id);
            flush_mail(play);
            sent()
        }
        A10Action::MailTake { id } => {
            if listed(play, *id).is_none() {
                return refuse("stale_handle", "that mail is not in the inbox now");
            }
            if play.mail.grams.attach.carried_by(*id).is_none() {
                return refuse("not_ready", "read the mail first: the server has not said it carries anything");
            }
            // one Take at a time: until the server says what the mail carries now (or 15 s)
            let taking = LOCAL.with(|l| l.borrow().taking);
            if let Some((t, since)) = taking
                && since.elapsed() < GIVE_UP
                && play.mail.grams.attach.carried_by(t).is_some()
            {
                return refuse("busy", "the last Take has not been answered yet");
            }
            LOCAL.with(|l| l.borrow_mut().taking = Some((*id, Instant::now())));
            play.mail.grams.out.push(crate::mailattach::take(*id));
            flush_mail(play);
            sent()
        }
        A10Action::MailDelete { id } => {
            let Some(h) = listed(play, *id) else {
                return refuse("stale_handle", "that mail is not in the inbox now");
            };
            let carries = play.mail.grams.attach.carried_by(*id).map(|c| c.words());
            let carries = carries.or_else(|| play.mail.grams.attach.marked(*id).then(|| "something".to_string()));
            let text = confirm::mail_delete_text(&bytes8(&h.from), &utf16(&h.subject), carries.as_deref());
            let act = Act::MailDelete { id: *id, subject: utf16(&h.subject) };
            super::a1_play::ask(act, text, "mail_inbox")
        }
        _ => refuse("unsupported", "not an inbox action"),
    }
}

/// The names a To line stands for, as the game's compose sends them: the live client's list, or the disc's one name.
/// `live` is the gram client's own (the server's offer of the live forms), as the compose window reads it.
fn recipients(to: &str, live: bool) -> Vec<Vec<u8>> {
    let w = crate::ui::wide(to);
    if live {
        return crate::gram::parse_recipients(&w);
    }
    // `Compose::select`: str16to8 into 24 bytes, then CapitalizeName
    let mut name: Vec<u8> = w.iter().take(23).map(|&c| if c < 256 { c as u8 } else { b'?' }).collect();
    name.make_ascii_uppercase();
    if name.len() > 1 {
        name[1..].make_ascii_lowercase();
    }
    if name.is_empty() { Vec::new() } else { vec![name] }
}

fn mail_send(play: &mut Play, action: &A10Action) -> DispatchOutcome {
    let A10Action::MailSend { to, subject, body, tunar, item } = action else {
        return refuse("unsupported", "not a send");
    };
    if let Err(e) = mail_ready(play, "mail_compose") {
        return e;
    }
    // the game's own checks, in its order (words 1632, 1808, 873)
    let names = recipients(to, play.mail.grams.live);
    if names.is_empty() {
        return refuse("invalid_argument", "to names nobody");
    }
    if subject.trim().is_empty() {
        return refuse("invalid_argument", "the mail needs a subject");
    }
    if body.trim().is_empty() {
        return refuse("invalid_argument", "the mail needs some text");
    }
    let gram_live = play.mail.grams.live;
    let limit = if gram_live { 511 } else { 512 };
    if body.chars().count() > limit {
        return refuse("out_of_range", format!("the text is longer than {limit} characters"));
    }
    let subject_w = crate::ui::wide(subject);
    let body_w = crate::ui::wide(body);
    if *tunar == 0 && item.is_none() {
        // plain mail: the game's compose has no question
        let g = &mut play.mail.grams;
        if gram_live {
            g.send_gram_live(&names, &subject_w, &body_w);
        } else {
            g.send_gram(&names[0], &subject_w, &body_w);
        }
        flush_mail(play);
        return sent();
    }
    // a mail that carries something: only where the server offers it, and each limit the compose window keeps
    if names.len() != 1 {
        return refuse(
            "invalid_argument",
            "a mail that carries tunar or an item goes to one name (the client cannot tell whether the server gives it to each)",
        );
    }
    let offer = match play.mail.grams.attach.offer {
        Some(o) if play.mail.grams.live => o,
        _ => return refuse("unsupported", "this server's mail carries nothing"),
    };
    if i64::from(*tunar) > play.dump.player.p34 {
        return refuse("out_of_range", "you do not carry that much tunar");
    }
    let picked = match item {
        None => None,
        Some(MailItem { slot, id, count }) => {
            if offer.items == 0 {
                return refuse("unsupported", "this server's mail carries tunar only");
            }
            let rows = pack_rows(play);
            let Some(it) = rows.get(*slot) else {
                return refuse("stale_handle", "that row is not in the pack now");
            };
            if it.item.id != *id {
                return refuse("stale_handle", "that row holds a different item now");
            }
            if *count > it.quantity.max(1) {
                return refuse("out_of_range", format!("the stack holds {}, not {count}", it.quantity.max(1)));
            }
            Some((it.inv_index, it.item.id, *count, it.item.name.clone()))
        }
    };
    let text = confirm::mail_send_text(
        &String::from_utf8_lossy(&names[0]),
        subject,
        *tunar,
        picked.as_ref().map(|(_, _, c, n)| (n.as_str(), *c)),
        offer.postage,
    );
    let act = Act::MailSend { to: to.clone(), subject: subject.clone(), body: body.clone(), tunar: *tunar, item: picked };
    super::a1_play::ask(act, text, "mail_compose")
}

// ---- auction actions ----

fn auction_ready(play: &Play) -> Result<(), DispatchOutcome> {
    if !caller_holds(AUCTION_KIND) {
        return Err(refuse(
            "not_ready",
            "open the auction window first (eqoa.windows.open_session 'auction_search'): the game's search pages are read only while one is open",
        ));
    }
    if game_auction_up(play) {
        return Err(refuse("busy", "the game's own auction or bank window is up"));
    }
    if !offered(play) {
        return Err(refuse("not_ready", "the bank does not offer the auction on this server"));
    }
    request_free(play)
}

/// One request at a time, as the game has it: the game's own state and the page this window waits for.
fn request_free(play: &Play) -> Result<(), DispatchOutcome> {
    let busy = LOCAL.with(|l| l.borrow().awaiting.is_some()) || play.bank.state != 0;
    if busy {
        return Err(refuse("busy", "the last bank or auction request has not been answered yet"));
    }
    Ok(())
}

/// A page request made: the game's state, and the wait.
fn begin_page(play: &mut Play, mode: &'static str, req: Vec<u8>) -> DispatchOutcome {
    send(play, req);
    play.bank.state = 5;
    LOCAL.with(|l| {
        let mut l = l.borrow_mut();
        l.mode = mode;
        l.awaiting = Some(Instant::now());
    });
    sent()
}

fn begin_out(play: &mut Play, state: i32, req: Vec<u8>, lot: Option<i64>, key: Option<u32>) -> DispatchOutcome {
    send(play, req);
    play.bank.state = state;
    LOCAL.with(|l| {
        let mut l = l.borrow_mut();
        l.out = Some((state, Instant::now()));
        l.out_target = (lot, key);
    });
    sent()
}

/// The lot a mod names, in the page shown now.
fn lot_in_results(play: &Play, lot: i64) -> Option<&net::dump::AuctionItem> {
    play.bank.auction.results.iter().find(|l| l.lot_id == lot)
}

fn query_of(a: &SearchArgs) -> Query {
    let mut q = Query { kind: 0, ..Default::default() };
    let max_price = a.max_price.unwrap_or(0x7FFF_FFFF);
    if max_price < 999_999_999 {
        q.kind = 16;
    }
    let wear = a.wear.unwrap_or(-2);
    let attack = a.attack.unwrap_or(-1);
    if wear >= -1 {
        q.kind |= 2;
    }
    if attack >= 0 {
        q.kind |= 1;
    }
    if !a.name.is_empty() {
        q.kind |= 12;
    }
    q.text = a.name.clone();
    q.name = a.seller.clone();
    q.pa0 = a.race.map_or(0xFFFF_FFFF, |r| 1u32 << r);
    q.p9c = a.class.map_or(0xFFFF_FFFF, |c| 1u32 << c);
    q.pbc = true;
    q.pa4 = a.min_level.unwrap_or(1);
    q.pa8 = a.max_level.unwrap_or(crate::s2::get().auction_max_level);
    q.pac = a.min_price.unwrap_or(0);
    q.pb0 = max_price;
    q.pb4 = wear;
    q.pb8 = attack;
    q
}

fn auction_action(play: &mut Play, action: &A10Action) -> DispatchOutcome {
    if let Err(e) = auction_ready(play) {
        return e;
    }
    match action {
        A10Action::AuctionSearch(args) => {
            let req = play.bank.auction.search(query_of(args));
            begin_page(play, "search", req)
        }
        A10Action::AuctionPage { forward } => {
            if LOCAL.with(|l| l.borrow().mode) != "search" {
                return refuse("not_ready", "pages belong to a search, not to your lots or bids");
            }
            if play.bank.auction.results.is_empty() {
                return refuse("not_ready", "there is no page shown to turn from: search first");
            }
            let req = if *forward { play.bank.auction.next_page() } else { play.bank.auction.prev_page() };
            begin_page(play, "search", req)
        }
        A10Action::AuctionMyLots => {
            let ids: Vec<i64> = play.dump.bank.lots.iter().map(|l| l.lot_id).collect();
            let req = play.bank.auction.request_ids(&ids);
            begin_page(play, "lots", req)
        }
        A10Action::AuctionMyBids => {
            let ids: Vec<i64> = play.dump.bank.bids.iter().map(|b| b.auction_id).collect();
            let req = play.bank.auction.request_ids(&ids);
            begin_page(play, "bids", req)
        }
        A10Action::AuctionBidders { lot } => {
            if lot_in_results(play, *lot).is_none() {
                return refuse("stale_handle", "that lot is not in the page shown now");
            }
            begin_out(play, 6, Auction::bidders(*lot as i32), Some(*lot), None)
        }
        A10Action::AuctionBid { lot, price, count } => ask_bid(play, *lot, *price, *count, false),
        A10Action::AuctionBuyout { lot } => {
            let Some(l) = lot_in_results(play, *lot) else {
                return refuse("stale_handle", "that lot is not in the page shown now");
            };
            let (price, count) = (l.p2f8, if live() { l.quantity } else { 1 });
            if price <= 0 {
                return refuse("not_ready", "that lot has no buyout price");
            }
            ask_bid(play, *lot, price as i32, count as i32, true)
        }
        _ => refuse("unsupported", "not an auction search action"),
    }
}

/// The checks the game's own bid form makes before it asks (`auctionwnd.rs`, state 2 and 4): the lot is still
/// running and is not the player's own.
fn bid_blocked(play: &Play, lot: i64, price: i32, count: i32, buyout: bool) -> Option<DispatchOutcome> {
    let Some(at) = play.bank.auction.results.iter().position(|l| l.lot_id == lot) else {
        return Some(refuse("stale_handle", "that lot is not in the page shown now"));
    };
    let l = &play.bank.auction.results[at];
    if LOCAL.with(|x| x.borrow().uncertain.as_ref().is_some_and(|u| u.kind == "bid" && u.lot == Some(lot))) {
        return Some(refuse("busy", "the last bid on this lot got no answer and may have gone through: read the page again (auction_search, auction_page or auction_my_bids) before bidding on it again"));
    }
    let a = &play.bank.auction;
    let now = play.session.now() as i64;
    if minutes_left(a, l.p2fc, now).is_some_and(|m| m < 0) {
        return Some(refuse("not_ready", "that lot has ended"));
    }
    if play.dump.bank.lots.iter().any(|x| x.lot_id == lot) {
        return Some(refuse("not_ready", "that is your own lot"));
    }
    if count > 1 && !crate::live::features().has(crate::live::Features::STACKS) {
        return Some(refuse("invalid_argument", "this server's auction takes whole lots one at a time: count must be 1"));
    }
    if i64::from(count) > l.quantity.max(1) {
        return Some(refuse("out_of_range", format!("the lot holds {}, not {count}", l.quantity.max(1))));
    }
    // the game's own bid form's limits (`auctionwnd.rs` `draw_bid`, `draw_bid_live`): a price from the floor to the
    // buyout, a count not below the one already bid, and the buyout price only for the whole stack
    let col = |k: usize| a.cols[k].get(at).copied().unwrap_or(0);
    let live = crate::live::features().live();
    if live && let Some((_, _, had)) = Auction::find_bid3(&play.dump.bank, lot) && i64::from(count) < had {
        return Some(refuse("out_of_range", format!("you already bid for {had}: a bid cannot be for fewer")));
    }
    if !buyout {
        let floor = if live { l.p2f4.max(col(2)) } else { l.p2f4.max(col(1) + 1) };
        if i64::from(price) < floor {
            return Some(refuse("out_of_range", format!("the lowest bid on this lot is {floor}")));
        }
        if l.p2f8 > 0 && i64::from(price) > l.p2f8 {
            return Some(refuse("out_of_range", format!("a bid cannot be above the lot's buyout, {}: use auction_buyout", l.p2f8)));
        }
        if live && l.p2f8 > 0 && i64::from(price) == l.p2f8 && i64::from(count) != l.quantity {
            return Some(refuse("out_of_range", format!("a bid at the buyout price is for the whole stack, {}", l.quantity)));
        }
    }
    None
}

fn ask_bid(play: &mut Play, lot: i64, price: i32, count: i32, buyout: bool) -> DispatchOutcome {
    if let Some(e) = bid_blocked(play, lot, price, count, buyout) {
        return e;
    }
    let name = lot_in_results(play, lot).map(|l| l.item.name.clone()).unwrap_or_default();
    let lot_buyout = lot_in_results(play, lot).map_or(0, |l| l.p2f8);
    let text = confirm::auction_bid_text(&name, price, count, buyout, lot_buyout);
    super::a1_play::ask(Act::AuctionBid { lot, price, count, buyout, name }, text, AUCTION_KIND)
}

/// The game's own sell form's limits (`auction.rs` `SellWnd::draw_sell`): a starting price from the item's value up
/// to 999,999,999, a buyout from the price up to the same, and 12 to 72 hours (the GM channel's shorter times
/// are not a mod's). Refused before any question.
fn sell_blocked(it: &net::dump::NetItem, price: i32, buyout: i32, minutes: i32) -> Option<DispatchOutcome> {
    const MAX: i32 = 999_999_999;
    if i64::from(price) < it.item.value as i64 {
        return Some(refuse("out_of_range", format!("the lowest starting price for this item is {}", it.item.value)));
    }
    if price > MAX || buyout > MAX {
        return Some(refuse("out_of_range", "a price is at most 999999999"));
    }
    if buyout < price {
        return Some(refuse("out_of_range", "the buyout cannot be below the starting price"));
    }
    if !(720..=4320).contains(&minutes) {
        return Some(refuse("out_of_range", "an auction runs 12 to 72 hours"));
    }
    if LOCAL.with(|l| l.borrow().uncertain.as_ref().is_some_and(|u| u.kind == "sell" && u.key == Some(it.inv_index))) {
        return Some(refuse("busy", "the last sale of this item got no answer and may have gone through: read your lots (auction_my_lots) before putting it up again"));
    }
    None
}

fn auction_sell(play: &mut Play, action: &A10Action) -> DispatchOutcome {
    let A10Action::AuctionSell { slot, id, count, price, buyout, hours } = action else {
        return refuse("unsupported", "not a sale");
    };
    if !caller_holds("bank") {
        return refuse("not_ready", "selling is a page of the bank window: hold the bank first (eqoa.windows.takeover 'bank')");
    }
    if !offered(play) {
        return refuse("not_ready", "the bank does not offer the auction on this server");
    }
    if game_auction_up(play) {
        return refuse("busy", "the game's own auction or bank window is up");
    }
    if matches!(window_takeover::hidden_bank_session(), None | Some(u32::MAX)) {
        return refuse("not_ready", "the banker is not known: the bank must be open for the sale to name it");
    }
    if let Err(e) = request_free(play) {
        return e;
    }
    if super::PENDING_BANK_TX.with(|b| b.borrow().is_some()) {
        return refuse("busy", "a bank transaction is already pending");
    }
    let Some(it) = play.dump.bank.items.items.get(*slot) else {
        return refuse("stale_handle", "that row is not in the bank now");
    };
    if it.item.id != *id {
        return refuse("stale_handle", "that row holds a different item now");
    }
    if !crate::auction::can_sell(it) {
        return refuse("not_ready", "that item cannot be put up at the auction (no trade, no rent or a stack this server will not take)");
    }
    let stacks = live() && crate::live::features().has(crate::live::Features::STACKS);
    if !stacks && *count != 1 {
        return refuse("invalid_argument", "this server's auction takes one item at a time: count must be 1");
    }
    if i64::from(*count) > it.quantity.max(1) {
        return refuse("out_of_range", format!("the stack holds {}, not {count}", it.quantity.max(1)));
    }
    let minutes = hours.saturating_mul(60);
    if let Some(e) = sell_blocked(it, *price, *buyout, minutes) {
        return e;
    }
    let text = confirm::auction_sell_text(&it.item.name, *count, *price, *buyout, minutes);
    let act = Act::AuctionSell {
        key: it.inv_index,
        id: it.item.id,
        name: it.item.name.clone(),
        count: *count,
        price: *price,
        buyout: *buyout,
        minutes,
    };
    super::a1_play::ask(act, text, "bank")
}

/// What a Yes does, after checking again that the mail, lot or item is what the question named.
pub(crate) fn carry_out(play: &mut Play, act: &Act) -> Result<(), String> {
    match act {
        Act::MailDelete { id, subject } => {
            if game_mail_up(play) || play.mail.grams.state > 0 {
                return Err("the mail window is busy".into());
            }
            match listed(play, *id) {
                Some(h) if utf16(&h.subject) == *subject => {}
                _ => return Err("that mail is no longer in the inbox".into()),
            }
            play.mail.grams.delete_message(*id);
            flush_mail(play);
            Ok(())
        }
        Act::MailSend { to, subject, body, tunar, item } => {
            if game_mail_up(play) || play.mail.grams.state > 0 {
                return Err("the mail window is busy".into());
            }
            let g = &play.mail.grams;
            if !g.live || g.attach.offer.is_none() {
                return Err("the server no longer offers mail that carries anything".into());
            }
            if i64::from(*tunar) > play.dump.player.p34 {
                return Err("you no longer carry that much tunar".into());
            }
            let pick = match item {
                None => None,
                Some((key, id, count, _)) => {
                    let ok = play
                        .dump
                        .inventory
                        .items
                        .iter()
                        .any(|it| it.inv_index == *key && it.item.id == *id && it.quantity.max(1) >= *count && crate::mailattach::mailable(it));
                    if !ok {
                        return Err("the item is no longer in the pack as it was".into());
                    }
                    Some((*key, *count))
                }
            };
            let names = recipients(to, play.mail.grams.live);
            if names.len() != 1 {
                return Err("a mail that carries something goes to one name".into());
            }
            let (s, b) = (crate::ui::wide(subject), crate::ui::wide(body));
            play.mail.grams.send_gram_attached(&names, &s, &b, *tunar, pick);
            flush_mail(play);
            Ok(())
        }
        Act::AuctionBid { lot, price, count, buyout, .. } => {
            if !auction_reading() {
                return Err("the auction window was closed".into());
            }
            if game_auction_up(play) || !offered(play) {
                return Err("the auction is not available".into());
            }
            request_free(play).map_err(|_| "a bank or auction request is still waiting".to_string())?;
            if let Some(DispatchOutcome::Refused { reason, .. }) = bid_blocked(play, *lot, *price, *count, *buyout) {
                return Err(reason);
            }
            let msg = if *buyout || live() { Auction::bid_n(*lot as i32, *price, *count) } else { Auction::bid(*lot as i32, *price) };
            begin_out(play, 4, msg, Some(*lot), None);
            Ok(())
        }
        Act::AuctionSell { key, id, count, price, buyout, minutes, .. } => {
            if game_auction_up(play) || !offered(play) {
                return Err("the auction is not available".into());
            }
            let Some(banker) = window_takeover::hidden_bank_session().filter(|b| *b != u32::MAX) else {
                return Err("the bank is no longer open".into());
            };
            request_free(play).map_err(|_| "a bank or auction request is still waiting".to_string())?;
            if play.bank.state != 0 {
                return Err("a bank request is still waiting".into());
            }
            let Some(it) = play.dump.bank.items.items.iter().find(|it| it.inv_index == *key && it.item.id == *id && it.quantity.max(1) >= i64::from(*count) && crate::auction::can_sell(it)) else {
                return Err("the item is no longer in the bank as it was".into());
            };
            if let Some(DispatchOutcome::Refused { reason, .. }) = sell_blocked(it, *price, *buyout, *minutes) {
                return Err(reason);
            }
            begin_out(play, 3, Auction::sell(banker, *key, *count, *price, *buyout, *minutes), None, Some(*key));
            Ok(())
        }
        _ => Err("not a mail or auction confirmation".into()),
    }
}

#[cfg(test)]
#[path = "a10_play_tests.rs"]
mod tests;
