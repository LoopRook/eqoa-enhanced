// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A10 from Play-shaped state: mail (inbox, read, delete, send, take) and the auction (search, pages, bid,
//! buyout, sell) for a mod's window, the request bytes against the game's own builders (and, for a search, against
//! the game's own search window pressed on the same state), the one pending request, the questions that come first,
//! and the game's own windows unchanged with no mod holding the kind.
//!
//! Each test says what its negative control is: the behaviour without the change would fail the assertion named.
use super::*;
use crate::fork_ext::addons::{
    action_broker, confirm,
    host::Host,
    native_core::{self, CORE, Core},
    native_events, window_sessions as ws,
    window_takeover::{self, TakeoverKind, TakeoverScope},
};
use crate::gram::{Date, Grams, Header};
use crate::mailattach::Offer;
use net::stream::{Reader, Writer};
use winit::keyboard::{Key, NamedKey};

fn ready() -> Play {
    CORE.with(|c| *c.borrow_mut() = Core::default());
    native_core::fixture_demand(true);
    action_broker::reset_all_for_test();
    ws::reset_for_test();
    window_takeover::reset_all_for_test();
    confirm::reset_for_test();
    super::dropped();
    SENT.with(|s| s.borrow_mut().clear());
    let mut p = super::super::tests::play();
    let _ = p.loading.wait.take();
    p.hud = Some((crate::ui::Vui::new(Default::default(), Default::default(), vec![]), Default::default(), crate::hud::Hud::new()));
    p.session.conn = Some(usize::MAX);
    CORE.with(|c| c.borrow_mut().view.in_world = true);
    p
}

fn item(id: i64, name: &str, key: u32, quantity: i64) -> net::dump::NetItem {
    let mut w = Writer::default();
    w.signed(quantity).signed(0).signed(0).signed(-1).signed(-1).u32(key);
    for v in [id, 10, 0, 4001, 0, -1, 0, 0, 1, 0, 0, 0, 0, 1, 20, 0, 0, -1, -1, 0, 0, 0, 0] {
        w.signed(v);
    }
    w.wide(name).wide("a thing").signed(0);
    let mut it = net::dump::read_net_item(&mut Reader::new(&w.data));
    it.item.rent = 1;
    it.item.no_trade = 0;
    it.item.lore = 0;
    // one item, not a stack: retail's auction takes no stack (`auction::can_sell`)
    it.item.max_stack = 1;
    it
}

fn rig() -> (Play, Host) {
    let mut p = ready();
    p.dump.bank.p78cc = true;
    p.dump.player.p34 = 1000;
    p.dump.inventory = net::dump::ItemList { p04: 0, items: vec![item(900_001, "Rusty Sword", 11, 1), item(900_002, "Iron Ore", 12, 5)] };
    let host = Host::with_test_capabilities(&[
        "eqoa.actions.*",
        "eqoa.mail.get",
        "eqoa.auction.get",
        "eqoa.windows.session",
        "eqoa.windows.takeover",
        "eqoa.events.on",
    ])
    .unwrap();
    host.execute(
        r#"
        results = {}
        eqoa.events.on("action.result", function(e) results[#results + 1] = e.data end)
        "#,
    )
    .unwrap();
    (p, host)
}

fn step(p: &mut Play, host: &Host) {
    super::super::begin_iteration(p, true);
    native_events::dispatch(host).unwrap();
}

fn open(p: &mut Play, host: &Host, kind: &str) {
    host.execute(&format!("s_{kind} = assert(eqoa.windows.open_session('{kind}'))")).unwrap();
    step(p, host);
}

/// The mod asks, the game steps, the mod hears. The last result's `ok`, `code` and `reason`.
fn act(p: &mut Play, host: &Host, code: &str) -> (bool, String, String) {
    host.execute(&format!("results = {{}}; last_err = nil; local r, e = (function() {code} end)(); last_err = e")).unwrap();
    step(p, host);
    let (ok, c, r): (Option<bool>, Option<String>, Option<String>) = host
        .lua
        .load("local r = results[#results]; if r == nil then return nil, last_err and last_err.code, last_err and last_err.message end return r.ok, r.code, r.reason")
        .eval()
        .unwrap();
    (ok.unwrap_or(false), c.unwrap_or_default(), r.unwrap_or_default())
}

fn sent() -> Vec<Vec<u8>> {
    SENT.with(|s| s.borrow().clone())
}

fn say_yes() {
    assert!(confirm::key(&Key::Named(NamedKey::ArrowRight)));
    assert!(confirm::key(&Key::Named(NamedKey::Enter)));
}

fn lua_ok(host: &Host, code: &str) {
    host.execute(code).unwrap();
}

// ---- mail ----

fn header(id: u32, from: &str, subject: &str) -> Header {
    Header { x00: 7, id, x08: 9, from: from.as_bytes().to_vec(), subject: crate::ui::wide(subject), date: Date { year: 2026, month: 10, day: 19, hour: 21, minute: 5, second: 3 }, read: 0 }
}

fn inbox_reply(hs: &[Header]) -> Vec<u8> {
    let mut w = Writer::default();
    w.u8(2).u8(2).signed(hs.len() as i64);
    for h in hs {
        h.write(&mut w);
    }
    w.data
}

fn message_reply(id: i64, text: &str) -> Vec<u8> {
    let mut w = Writer::default();
    w.u8(3).u8(3).signed(id).wide(text);
    w.data
}

/// The mod's inbox window open and the inbox answered with two mails.
fn inbox_rig() -> (Play, Host) {
    let (mut p, host) = rig();
    open(&mut p, &host, "mail_inbox");
    p.mail_recv(&inbox_reply(&[header(11, "Ann", "One"), header(12, "Bob", "Two")]));
    step(&mut p, &host);
    (p, host)
}

#[test]
fn a10_the_inbox_is_asked_for_at_open_read_and_one_mail_opened_with_the_games_own_requests() {
    let (mut p, host) = rig();
    // control: no mod window, no request, and the gram client is as the game left it
    step(&mut p, &host);
    assert!(sent().is_empty() && p.mail.grams.state == 0);
    open(&mut p, &host, "mail_inbox");
    let mut native = Grams::default();
    native.request_inbox();
    assert_eq!(sent(), vec![native.out[0].clone()], "the inbox request is the game's own, once");
    assert_eq!(p.mail.grams.state, 1);
    p.mail_recv(&inbox_reply(&[header(11, "Ann", "One"), header(12, "Bob", "Two")]));
    step(&mut p, &host);
    lua_ok(
        &host,
        r#"local m = assert(eqoa.mail.get())
        assert(m.status == "idle" and m.count == 2 and #m.entries == 2 and m.truncated == false, m.status)
        assert(m.entries[1].from == "Ann" and m.entries[2].subject == "Two" and m.entries[1].read == false)
        assert(m.message == nil)"#,
    );
    // a read is the game's own request for that row
    let (ok, _, _) = act(&mut p, &host, "return eqoa.actions.mail_read(12)");
    assert!(ok);
    let mut g = Grams::default();
    g.headers = p.mail.grams.headers.clone();
    g.count = 2;
    g.request_message(12);
    assert_eq!(sent().last().unwrap(), &g.out[0], "the same bytes as the game's own Select");
    assert_eq!(p.mail.grams.state, 2);
    p.mail_recv(&message_reply(12, "hello there"));
    step(&mut p, &host);
    lua_ok(
        &host,
        r#"local m = assert(eqoa.mail.get())
        assert(m.message and m.message.text == "hello there" and m.message.from == "Bob" and m.message.id == 12)
        assert(m.entries[2].read == true and m.entries[1].read == false)"#,
    );
    // negative control: a mail that is not in the inbox is refused and nothing is sent
    let before = sent().len();
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.mail_read(99)");
    assert!(!ok && code == "stale_handle", "{code}");
    assert_eq!(sent().len(), before);
}

#[test]
fn a10_one_mail_request_at_a_time_and_a_lost_connection_never_replays_a_send() {
    let (mut p, host) = rig();
    open(&mut p, &host, "mail_inbox");
    open(&mut p, &host, "mail_compose");
    assert_eq!(p.mail.grams.state, 1, "the inbox is still waited for");
    // the inbox is awaited: a refresh or a read is refused busy, nothing is sent
    let n = sent().len();
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.mail_refresh()");
    assert!(!ok && code == "busy", "{code}");
    p.mail_recv(&inbox_reply(&[header(11, "Ann", "One")]));
    // plain mail goes out with no question, as the game's compose sends it (the disc's one name)
    let (ok, _, _) = act(&mut p, &host, r#"return eqoa.actions.mail_send({to = "bob", subject = "Hi", body = "yo"})"#);
    assert!(ok && !confirm::is_open());
    let mut g = Grams::default();
    g.send_gram(b"Bob", &crate::ui::wide("Hi"), &crate::ui::wide("yo"));
    assert_eq!(sent().len(), n + 1);
    assert_eq!(sent().last().unwrap(), &g.out[0]);
    assert_eq!(p.mail.grams.state, 3);
    // a second send while the first awaits its answer: refused, nothing sent
    let (ok, code, _) = act(&mut p, &host, r#"return eqoa.actions.mail_send({to = "bob", subject = "Hi", body = "again"})"#);
    assert!(!ok && code == "busy", "{code}");
    assert_eq!(sent().len(), n + 1);
    // the connection drops and comes back: the game forgets the send (it is not asked for again) and so does this
    p.mail_connect();
    step(&mut p, &host);
    assert_eq!(p.mail.grams.state, 0);
    assert!(p.mail.grams.out.is_empty());
    assert_eq!(sent().len(), n + 1, "an uncertain send is never replayed");
    lua_ok(&host, r#"assert(eqoa.mail.get().status == "idle")"#);
}

#[test]
fn a10_the_games_checks_come_first_an_empty_name_subject_or_text_is_refused() {
    let (mut p, host) = inbox_rig();
    open(&mut p, &host, "mail_compose");
    let n = sent().len();
    for (code_in, why) in [
        (r#"return eqoa.actions.mail_send({to = "", subject = "S", body = "B"})"#, "to"),
        (r#"return eqoa.actions.mail_send({to = "Bob", subject = "", body = "B"})"#, "subject"),
        (r#"return eqoa.actions.mail_send({to = "Bob", subject = "S", body = "  "})"#, "text"),
    ] {
        let (ok, code, _) = act(&mut p, &host, code_in);
        assert!(!ok && code == "invalid_argument", "{why}: {code}");
    }
    assert_eq!(sent().len(), n);
    // the control: the same call with all three goes
    let (ok, _, _) = act(&mut p, &host, r#"return eqoa.actions.mail_send({to = "Bob", subject = "S", body = "B"})"#);
    assert!(ok);
    assert_eq!(sent().len(), n + 1);
}

#[test]
fn a10_delete_asks_first_and_checks_the_mail_again_at_the_yes() {
    let (mut p, host) = inbox_rig();
    let n = sent().len();
    let (ok, _, _) = act(&mut p, &host, "return eqoa.actions.mail_delete(12)");
    assert!(ok);
    // negative control: the call alone deletes nothing
    assert_eq!(sent().len(), n);
    assert_eq!(p.mail.grams.count, 2);
    let q = confirm::current_for_test().expect("the question is up");
    assert!(!q.yes && q.text.contains("Two") && q.text.contains("Bob"), "{}", q.text);
    say_yes();
    step(&mut p, &host);
    let mut g = Grams::default();
    g.request_inbox();
    g.recv(&inbox_reply(&[header(11, "Ann", "One"), header(12, "Bob", "Two")]));
    g.out.clear();
    g.delete_message(12);
    assert_eq!(sent().len(), n + 1);
    assert_eq!(sent().last().unwrap(), &g.out[0], "the game's own delete");
    // the same question, but the mail went before the Yes (a new inbox without it): nothing is sent
    p.mail.grams.state = 0;
    p.mail.grams.headers = vec![header(11, "Ann", "One")];
    p.mail.grams.count = 1;
    let m = sent().len();
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.mail_delete(12)");
    assert!(!ok && code == "stale_handle", "{code}");
    // a question for a mail whose subject changes under it: refused at the Yes
    p.mail.grams.headers = vec![header(11, "Ann", "One")];
    let (ok, _, _) = act(&mut p, &host, "return eqoa.actions.mail_delete(11)");
    assert!(ok);
    p.mail.grams.headers = vec![header(11, "Ann", "Something else")];
    say_yes();
    step(&mut p, &host);
    assert_eq!(sent().len(), m, "the mail is no longer the one asked about: not deleted");
    lua_ok(&host, "assert(eqoa.mail.get().entries[1].subject == 'Something else')");
}

#[test]
fn a10_a_mail_that_carries_tunar_or_an_item_asks_first_and_is_sent_once_on_the_yes() {
    let (mut p, host) = inbox_rig();
    open(&mut p, &host, "mail_compose");
    p.mail.grams.live = true;
    let call = r#"return eqoa.actions.mail_send({to = "Bob", subject = "S", body = "B", tunar = 100, item = {slot = 2, id = 900002, count = 2}})"#;
    let n = sent().len();
    // the server offers no attachments: refused, nothing asked
    let (ok, code, _) = act(&mut p, &host, call);
    assert!(!ok && code == "unsupported", "{code}");
    assert!(!confirm::is_open());
    p.mail.grams.attach.offer = Some(Offer { items: 1, postage: 15 });
    // more tunar than carried: refused
    let (ok, code, _) = act(&mut p, &host, r#"return eqoa.actions.mail_send({to = "Bob", subject = "S", body = "B", tunar = 5000})"#);
    assert!(!ok && code == "out_of_range", "{code}");
    // a stack bigger than the pack's, and a row that holds another item: refused
    let (ok, code, _) = act(&mut p, &host, r#"return eqoa.actions.mail_send({to = "Bob", subject = "S", body = "B", item = {slot = 2, id = 900002, count = 9}})"#);
    assert!(!ok && code == "out_of_range", "{code}");
    let (ok, code, _) = act(&mut p, &host, r#"return eqoa.actions.mail_send({to = "Bob", subject = "S", body = "B", item = {slot = 2, id = 900001, count = 1}})"#);
    assert!(!ok && code == "stale_handle", "{code}");
    assert_eq!(sent().len(), n);
    // the good one asks, and sends nothing (the control: a plain mail goes at once, the one above must not)
    let (ok, _, _) = act(&mut p, &host, call);
    assert!(ok);
    assert_eq!(sent().len(), n, "the question comes first");
    let q = confirm::current_for_test().expect("the question is up");
    assert!(!q.yes && q.text.contains("100 tunar") && q.text.contains("Iron Ore") && q.text.contains("postage is 15"), "{}", q.text);
    say_yes();
    step(&mut p, &host);
    assert_eq!(sent().len(), n + 1, "sent once");
    let want = crate::mailattach::send(&[b"Bob".to_vec()], &crate::ui::wide("S"), &crate::ui::wide("B"), 100, Some((12, 2)));
    assert_eq!(sent().last().unwrap(), &want, "the bytes the game's compose window sends for the same mail");
    assert_eq!(p.mail.grams.state, 3);
    step(&mut p, &host);
    assert_eq!(sent().len(), n + 1, "a later step sends nothing more");
    // the mod's own Yes does not exist
    lua_ok(&host, "assert(eqoa.actions.confirm_yes == nil and eqoa.windows.answer == nil)");
}

#[test]
fn a10_the_pack_row_must_still_hold_the_item_at_the_yes_and_a_take_needs_a_read_first() {
    let (mut p, host) = inbox_rig();
    open(&mut p, &host, "mail_compose");
    p.mail.grams.live = true;
    p.mail.grams.attach.offer = Some(Offer { items: 1, postage: 0 });
    let (ok, _, _) = act(&mut p, &host, r#"return eqoa.actions.mail_send({to = "Bob", subject = "S", body = "B", item = {slot = 2, id = 900002, count = 2}})"#);
    assert!(ok && confirm::is_open());
    let n = sent().len();
    // the ore is sold before the Yes: nothing goes (negative control: the first send above would have been sent)
    p.dump.inventory.items.remove(1);
    say_yes();
    step(&mut p, &host);
    assert_eq!(sent().len(), n);
    // take: the mail must have been read and said to carry something
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.mail_take(11)");
    assert!(!ok && code == "not_ready", "{code}");
    p.mail.grams.attach.carried = Some(crate::mailattach::Carried { id: 11, tunar: 40, items: vec![], returned: false });
    let (ok, _, _) = act(&mut p, &host, "return eqoa.actions.mail_take(11)");
    assert!(ok);
    assert_eq!(sent().last().unwrap(), &crate::mailattach::take(11));
}

#[test]
fn a10_the_games_own_mail_window_up_means_busy_and_no_mod_window_changes_nothing() {
    let (mut p, host) = rig();
    // the game's inbox is up: the mod's window opens without asking for anything
    p.mode = 24;
    open(&mut p, &host, "mail_inbox");
    assert!(sent().is_empty() && p.mail.grams.state == 0, "the game's window owns the gram client");
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.mail_refresh()");
    assert!(!ok && code == "busy", "{code}");
    assert!(sent().is_empty());
    // the game's window gone: the same call goes (the control)
    p.mode = 0;
    let (ok, _, _) = act(&mut p, &host, "return eqoa.actions.mail_refresh()");
    assert!(ok);
    assert_eq!(sent().len(), 1);
    // with no window of the mod's own, the action is not ready
    host.execute("assert(eqoa.windows.close_session(s_mail_inbox.ref))").unwrap();
    step(&mut p, &host);
    lua_ok(&host, "assert(eqoa.mail.get() == nil)");
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.mail_refresh()");
    assert!(!ok && code == "not_ready", "{code}");
}

// ---- the auction ----

/// A lot on the wire, as the server writes one; it ends `ends` minutes on the server's clock.
fn lot_bytes(w: &mut Writer, id: i64, qty: i64, buyout: i64, ends: i64) {
    for v in [id, qty, 0, 0, 10, buyout, ends] {
        w.signed(v);
    }
    w.string(b"Seller");
    let zeros = vec![0u8; 4096];
    let mut r = Reader::new(&zeros);
    let _ = net::dump::read_item(&mut r);
    w.bytes(&vec![0u8; r.pos]);
}

/// A results page (0x1206, sub-op 129): the lots (id, quantity, buyout), the server's minutes and the three columns.
fn page_reply(page: i64, lots: &[(i64, i64, i64)]) -> Vec<u8> {
    let mut w = Writer::default();
    w.u8(129).signed(page).signed(30).signed(lots.len() as i64);
    for (id, qty, buyout) in lots {
        lot_bytes(&mut w, *id, *qty, *buyout, 500);
    }
    for col in [7i64, -1, 3] {
        w.signed(lots.len() as i64);
        for _ in lots {
            w.signed(col);
        }
    }
    w.data
}

fn auction_rig() -> (Play, Host) {
    let (mut p, host) = rig();
    open(&mut p, &host, "auction_search");
    (p, host)
}

/// What the game's own search window sends for the form's defaults, pressed on the same state.
fn native_search_bytes(a: &mut Auction, bank: &net::dump::Bank) -> Vec<u8> {
    use crate::auctionwnd::{SearchOut, SearchWnd, View};
    use crate::pointer::testing::{NullRaster, vui};
    let (vui, mut r) = (vui(), NullRaster);
    let mut w = SearchWnd::default();
    let _ = w.open(&vui, &r, 0xA801_0000, a, bank, 0);
    let icon = |_: &net::dump::NetItem| 0;
    let can_use = |_: &net::dump::NetItem| true;
    let v = View { bank, idle: true, searching: false, player: 1, now: 0, icon: &icon, can_use: &can_use, attack_words: &[] };
    let (mut cues, mut out) = (Vec::new(), Vec::new());
    // the form starts on its first row: Up goes round to Search, Enter presses it
    for k in [19, 13] {
        w.key(&vui, &mut r, a, &v, k, true, &mut cues, &mut out);
    }
    match out.pop() {
        Some(SearchOut::Send(bytes, 5)) => bytes,
        other => panic!("the form did not search: {other:?}"),
    }
}

#[test]
fn a10_a_search_is_the_game_forms_own_bytes_and_one_page_is_awaited_at_a_time() {
    let (mut p, host) = rig();
    // the game's own press, on this state (the page counter the mod's opening and the form's opening both raise)
    let mut native = p.bank.auction.clone();
    let want = native_search_bytes(&mut native, &p.dump.bank);
    open(&mut p, &host, "auction_search");
    let (ok, _, _) = act(&mut p, &host, "return eqoa.actions.auction_search()");
    assert!(ok, "searched");
    assert_eq!(sent(), vec![want], "the same bytes as the game's own Search press");
    assert_eq!(p.bank.state, 5);
    lua_ok(&host, r#"local a = assert(eqoa.auction.get()); assert(a.pending == "search" and a.offered == true and #a.results == 0, a.pending)"#);
    // one at a time: a second search and a page turn are refused while the page is awaited
    for call in ["return eqoa.actions.auction_search({name = 'Iron'})", "return eqoa.actions.auction_page(true)", "return eqoa.actions.auction_my_lots()"] {
        let (ok, code, _) = act(&mut p, &host, call);
        assert!(!ok && code == "busy", "{call}: {code}");
    }
    assert_eq!(sent().len(), 1);
    // a late page of an earlier request arrives first: the game clears its state for it, the mod still waits
    let page = p.bank.auction.page as i64;
    p.bank_opcode(0x1206, &page_reply(page - 1, &[(900, 1, 0)]));
    assert!(p.bank.auction.results.is_empty(), "the page counter drops it");
    step(&mut p, &host);
    lua_ok(&host, r#"assert(eqoa.auction.get().pending == "search", "still waiting for the right page")"#);
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.auction_search({name = 'Iron'})");
    assert!(!ok && code == "busy", "{code}: the late page did not free the window");
    // the right page: read, and the wait ends
    p.bank_opcode(0x1206, &page_reply(page, &[(501, 1, 400), (502, 3, 0)]));
    step(&mut p, &host);
    lua_ok(
        &host,
        r#"local a = assert(eqoa.auction.get())
        assert(a.pending == "none" and #a.results == 2, a.pending)
        assert(a.results[1].lot == 501 and a.results[1].buyout == 400 and a.results[1].quantity == 1 and a.results[1].seller == "Seller")
        assert(a.results[1].minutes_left == 470 and a.results[2].quantity == 3)
        assert(a.results[1].mine == false and a.results[1].leader == 7 and a.results[1].bids == 3 and a.results[1].high_bid == nil)"#,
    );
}

#[test]
fn a10_without_the_one_line_the_hidden_search_misses_its_pages() {
    let (mut p, host) = rig();
    // the game's own reading, as `bankplay.rs` has it with the mode at 0 and no mod window: the page is dropped
    p.bank.auction.page = 4;
    p.bank_opcode(0x1206, &page_reply(4, &[(501, 1, 0)]));
    assert!(p.bank.auction.results.is_empty(), "negative control: the game drops a page while its own window (mode 39) is not up");
    // the game's own window up: kept (unchanged behaviour with no mod)
    p.mode = 39;
    p.bank.auction.page = 4;
    p.bank_opcode(0x1206, &page_reply(4, &[(501, 1, 0)]));
    assert_eq!(p.bank.auction.results.len(), 1, "mode 39 keeps its page as it always did");
    p.mode = 0;
    p.bank.auction.clear_results();
    // a mod's own window: kept without the game's window
    open(&mut p, &host, "auction_search");
    p.bank.auction.page = 6;
    p.bank_opcode(0x1206, &page_reply(6, &[(501, 1, 0), (502, 1, 0)]));
    assert_eq!(p.bank.auction.results.len(), 2, "the mod's session receives its replies");
    // the window closed: the game's own rule again, and the state is emptied as the game's close does
    host.execute("assert(eqoa.windows.close_session(s_auction_search.ref))").unwrap();
    step(&mut p, &host);
    assert!(p.bank.auction.results.is_empty());
    p.bank.auction.page = 8;
    p.bank_opcode(0x1206, &page_reply(8, &[(501, 1, 0)]));
    assert!(p.bank.auction.results.is_empty(), "no mod window, no hook");
    lua_ok(&host, "assert(eqoa.auction.get() == nil)");
    // the wiring: the hook is the one changed line in bankplay.rs, and upstream's own test is still in it
    let src = include_str!("../bankplay.rs").replace("\r\n", "\n");
    assert_eq!(src.matches("fork_hooks::auction_reading()").count(), 1);
    assert!(src.contains("self.mode == 39 || super::fork_hooks::auction_reading()"));
}

#[test]
fn a10_pages_turn_with_the_games_own_requests_and_only_in_a_search() {
    let (mut p, host) = auction_rig();
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.auction_page(true)");
    assert!(!ok && code == "not_ready", "{code}: nothing to turn from");
    let (ok, _, _) = act(&mut p, &host, "return eqoa.actions.auction_search({name = 'Iron', min_level = 5, max_level = 40, class = 3})");
    assert!(ok);
    // the form's fields go into the query the way `select` builds it
    let mut a = Auction::new();
    a.page = p.bank.auction.page - 1;
    let q = Query {
        kind: 12,
        text: "Iron".into(),
        name: String::new(),
        p9c: 1 << 3,
        pa0: 0xFFFF_FFFF,
        pa4: 5,
        pa8: 40,
        pac: 0,
        pb0: 0x7FFF_FFFF,
        pb4: -2,
        pb8: -1,
        pbc: true,
    };
    assert_eq!(sent().last().unwrap(), &a.search(q), "the form's own fields");
    let page = p.bank.auction.page as i64;
    p.bank_opcode(0x1206, &page_reply(page, &[(501, 1, 0), (502, 1, 0)]));
    step(&mut p, &host);
    // next page: the game's own request from the last lot shown
    let mut want = p.bank.auction.clone();
    let (ok, _, _) = act(&mut p, &host, "return eqoa.actions.auction_page(true)");
    assert!(ok);
    assert_eq!(sent().last().unwrap(), &want.next_page());
    let page = p.bank.auction.page as i64;
    p.bank_opcode(0x1206, &page_reply(page, &[(503, 1, 0)]));
    step(&mut p, &host);
    let mut want = p.bank.auction.clone();
    let (ok, _, _) = act(&mut p, &host, "return eqoa.actions.auction_page(false)");
    assert!(ok);
    assert_eq!(sent().last().unwrap(), &want.prev_page());
    // a bad page argument and a wrong field are refused before anything is sent
    let n = sent().len();
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.auction_page('next')");
    assert!(!ok && code == "invalid_argument", "{code}");
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.auction_search({min_level = 'a lot'})");
    assert!(!ok && code == "invalid_argument", "{code}");
    assert_eq!(sent().len(), n);
}

fn lot_page(p: &mut Play, host: &Host, lots: &[(i64, i64, i64)]) {
    let (ok, _, _) = act(p, host, "return eqoa.actions.auction_search()");
    assert!(ok);
    let page = p.bank.auction.page as i64;
    p.bank_opcode(0x1206, &page_reply(page, lots));
    step(p, host);
}

#[test]
fn a10_a_bid_asks_first_checks_the_lot_again_and_sends_the_games_bid_once() {
    let (mut p, host) = auction_rig();
    lot_page(&mut p, &host, &[(501, 1, 400), (502, 1, 0)]);
    let n = sent().len();
    let (ok, _, _) = act(&mut p, &host, "return eqoa.actions.auction_bid({lot = 501, price = 150})");
    assert!(ok);
    assert_eq!(sent().len(), n, "negative control: the call alone bids nothing");
    let q = confirm::current_for_test().expect("the question is up");
    assert!(!q.yes && q.text.contains("150") && q.text.contains("Bid"), "{}", q.text);
    // a second request while the question is up is refused (one question at a time)
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.auction_bid({lot = 502, price = 99})");
    assert!(!ok && code == "busy", "{code}");
    say_yes();
    step(&mut p, &host);
    assert_eq!(sent().len(), n + 1);
    assert_eq!(sent().last().unwrap(), &Auction::bid(501, 150), "the disc's bid, one item");
    assert_eq!(p.bank.state, 4);
    step(&mut p, &host);
    assert_eq!(sent().len(), n + 1, "sent once");
    // answered: free again
    p.bank_opcode(0x1206, &{
        let mut w = Writer::default();
        w.u8(132);
        w.data
    });
    p.bank.state = 0;
    step(&mut p, &host);
    // buyout asks for the lot's own price; a lot with none is refused
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.auction_buyout(502)");
    assert!(!ok && code == "not_ready", "{code}");
    let (ok, _, _) = act(&mut p, &host, "return eqoa.actions.auction_buyout(501)");
    assert!(ok);
    assert!(confirm::current_for_test().unwrap().text.contains("Buy out") && confirm::current_for_test().unwrap().text.contains("400"));
    // the lot's page changes before the Yes: nothing is bought
    let m = sent().len();
    p.bank.auction.results.clear();
    say_yes();
    step(&mut p, &host);
    assert_eq!(sent().len(), m, "the lot is not in the page any more: not bought");
    // a lot that is not shown, one's own lot and an ended lot are refused with no question
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.auction_bid({lot = 999, price = 5})");
    assert!(!ok && code == "stale_handle", "{code}");
}

#[test]
fn a10_a_lot_of_my_own_or_one_that_has_ended_is_not_bid_on() {
    let (mut p, host) = auction_rig();
    lot_page(&mut p, &host, &[(501, 1, 400)]);
    p.dump.bank.lots.push(p.bank.auction.results[0].clone());
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.auction_bid({lot = 501, price = 5})");
    assert!(!ok && code == "not_ready", "{code}");
    assert!(!confirm::is_open());
    p.dump.bank.lots.clear();
    // the server's clock runs past the lot's end
    p.bank.auction.minutes = 900;
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.auction_bid({lot = 501, price = 5})");
    assert!(!ok && code == "not_ready", "{code}");
    assert!(!confirm::is_open());
    // the control: back in time, it asks
    p.bank.auction.minutes = 30;
    let (ok, _, _) = act(&mut p, &host, "return eqoa.actions.auction_bid({lot = 501, price = 5})");
    assert!(ok && confirm::is_open());
    // the disc's auction takes one item at a time
    confirm::reset_for_test();
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.auction_bid({lot = 501, price = 5, count = 2})");
    assert!(!ok && code == "invalid_argument", "{code}");
}

#[test]
fn a10_the_live_auction_bids_for_a_count_and_buys_out_the_whole_stack() {
    crate::live::with_features(Some(0x1B), || {
        let (mut p, host) = auction_rig();
        lot_page(&mut p, &host, &[(501, 20, 400)]);
        let n = sent().len();
        let (ok, _, _) = act(&mut p, &host, "return eqoa.actions.auction_bid({lot = 501, price = 150, count = 5})");
        assert!(ok);
        say_yes();
        step(&mut p, &host);
        assert_eq!(sent().len(), n + 1);
        assert_eq!(sent().last().unwrap(), &Auction::bid_n(501, 150, 5));
        p.bank.state = 0;
        step(&mut p, &host);
        let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.auction_bid({lot = 501, price = 150, count = 25})");
        assert!(!ok && code == "out_of_range", "{code}");
        let (ok, _, _) = act(&mut p, &host, "return eqoa.actions.auction_buyout(501)");
        assert!(ok);
        say_yes();
        step(&mut p, &host);
        assert_eq!(sent().last().unwrap(), &Auction::bid_n(501, 400, 20), "the whole stack at the lot's price");
        lua_ok(&host, "assert(eqoa.auction.get().results[1].next_bid == 3 and eqoa.auction.get().results[1].leader == nil)");
    });
}

fn bank_rig() -> (Play, Host) {
    let (mut p, host) = rig();
    p.dump.bank.items = net::dump::ItemList { p04: 0, items: vec![item(700_001, "Gold Ring", 21, 1), item(700_002, "Soul Shard", 22, 1)] };
    p.dump.bank.items.items[1].item.no_trade = 1;
    let vm = native_events::bus(&host.lua).borrow().vm;
    window_takeover::takeover(vm, "mod", TakeoverKind::Bank, TakeoverScope::Own).unwrap();
    window_takeover::activate_hidden_for_test(vm, TakeoverKind::Bank, 77);
    step(&mut p, &host);
    host.execute("held_bank = assert(eqoa.windows.session('bank'))").unwrap();
    (p, host)
}

#[test]
fn a10_selling_is_a_page_of_the_bank_asks_first_and_names_the_banker() {
    let (mut p, host) = bank_rig();
    let call = "return eqoa.actions.auction_sell({slot = 1, id = 700001, price = 100, buyout = 300, hours = 12})";
    let n = sent().len();
    let (ok, _, _) = act(&mut p, &host, call);
    assert!(ok);
    assert_eq!(sent().len(), n, "negative control: the call alone sells nothing");
    let q = confirm::current_for_test().expect("the question is up");
    assert!(!q.yes && q.text.contains("Gold Ring") && q.text.contains("100") && q.text.contains("300") && q.text.contains("12 hours"), "{}", q.text);
    lua_ok(&host, r#"local a = eqoa.auction.get(); assert(a == nil or #a.sellable >= 0)"#);
    say_yes();
    step(&mut p, &host);
    assert_eq!(sent().len(), n + 1);
    assert_eq!(sent().last().unwrap(), &Auction::sell(77, 21, 1, 100, 300, 720), "the banker's id and the bank record's key, 12 hours in minutes");
    assert_eq!(p.bank.state, 3);
    // one request at a time: another sale waits for the answer
    let (ok, code, _) = act(&mut p, &host, call);
    assert!(!ok && code == "busy", "{code}");
    // an item that cannot be sold (no trade) and a row of another item are refused with no question
    p.bank.state = 0;
    confirm::reset_for_test();
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.auction_sell({slot = 2, id = 700002, price = 1, buyout = 2, hours = 1})");
    assert!(!ok && code == "not_ready", "{code}");
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.auction_sell({slot = 1, id = 700002, price = 1, buyout = 2, hours = 1})");
    assert!(!ok && code == "stale_handle", "{code}");
    // the disc's auction puts one up at a time
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.auction_sell({slot = 1, id = 700001, count = 3, price = 1, buyout = 2, hours = 1})");
    assert!(!ok && code == "invalid_argument", "{code}");
    assert!(!confirm::is_open());
}

#[test]
fn a10_selling_needs_the_mods_own_bank_and_the_bank_offering_the_auction_and_the_item_still_there_at_the_yes() {
    // no bank held: refused, with the words that say what to do
    let (mut p, host) = rig();
    p.dump.bank.items = net::dump::ItemList { p04: 0, items: vec![item(700_001, "Gold Ring", 21, 1)] };
    let (ok, code, reason) = act(&mut p, &host, "return eqoa.actions.auction_sell({slot = 1, id = 700001, price = 1, buyout = 2, hours = 1})");
    assert!(!ok && code == "not_ready" && reason.contains("hold the bank"), "{code} {reason}");
    // the bank offers no auction: refused (the control: it works in the next block)
    let (mut p, host) = bank_rig();
    p.dump.bank.p78cc = false;
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.auction_sell({slot = 1, id = 700001, price = 1, buyout = 2, hours = 1})");
    assert!(!ok && code == "not_ready", "{code}");
    p.dump.bank.p78cc = true;
    let (ok, _, _) = act(&mut p, &host, "return eqoa.actions.auction_sell({slot = 1, id = 700001, price = 1, buyout = 2, hours = 1})");
    assert!(ok);
    // the ring leaves the bank before the Yes
    let n = sent().len();
    p.dump.bank.items.items.clear();
    say_yes();
    step(&mut p, &host);
    assert_eq!(sent().len(), n, "the bank no longer holds it: nothing is sold");
    // the game's own bank window up: busy
    confirm::reset_for_test();
    p.mode = 11;
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.auction_sell({slot = 1, id = 700001, price = 1, buyout = 2, hours = 1})");
    assert!(!ok && code == "busy", "{code}");
}

#[test]
fn a10_actions_are_offered_only_where_the_bank_offers_the_auction() {
    let (mut p, host) = rig();
    p.dump.bank.p78cc = false;
    open(&mut p, &host, "auction_search");
    lua_ok(&host, "assert(eqoa.auction.get().offered == false)");
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.auction_search()");
    assert!(!ok && code == "not_ready", "{code}");
    assert!(sent().is_empty());
    p.dump.bank.p78cc = true;
    let (ok, _, _) = act(&mut p, &host, "return eqoa.actions.auction_search()");
    assert!(ok, "the control: offered, it searches");
}

#[test]
fn a10_a_request_the_server_never_answers_is_given_up_on_and_never_replayed() {
    let (mut p, host) = auction_rig();
    let (ok, _, _) = act(&mut p, &host, "return eqoa.actions.auction_search()");
    assert!(ok);
    let n = sent().len();
    // before the time: still waiting
    step(&mut p, &host);
    lua_ok(&host, r#"assert(eqoa.auction.get().pending == "search" and eqoa.auction.get().timed_out == false)"#);
    LOCAL.with(|l| l.borrow_mut().awaiting = Some(Instant::now() - GIVE_UP - Duration::from_secs(1)));
    step(&mut p, &host);
    lua_ok(&host, r#"local a = eqoa.auction.get(); assert(a.pending == "none" and a.timed_out == true, a.pending)"#);
    assert_eq!(p.bank.state, 0);
    assert_eq!(sent().len(), n, "never replayed");
    // a new request goes (and clears the flag)
    let (ok, _, _) = act(&mut p, &host, "return eqoa.actions.auction_search()");
    assert!(ok);
    lua_ok(&host, "assert(eqoa.auction.get().timed_out == false)");
}

#[test]
fn a10_my_lots_and_bids_and_the_bidders_use_the_games_requests() {
    let (mut p, host) = auction_rig();
    let mut l = net::dump::AuctionItem::default();
    l.lot_id = 31;
    p.dump.bank.lots.push(l);
    p.dump.bank.bids.push(net::dump::Bid { character_id: 1, auction_id: 41, x08: 100, x0c: 2, x10: 200, x14: 0 });
    let mut want = p.bank.auction.clone();
    let (ok, _, _) = act(&mut p, &host, "return eqoa.actions.auction_my_lots()");
    assert!(ok);
    assert_eq!(sent().last().unwrap(), &want.request_ids(&[31]));
    lua_ok(
        &host,
        r#"local a = eqoa.auction.get(); assert(a.mode == "lots" and a.pending == "lots" and #a.lots == 1 and a.lots[1].lot == 31)
        assert(#a.bids == 1 and a.bids[1].lot == 41 and a.bids[1].price == 100 and a.bids[1].count == 2 and a.bids[1].escrow == 200)"#,
    );
    let page = p.bank.auction.page as i64;
    p.bank_opcode(0x1206, &page_reply(page, &[(31, 1, 0)]));
    step(&mut p, &host);
    let (ok, _, _) = act(&mut p, &host, "return eqoa.actions.auction_bidders(31)");
    assert!(ok);
    assert_eq!(sent().last().unwrap(), &Auction::bidders(31));
    assert_eq!(p.bank.state, 6);
    // a page turn does not belong to the lots list
    p.bank.state = 0;
    LOCAL.with(|l| l.borrow_mut().out = None);
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.auction_page(true)");
    assert!(!ok && code == "not_ready", "{code}");
    let mut want = p.bank.auction.clone();
    let (ok, _, _) = act(&mut p, &host, "return eqoa.actions.auction_my_bids()");
    assert!(ok);
    assert_eq!(sent().last().unwrap(), &want.request_ids(&[41]));
}

#[test]
fn a10_the_games_own_auction_window_up_is_busy_and_no_mod_window_leaves_everything_as_it_was() {
    let (mut p, host) = auction_rig();
    p.mode = 39;
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.auction_search()");
    assert!(!ok && code == "busy", "{code}");
    assert!(sent().is_empty());
    p.mode = 0;
    // the capability: a mod without it reads nothing
    let bare = Host::with_test_capabilities(&["eqoa.windows.session"]).unwrap();
    bare.execute("local a, e = eqoa.auction.get(); assert(a == nil and e and e.code == 'not_declared' or e.code == 'permission_denied', e and e.code)").unwrap();
    bare.execute("local a, e = eqoa.mail.get(); assert(a == nil and e ~= nil)").unwrap();
    // closing the window empties what it read, as the game's own close does
    host.execute("assert(eqoa.windows.close_session(s_auction_search.ref))").unwrap();
    step(&mut p, &host);
    assert!(native_a10::current_auction().is_none() && p.bank.auction.results.is_empty());
}

#[test]
fn a10_dead_or_out_of_the_world_nothing_is_sent() {
    let (mut p, host) = inbox_rig();
    open(&mut p, &host, "auction_search");
    let n = sent().len();
    p.death = Some(crate::death::DeathCam::new([0.; 3], [0., 0., 1.], [0.; 3]));
    for call in ["return eqoa.actions.mail_refresh()", "return eqoa.actions.auction_search()", "return eqoa.actions.mail_read(11)"] {
        let (ok, code, _) = act(&mut p, &host, call);
        assert!(!ok && code == "not_ready", "{call}: {code}");
    }
    assert_eq!(sent().len(), n);
    super::dropped();
    assert!(native_a10::current_mail().is_none() && native_a10::current_auction().is_none());
}
