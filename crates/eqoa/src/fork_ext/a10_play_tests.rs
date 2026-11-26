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
    crate::fork_ext::bindpage::reset_in_memory();
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
    it.item.value = 50;
    // no art to fetch: the headless host has no icon textures (the Bags window asks for one only when an item has an icon)
    it.item.icon = 0;
    it.item.icon2 = 0;
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
    let call = r#"return eqoa.actions.mail_send({to = "Bob", subject = "S", body = "B", tunar = 100, item = {slot = 2, id = '900002', count = 2}})"#;
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
    let (ok, code, _) = act(&mut p, &host, r#"return eqoa.actions.mail_send({to = "Bob", subject = "S", body = "B", item = {slot = 2, id = '900002', count = 9}})"#);
    assert!(!ok && code == "out_of_range", "{code}");
    let (ok, code, _) = act(&mut p, &host, r#"return eqoa.actions.mail_send({to = "Bob", subject = "S", body = "B", item = {slot = 2, id = '900001', count = 1}})"#);
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
    let (ok, _, _) = act(&mut p, &host, r#"return eqoa.actions.mail_send({to = "Bob", subject = "S", body = "B", item = {slot = 2, id = '900002', count = 2}})"#);
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
    assert!(!ok && code == "not_ready", "{code}");
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
    assert!(!ok && code == "window_closed", "{code}");
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
    page_reply_cols(page, lots, [7, -1, 3])
}

/// The same with the page's three columns set (the leader, the high bid, the bids or the next bid's floor).
fn page_reply_cols(page: i64, lots: &[(i64, i64, i64)], cols: [i64; 3]) -> Vec<u8> {
    let mut w = Writer::default();
    w.u8(129).signed(page).signed(30).signed(lots.len() as i64);
    for (id, qty, buyout) in lots {
        lot_bytes(&mut w, *id, *qty, *buyout, 500);
    }
    for col in cols {
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
    // answered: free again (the game's own reply ends the wait, nothing here does)
    p.bank_opcode(0x1206, &{
        let mut w = Writer::default();
        w.u8(132);
        w.data
    });
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
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.auction_bid({lot = 501, price = 150})");
    assert!(!ok && code == "not_ready", "{code}");
    assert!(!confirm::is_open());
    p.dump.bank.lots.clear();
    // the server's clock runs past the lot's end
    p.bank.auction.minutes = 900;
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.auction_bid({lot = 501, price = 150})");
    assert!(!ok && code == "not_ready", "{code}");
    assert!(!confirm::is_open());
    // the control: back in time, it asks
    p.bank.auction.minutes = 30;
    let (ok, _, _) = act(&mut p, &host, "return eqoa.actions.auction_bid({lot = 501, price = 150})");
    assert!(ok && confirm::is_open());
    // the disc's auction takes one item at a time
    confirm::reset_for_test();
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.auction_bid({lot = 501, price = 150, count = 2})");
    assert!(!ok && code == "invalid_argument", "{code}");
}

#[test]
fn a10_the_live_auction_bids_for_a_count_and_buys_out_the_whole_stack() {
    // the play fixture sets the feature word itself, so the live word is set after it is made
    let (mut p, host) = auction_rig();
    crate::live::with_features(Some(0x1B), || {
        lot_page(&mut p, &host, &[(501, 20, 400)]);
        let n = sent().len();
        let (ok, code, reason) = act(&mut p, &host, "return eqoa.actions.auction_bid({lot = 501, price = 150, count = 5})");
        assert!(ok, "{code} {reason}");
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
    let call = "return eqoa.actions.auction_sell({slot = 1, id = '700001', price = 100, buyout = 300, hours = 12})";
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
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.auction_sell({slot = 2, id = '700002', price = 100, buyout = 300, hours = 12})");
    assert!(!ok && code == "not_ready", "{code}");
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.auction_sell({slot = 1, id = '700002', price = 100, buyout = 300, hours = 12})");
    assert!(!ok && code == "stale_handle", "{code}");
    // the disc's auction puts one up at a time
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.auction_sell({slot = 1, id = '700001', count = 3, price = 100, buyout = 300, hours = 12})");
    assert!(!ok && code == "invalid_argument", "{code}");
    assert!(!confirm::is_open());
}

#[test]
fn a10_selling_needs_the_mods_own_bank_and_the_bank_offering_the_auction_and_the_item_still_there_at_the_yes() {
    // no bank held: refused, with the words that say what to do
    let (mut p, host) = rig();
    p.dump.bank.items = net::dump::ItemList { p04: 0, items: vec![item(700_001, "Gold Ring", 21, 1)] };
    let (ok, code, reason) = act(&mut p, &host, "return eqoa.actions.auction_sell({slot = 1, id = '700001', price = 100, buyout = 300, hours = 12})");
    assert!(!ok && code == "window_closed" && reason.contains("hold the bank"), "{code} {reason}");
    // the bank offers no auction: refused (the control: it works in the next block)
    let (mut p, host) = bank_rig();
    p.dump.bank.p78cc = false;
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.auction_sell({slot = 1, id = '700001', price = 100, buyout = 300, hours = 12})");
    assert!(!ok && code == "unsupported", "{code}");
    p.dump.bank.p78cc = true;
    let (ok, _, _) = act(&mut p, &host, "return eqoa.actions.auction_sell({slot = 1, id = '700001', price = 100, buyout = 300, hours = 12})");
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
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.auction_sell({slot = 1, id = '700001', price = 100, buyout = 300, hours = 12})");
    assert!(!ok && code == "not_ready", "{code}");
}

#[test]
fn a10_actions_are_offered_only_where_the_bank_offers_the_auction() {
    let (mut p, host) = rig();
    p.dump.bank.p78cc = false;
    open(&mut p, &host, "auction_search");
    lua_ok(&host, "assert(eqoa.auction.get().offered == false)");
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.auction_search()");
    assert!(!ok && code == "unsupported", "{code}");
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
    lua_ok(&host, r#"assert(eqoa.auction.get().pending == "search" and eqoa.auction.get().uncertain == nil)"#);
    LOCAL.with(|l| l.borrow_mut().awaiting = Some(Instant::now() - GIVE_UP - Duration::from_secs(1)));
    step(&mut p, &host);
    lua_ok(&host, r#"local a = eqoa.auction.get(); assert(a.pending == "none" and a.uncertain == "search", a.pending)"#);
    assert_eq!(p.bank.state, 0);
    assert_eq!(sent().len(), n, "never replayed");
    // a new request goes, and the page it brings is the fresh read that ends the doubt
    let (ok, _, _) = act(&mut p, &host, "return eqoa.actions.auction_search()");
    assert!(ok);
    lua_ok(&host, r#"assert(eqoa.auction.get().uncertain == "search", "not before a page arrives")"#);
    let page = p.bank.auction.page as i64;
    p.bank_opcode(0x1206, &page_reply(page, &[(501, 1, 400)]));
    step(&mut p, &host);
    lua_ok(&host, "assert(eqoa.auction.get().uncertain == nil)");
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
    assert!(!ok && code == "not_ready", "{code}");
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

// ---- the fix round (review of 2026-11-24) ----

fn live_rig() -> (Play, Host) {
    // the play fixture sets the feature word itself, so the live word is set after it is made
    auction_rig()
}

#[test]
fn a10_m1_a_bid_is_held_to_the_games_own_bid_form_limits_and_the_question_shows_the_buyout() {
    let (mut p, host) = auction_rig();
    // the disc's form: the lowest bid is the lot's minimum or one above the high bid
    let (ok, _, _) = act(&mut p, &host, "return eqoa.actions.auction_search()");
    assert!(ok);
    let page = p.bank.auction.page as i64;
    p.bank_opcode(0x1206, &page_reply_cols(page, &[(501, 1, 400)], [7, 100, 3]));
    step(&mut p, &host);
    let n = sent().len();
    for (price, why) in [(100, "not above the high bid"), (5, "below the minimum"), (401, "above the buyout"), (500000, "far above the buyout")] {
        let (ok, code, _) = act(&mut p, &host, &format!("return eqoa.actions.auction_bid({{lot = 501, price = {price}}})"));
        assert!(!ok && code == "out_of_range", "{why}: {code}");
        assert!(!confirm::is_open(), "{why}: no question for a bid the game's form cannot make");
    }
    assert_eq!(sent().len(), n);
    // the control: the lowest allowed bid, and the buyout itself, ask; and the question shows the buyout
    let (ok, _, _) = act(&mut p, &host, "return eqoa.actions.auction_bid({lot = 501, price = 101})");
    assert!(ok && confirm::is_open());
    assert!(confirm::current_for_test().unwrap().text.contains("buyout is 400"), "{}", confirm::current_for_test().unwrap().text);
    confirm::reset_for_test();
    let (ok, _, _) = act(&mut p, &host, "return eqoa.actions.auction_bid({lot = 501, price = 400})");
    assert!(ok, "a bid at the buyout is the form's top");
}

#[test]
fn a10_m1_the_live_form_holds_the_count_at_the_existing_bids_and_the_buyout_price_to_the_whole_stack() {
    let (mut p, host) = auction_rig();
    crate::live::with_features(Some(0x1B), || {
        lot_page(&mut p, &host, &[(501, 20, 400)]);
        // the floor is the page's third column (3) and the lot's minimum (10)
        let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.auction_bid({lot = 501, price = 9})");
        assert!(!ok && code == "out_of_range", "{code}");
        // a bid already for 5: fewer is refused, the same count asks
        p.dump.bank.bids.push(net::dump::Bid { character_id: 1, auction_id: 501, x08: 150, x0c: 5, x10: 750, x14: 0 });
        let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.auction_bid({lot = 501, price = 160, count = 3})");
        assert!(!ok && code == "out_of_range", "{code}");
        let (ok, _, _) = act(&mut p, &host, "return eqoa.actions.auction_bid({lot = 501, price = 160, count = 5})");
        assert!(ok);
        assert!(confirm::current_for_test().unwrap().text.contains("800 in all"), "{}", confirm::current_for_test().unwrap().text);
        confirm::reset_for_test();
        // at the buyout price only for the whole stack (the game's word 1858)
        let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.auction_bid({lot = 501, price = 400, count = 5})");
        assert!(!ok && code == "out_of_range", "{code}");
        let (ok, _, _) = act(&mut p, &host, "return eqoa.actions.auction_bid({lot = 501, price = 400, count = 20})");
        assert!(ok);
    });
}

#[test]
fn a10_m1_a_lot_that_changed_before_the_yes_is_not_bid_on() {
    let (mut p, host) = auction_rig();
    lot_page(&mut p, &host, &[(501, 1, 400)]);
    let n = sent().len();
    let (ok, _, _) = act(&mut p, &host, "return eqoa.actions.auction_bid({lot = 501, price = 150})");
    assert!(ok);
    // the buyout drops under the price the player was asked about (a page that changed the lot)
    p.bank.auction.results[0].p2f8 = 100;
    say_yes();
    step(&mut p, &host);
    assert_eq!(sent().len(), n, "negative control: the price is above the buyout now, nothing is bid");
    // the minimum rises above it
    let (ok, _, _) = act(&mut p, &host, "return eqoa.actions.auction_bid({lot = 501, price = 90})");
    assert!(ok);
    p.bank.auction.results[0].p2f4 = 95;
    say_yes();
    step(&mut p, &host);
    assert_eq!(sent().len(), n);
}

#[test]
fn a10_m2_a_sale_is_held_to_the_games_own_sell_form_limits() {
    let (mut p, host) = bank_rig();
    let n = sent().len();
    for (call, why) in [
        ("{slot = 1, id = '700001', price = 49, buyout = 300, hours = 12}", "below the item's value (50)"),
        ("{slot = 1, id = '700001', price = 0, buyout = 1, hours = 12}", "free"),
        ("{slot = 1, id = '700001', price = 100, buyout = 99, hours = 12}", "buyout below the price"),
        ("{slot = 1, id = '700001', price = 100, buyout = 1000000000, hours = 12}", "above 999999999"),
        ("{slot = 1, id = '700001', price = 100, buyout = 300, hours = 11}", "under 12 hours"),
        ("{slot = 1, id = '700001', price = 100, buyout = 300, hours = 73}", "over 72 hours"),
        ("{slot = 1, id = '700001', price = 100, buyout = 300, hours = 10000}", "far over"),
    ] {
        let (ok, code, _) = act(&mut p, &host, &format!("return eqoa.actions.auction_sell({call})"));
        assert!(!ok && code == "out_of_range", "{why}: {code}");
        assert!(!confirm::is_open(), "{why}: no question for a sale the game's form cannot make");
    }
    assert_eq!(sent().len(), n);
    // the controls: the form's edges ask
    for call in [
        "{slot = 1, id = '700001', price = 50, buyout = 50, hours = 12}",
        "{slot = 1, id = '700001', price = 100, buyout = 999999999, hours = 72}",
    ] {
        confirm::reset_for_test();
        let (ok, _, _) = act(&mut p, &host, &format!("return eqoa.actions.auction_sell({call})"));
        assert!(ok && confirm::is_open(), "{call}");
    }
    // a sale whose limits stop holding before the Yes (the item's value changes) is not sent
    p.dump.bank.items.items[0].item.value = 5000;
    say_yes();
    step(&mut p, &host);
    assert_eq!(sent().len(), n);
}

#[test]
fn a10_m3_the_mods_window_going_never_empties_the_games_own_auction_window() {
    for how in ["close", "disable", "lua_error", "death"] {
        let (mut p, host) = auction_rig();
        lot_page(&mut p, &host, &[(501, 1, 400)]);
        // the player opens the game's own Auction over it and its page is shown
        p.mode = 39;
        let page_before = p.bank.auction.page;
        assert_eq!(p.bank.auction.results.len(), 1);
        match how {
            "close" => host.execute("assert(eqoa.windows.close_session(s_auction_search.ref))").unwrap(),
            "disable" => ws::end_vm_told(native_events::bus(&host.lua).borrow().vm, "disabled"),
            "lua_error" => ws::mods_stopped("lua_error"),
            _ => ws::end_own("death"),
        }
        step(&mut p, &host);
        assert_eq!(p.bank.auction.results.len(), 1, "{how}: the page the player is looking at stays");
        assert_eq!(p.bank.auction.page, page_before, "{how}: the game's page counter is not bumped");
        // the control: with the game's window not up, the same ending does clear the mod's own page
        let (mut q, host2) = auction_rig();
        lot_page(&mut q, &host2, &[(501, 1, 400)]);
        ws::mods_stopped("lua_error");
        step(&mut q, &host2);
        assert!(q.bank.auction.results.is_empty(), "{how}: control");
    }
}

#[test]
fn a10_l1_a_bid_given_up_on_stays_unknown_until_a_fresh_read_and_is_not_repeated() {
    let (mut p, host) = auction_rig();
    lot_page(&mut p, &host, &[(501, 1, 400), (502, 1, 400)]);
    let (ok, _, _) = act(&mut p, &host, "return eqoa.actions.auction_bid({lot = 501, price = 150})");
    assert!(ok);
    say_yes();
    step(&mut p, &host);
    let n = sent().len();
    LOCAL.with(|l| l.borrow_mut().out = Some((4, Instant::now() - GIVE_UP - Duration::from_secs(1))));
    step(&mut p, &host);
    lua_ok(&host, r#"assert(eqoa.auction.get().uncertain == "bid" and eqoa.auction.get().pending == "none")"#);
    // the same lot is not bid on again (nothing asked), another is
    let (ok, code, reason) = act(&mut p, &host, "return eqoa.actions.auction_bid({lot = 501, price = 150})");
    assert!(!ok && code == "busy" && reason.contains("may have gone through"), "{code} {reason}");
    assert!(!confirm::is_open());
    let (ok, _, _) = act(&mut p, &host, "return eqoa.actions.auction_bid({lot = 502, price = 150})");
    assert!(ok, "another lot is not in doubt");
    confirm::reset_for_test();
    assert_eq!(sent().len(), n, "nothing replayed");
    // a fresh page: the doubt ends and the lot may be bid on
    lot_page(&mut p, &host, &[(501, 1, 400)]);
    lua_ok(&host, "assert(eqoa.auction.get().uncertain == nil)");
    let (ok, _, _) = act(&mut p, &host, "return eqoa.actions.auction_bid({lot = 501, price = 160})");
    assert!(ok);
}

#[test]
fn a10_l2_the_give_up_never_resets_the_game_windows_own_request_state() {
    let (mut p, host) = auction_rig();
    lot_page(&mut p, &host, &[(501, 1, 400)]);
    let (ok, _, _) = act(&mut p, &host, "return eqoa.actions.auction_bid({lot = 501, price = 150})");
    assert!(ok);
    say_yes();
    step(&mut p, &host);
    assert_eq!(p.bank.state, 4);
    // the player's own window opens and has its own request out (state 5); the mod's request goes stale
    p.mode = 39;
    p.bank.state = 5;
    LOCAL.with(|l| l.borrow_mut().out = Some((5, Instant::now() - GIVE_UP - Duration::from_secs(1))));
    step(&mut p, &host);
    assert_eq!(p.bank.state, 5, "the game's own window does not look idle early");
    // the control: with no game window up the give-up resets, as before
    p.mode = 0;
    LOCAL.with(|l| l.borrow_mut().out = Some((5, Instant::now() - GIVE_UP - Duration::from_secs(1))));
    step(&mut p, &host);
    assert_eq!(p.bank.state, 0);
}

#[test]
fn a10_l3_opening_the_mods_auction_window_does_not_forget_a_sale_waiting() {
    let (mut p, host) = bank_rig();
    let (ok, _, _) = act(&mut p, &host, "return eqoa.actions.auction_sell({slot = 1, id = '700001', price = 100, buyout = 300, hours = 12})");
    assert!(ok);
    say_yes();
    step(&mut p, &host);
    assert_eq!(p.bank.state, 3);
    assert!(LOCAL.with(|l| l.borrow().out.is_some()));
    open(&mut p, &host, "auction_search");
    assert!(LOCAL.with(|l| l.borrow().out.is_some()), "the sale is still waited for, and still covered by the give-up");
    assert_eq!(p.bank.state, 3);
    // and it is given up on as before (a sale is the bank's)
    LOCAL.with(|l| l.borrow_mut().out = Some((3, Instant::now() - GIVE_UP - Duration::from_secs(1))));
    step(&mut p, &host);
    lua_ok(&host, r#"assert(eqoa.auction.get().uncertain == "sell")"#);
    // the same bank record is not put up again until a fresh read
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.auction_sell({slot = 1, id = '700001', price = 100, buyout = 300, hours = 12})");
    assert!(!ok && code == "busy", "{code}");
}

#[test]
fn a10_l4_a_mail_that_carries_something_goes_to_one_name_and_the_question_names_it_as_sent() {
    let (mut p, host) = inbox_rig();
    open(&mut p, &host, "mail_compose");
    p.mail.grams.live = true;
    p.mail.grams.attach.offer = Some(Offer { items: 1, postage: 0 });
    let n = sent().len();
    // two names: refused, nothing asked (control: plain mail to two names still goes, as the game's compose does)
    let (ok, code, _) = act(&mut p, &host, r#"return eqoa.actions.mail_send({to = "Bob, Ann", subject = "S", body = "B", tunar = 10})"#);
    assert!(!ok && code == "invalid_argument", "{code}");
    assert!(!confirm::is_open());
    let (ok, _, _) = act(&mut p, &host, r#"return eqoa.actions.mail_send({to = "Bob, Ann", subject = "S", body = "B"})"#);
    assert!(ok);
    assert_eq!(sent().len(), n + 1);
    p.mail.grams.state = 0;
    // the question names the name as it will be sent (the game keeps A-Z only and capitalises)
    let (ok, _, _) = act(&mut p, &host, r#"return eqoa.actions.mail_send({to = "b0b", subject = "S", body = "B", tunar = 10})"#);
    assert!(ok);
    let t = confirm::current_for_test().unwrap().text;
    assert!(t.contains("to Bb "), "{t}");
    // a subject of two lines is refused where it is parsed
    confirm::reset_for_test();
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.mail_send({to = 'Bob', subject = 'a\\nb', body = 'B', tunar = 10})");
    assert!(!ok && code == "invalid_argument", "{code}");
}

#[test]
fn a10_l5_one_take_at_a_time_and_l7_a_mail_id_is_the_whole_u32() {
    let (mut p, host) = inbox_rig();
    p.mail.grams.attach.carried = Some(crate::mailattach::Carried { id: 11, tunar: 40, items: vec![], returned: false });
    let (ok, _, _) = act(&mut p, &host, "return eqoa.actions.mail_take(11)");
    assert!(ok);
    let n = sent().len();
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.mail_take(11)");
    assert!(!ok && code == "busy", "{code}");
    assert_eq!(sent().len(), n, "negative control: the second Take is not sent");
    // the server answers: the mail carries nothing now, and the question is over
    p.mail.grams.attach.carried = Some(crate::mailattach::Carried { id: 11, tunar: 0, items: vec![], returned: false });
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.mail_take(11)");
    assert!(!ok && code == "not_ready", "{code}");
    // an id past i32 is taken as a mail id (it is not in the inbox: a stale handle, not a bad argument)
    let (ok, code, _) = act(&mut p, &host, "return eqoa.actions.mail_read(3000000000)");
    assert!(!ok && code == "stale_handle", "{code}");
}

// ---- the bundled mods (Mail, Auction) and the Bag Viewer's listing, driven against the real game state ----

mod mods {
    use super::*;
    use crate::fork_ext::addons::{
        bridge::Bridge,
        discovery::DiscoveredAddon,
    };
    use dear_imgui_rs::Context;
    use serde_json::json;
    use std::path::PathBuf;

    fn root(id: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bundled-mods").join(id).canonicalize().unwrap()
    }

    fn addon(id: &str) -> DiscoveredAddon {
        crate::fork_ext::addons::discovery::discover(root(id).parent().unwrap())
            .addons
            .into_iter()
            .find(|a| a.id == id)
            .unwrap_or_else(|| panic!("{id} is bundled"))
    }

    fn context() -> Context {
        let mut context = Context::create();
        context.set_ini_filename::<PathBuf>(None).unwrap();
        context.io_mut().set_display_size([1024.0, 768.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        context.font_atlas().try_claim_legacy_renderer().unwrap().build();
        context
    }

    struct Rig {
        p: Play,
        host: Host,
        bridge: Bridge,
        context: Context,
        w: f64,
        h: f64,
        k: f64,
        x0: f64,
        y0: f64,
    }

    impl Rig {
        fn frames(&mut self, n: usize) {
            for _ in 0..n {
                CORE.with(|c| c.borrow_mut().frame += 1);
                self.context.frame();
                self.bridge
                    .invoke(|| {
                        native_events::dispatch(&self.host)?;
                        native_events::render(&self.host)
                    })
                    .unwrap();
                drop(self.context.render_legacy());
            }
        }

        fn settle(&mut self) {
            // the game's own snapshot of the pack, the bank and the world, as the client takes it
            self.p.forge_snapshot(None);
            step(&mut self.p, &self.host);
            self.frames(2);
        }

        fn relayout(&mut self) {
            let (k, sx, sy, ox, oy): (f64, f64, f64, f64, f64) = self
                .host
                .lua
                .load("local f = eqoa.hud.frame(); return math.min(f.scale_x, f.scale_y), f.scale_x, f.scale_y, f.offset_x, f.offset_y")
                .eval()
                .unwrap();
            self.k = k;
            self.x0 = (1024.0 - self.w * k).min(ox + (640.0 - self.w) / 2.0 * sx).max(0.0);
            self.y0 = (768.0 - self.h * k).min(oy + (448.0 - self.h) / 2.0 * sy).max(0.0);
        }

        fn tap(&mut self, x: f64, y: f64) {
            self.relayout();
            let (px, py) = (self.x0 + x * self.k, self.y0 + y * self.k);
            CORE.with(|c| c.borrow_mut().emit("pointer.pressed", json!({"x": px, "y": py, "inside_window": true, "button": "left"})));
            native_events::dispatch(&self.host).unwrap();
            self.settle();
        }

        fn key(&mut self, key: &str, text: &str) {
            CORE.with(|c| {
                c.borrow_mut().emit("input.key", json!({"key": key, "combo": key, "down": true, "repeat": false, "synthetic": false, "text": text}))
            });
            native_events::dispatch(&self.host).unwrap();
            self.frames(1);
        }

        fn type_text(&mut self, s: &str) {
            for ch in s.chars() {
                let t = ch.to_string();
                self.key(&t, &t);
            }
        }

        fn why(&self) -> String {
            let sessions: Vec<String> = ws::sessions_for_test().iter().map(|x| format!("{}:{}", x.kind, x.vm)).collect();
            let form: String = self
                .host
                .lua
                .load("local ok, L = pcall(require, 'auction_list'); if not ok then return 'no module' end; local a = eqoa.auction.get(); return 'form=' .. tostring(L.form and L.form.mode) .. ' frames=' .. tostring(L.form and L.form.frames) .. ' session=' .. tostring(L.session) .. ' auction=' .. tostring(a) .. ' sellable=' .. tostring(a and #a.sellable) .. ' bank=' .. tostring(eqoa.bank.get() and eqoa.bank.get().open)")
                .eval()
                .unwrap_or_else(|e| format!("({e})"));
            format!(
                "queued {:?}; logs {:?}; mode {}; sessions {sessions:?}; hidden bank {:?}; published {}; {form}",
                action_broker::queued_kinds_for_test(),
                self.host.drain_logs(),
                self.p.mode,
                window_takeover::hidden_bank_session(),
                native_a10::current_auction().is_some()
            )
        }
    }

    /// The mod's window open, as its own key opens it.
    fn rig_with(id: &str, w: f64, h: f64, combo: &str, key: &str, prep: impl FnOnce(&mut Play)) -> Rig {
        let (mut p, _unused) = super::rig();
        prep(&mut p);
        crate::fork_ext::addons::hud_layout::set_window_size(1024, 768);
        let context = context();
        let host = Host::for_addon(&addon(id)).unwrap();
        let bridge = Bridge::install(&host).unwrap();
        host.run_addon(&addon(id)).unwrap();
        let mut r = Rig { p, host, bridge, context, w, h, k: 1.0, x0: 0.0, y0: 0.0 };
        CORE.with(|c| {
            c.borrow_mut().emit("input.key", json!({"key": key, "combo": combo, "down": true, "repeat": false, "synthetic": false, "text": ""}))
        });
        native_events::dispatch(&r.host).unwrap();
        r.settle();
        r
    }

    fn mail_rig(prep: impl FnOnce(&mut Play)) -> Rig {
        rig_with("eqoa_mail", 360.0, 336.0, "shift+m", "m", prep)
    }

    fn auction_rig_mod() -> Rig {
        rig_with("eqoa_auction", 380.0, 364.0, "shift+u", "u", |_| {})
    }

    #[test]
    fn mods_mail_reads_a_mail_and_a_delete_asks_first_and_is_sent_only_on_the_yes() {
        let mut r = mail_rig(|_| {});
        assert!(ws::any_open_of(&["mail_inbox"]), "Shift+M opened a mail_inbox session; {}", r.why());
        let mut native = Grams::default();
        native.request_inbox();
        assert_eq!(sent(), vec![native.out[0].clone()], "the inbox is asked for as the game's own does");
        r.p.mail_recv(&inbox_reply(&[header(11, "Ann", "One"), header(12, "Bob", "Two")]));
        r.settle();
        // a click on the second mail asks for its text, as the game's inbox does
        r.tap(60.0, 59.0);
        let mut g = Grams::default();
        g.headers = r.p.mail.grams.headers.clone();
        g.count = 2;
        g.request_message(12);
        assert_eq!(sent().last().unwrap(), &g.out[0], "{}", r.why());
        r.p.mail_recv(&message_reply(12, "hello there"));
        r.settle();
        // Delete: the question comes first and nothing is sent
        let n = sent().len();
        r.tap(250.0, 304.0);
        assert_eq!(sent().len(), n, "negative control: the click alone deletes nothing; {}", r.why());
        let q = confirm::current_for_test().expect("the question is up");
        assert!(!q.yes && q.text.contains("Two") && q.text.contains("Bob"), "{}", q.text);
        say_yes();
        r.settle();
        let mut g = Grams::default();
        g.request_inbox();
        g.recv(&inbox_reply(&[header(11, "Ann", "One"), header(12, "Bob", "Two")]));
        g.out.clear();
        g.delete_message(12);
        assert_eq!(sent().len(), n + 1);
        assert_eq!(sent().last().unwrap(), &g.out[0]);
        let logs = r.host.drain_logs().unwrap();
        assert!(logs.is_empty(), "no diagnostics: {logs:?}");
    }

    #[test]
    fn mods_mail_a_mail_that_carries_something_goes_to_one_name_and_asks_first() {
        let mut r = mail_rig(|p| {
            p.mail.grams.live = true;
            p.mail.grams.attach.offer = Some(Offer { items: 1, postage: 15 });
        });
        r.p.mail_recv(&inbox_reply(&[header(11, "Ann", "One")]));
        r.settle();
        // the Write tab; To, Subject, Text and Tunar typed (Tab moves on)
        r.tap(110.0, 28.0);
        assert!(ws::any_open_of(&["mail_compose"]), "{}", r.why());
        r.tap(30.0, 58.0);
        r.type_text("Bob, Ann");
        r.key("tab", "");
        r.type_text("Hi");
        r.key("tab", "");
        r.type_text("yo");
        r.key("tab", "");
        r.type_text("10");
        r.key("escape", "");
        let n = sent().len();
        // two names and tunar: the Send button is not there, and nothing is asked or sent
        r.tap(30.0, 304.0);
        assert_eq!(sent().len(), n, "{}", r.why());
        assert!(!confirm::is_open(), "no question for a mail the game's rule refuses");
        // the control: the same mail without tunar goes at once to both names, as the game's compose sends it
        r.tap(90.0, 304.0);
        r.tap(30.0, 58.0);
        r.type_text("Bob, Ann");
        r.key("tab", "");
        r.type_text("Hi");
        r.key("tab", "");
        r.type_text("yo");
        r.key("escape", "");
        r.tap(30.0, 304.0);
        let mut g = Grams::default();
        g.live = true;
        g.send_gram_live(&[b"Bob".to_vec(), b"Ann".to_vec()], &crate::ui::wide("Hi"), &crate::ui::wide("yo"));
        assert_eq!(sent().len(), n + 1, "{}", r.why());
        assert_eq!(sent().last().unwrap(), &g.out[0]);
        // the answer comes back: the mail is out and the form empties
        r.p.mail_recv(&[1, 1]);
        r.settle();
        // one name and tunar: asks, shows the name, sends once on the Yes
        r.tap(30.0, 58.0);
        r.type_text("Bob");
        r.key("tab", "");
        r.type_text("Hi");
        r.key("tab", "");
        r.type_text("yo");
        r.key("tab", "");
        r.type_text("10");
        r.key("escape", "");
        let m = sent().len();
        r.tap(30.0, 304.0);
        assert_eq!(sent().len(), m, "the question comes first; {}", r.why());
        let q = confirm::current_for_test().expect("the question is up");
        assert!(q.text.contains("10 tunar") && q.text.contains("to Bob ") && q.text.contains("postage is 15"), "{}", q.text);
        say_yes();
        r.settle();
        assert_eq!(sent().len(), m + 1);
        assert_eq!(
            sent().last().unwrap(),
            &crate::mailattach::send(&[b"Bob".to_vec()], &crate::ui::wide("Hi"), &crate::ui::wide("yo"), 10, None)
        );
        let logs = r.host.drain_logs().unwrap();
        assert!(logs.is_empty(), "no diagnostics: {logs:?}");
    }

    #[test]
    fn mods_auction_a_search_is_the_games_bytes_and_a_bid_or_a_buy_now_asks_first_with_the_price() {
        let mut r = auction_rig_mod();
        assert!(ws::any_open_of(&["auction_search"]), "Shift+U opened an auction_search session; {}", r.why());
        let mut want = r.p.bank.auction.clone();
        let q = Query {
            kind: 12,
            text: "Iron".into(),
            name: String::new(),
            p9c: 0xFFFF_FFFF,
            pa0: 0xFFFF_FFFF,
            pa4: 1,
            pa8: crate::s2::get().auction_max_level,
            pac: 0,
            pb0: 0x7FFF_FFFF,
            pb4: -2,
            pb8: -1,
            pbc: true,
        };
        let expected = want.search(q);
        r.tap(100.0, 46.0);
        r.type_text("Iron");
        r.tap(200.0, 67.0);
        assert_eq!(sent(), vec![expected], "the search the game's form would send; {}", r.why());
        // the page: one lot, bid and buy now
        let page = r.p.bank.auction.page as i64;
        r.p.bank_opcode(0x1206, &page_reply(page, &[(501, 1, 400)]));
        r.settle();
        r.tap(60.0, 132.0);
        let n = sent().len();
        r.tap(256.0, 295.0);
        assert_eq!(sent().len(), n, "negative control: the click alone bids nothing; {}", r.why());
        let q = confirm::current_for_test().expect("the question is up");
        assert!(!q.yes && q.text.contains("Bid 10 tunar") && q.text.contains("buyout is 400"), "{}", q.text);
        say_yes();
        r.settle();
        assert_eq!(sent().len(), n + 1);
        assert_eq!(sent().last().unwrap(), &Auction::bid(501, 10));
        // answered; buy now asks with its price
        r.p.bank.state = 0;
        r.settle();
        r.tap(330.0, 295.0);
        let q = confirm::current_for_test().expect("the buy now question is up");
        assert!(q.text.contains("Buy out") && q.text.contains("400"), "{}", q.text);
        assert_eq!(sent().len(), n + 1);
        say_yes();
        r.settle();
        assert_eq!(sent().last().unwrap(), &Auction::bid_n(501, 400, 1));
        let logs = r.host.drain_logs().unwrap();
        assert!(logs.is_empty(), "no diagnostics: {logs:?}");
    }

    #[test]
    fn mods_auction_with_no_auction_on_the_server_the_window_says_so_and_sends_nothing() {
        let mut r = rig_with("eqoa_auction", 380.0, 364.0, "shift+u", "u", |p| p.dump.bank.p78cc = false);
        r.settle();
        r.tap(200.0, 67.0);
        assert!(sent().is_empty(), "{}", r.why());
    }

    #[test]
    fn mods_zoning_and_a_new_world_end_the_windows_and_nothing_stale_is_left_to_reopen_with() {
        for (id, w, combo, key, kinds) in [
            ("eqoa_mail", 360.0, "shift+m", "m", vec!["mail_inbox", "mail_compose"]),
            ("eqoa_auction", 380.0, "shift+u", "u", vec!["auction_search"]),
        ] {
            for how in ["zoning", "state.reset", "world.entered"] {
                let mut r = rig_with(id, w, if id == "eqoa_auction" { 364.0 } else { 336.0 }, combo, key, |_| {});
                assert!(ws::any_open_of(&kinds), "{id}: open; {}", r.why());
                // a box is being typed in, so the Enter claim is held
                if id == "eqoa_auction" {
                    r.tap(100.0, 46.0);
                } else {
                    r.tap(110.0, 28.0);
                    r.tap(30.0, 58.0);
                }
                // control: nothing has happened yet, it is still open
                assert!(ws::any_open_of(&kinds));
                match how {
                    "zoning" => {
                        ws::set_env(ws::Env { away: true, ..ws::Env::default() });
                        r.frames(2);
                        ws::set_env(ws::Env::default());
                    }
                    other => {
                        CORE.with(|c| c.borrow_mut().emit(other, json!({})));
                        native_events::dispatch(&r.host).unwrap();
                        r.frames(2);
                    }
                }
                assert!(!ws::any_open_of(&kinds), "{id} after {how}: the window and its session are gone");
                // the mod works again at once: the key opens it, a box takes typing, the Enter claim is taken fresh
                CORE.with(|c| {
                    c.borrow_mut().emit("input.key", json!({"key": key, "combo": combo, "down": true, "repeat": false, "synthetic": false, "text": ""}))
                });
                native_events::dispatch(&r.host).unwrap();
                r.settle();
                assert!(ws::any_open_of(&kinds), "{id} after {how}: reopens; {}", r.why());
                r.tap(100.0, 46.0);
                r.type_text("ab");
                let logs = r.host.drain_logs().unwrap();
                assert!(logs.is_empty(), "{id} after {how}: no diagnostics: {logs:?}");
            }
        }
    }

    // ---- the fix round: the real mods, real clicks and keys, the real game state behind them ----

    /// The window pixels of a thing the Bag Viewer drew, read from its own tables.
    fn bv_point(r: &Rig, expr: &str) -> (f64, f64) {
        let (x, y): (f64, f64) = r.host.lua.load(expr).eval().unwrap_or_else(|e| panic!("{expr}: {e}"));
        (x, y)
    }

    impl Rig {
        /// A press at window pixels (not HUD pixels), with a modifier or another button.
        fn press_px(&mut self, x: f64, y: f64, button: &str, ctrl: bool) {
            CORE.with(|c| c.borrow_mut().emit("pointer.pressed", json!({"x": x, "y": y, "inside_window": true, "button": button, "ctrl": ctrl, "shift": false, "alt": false})));
            native_events::dispatch(&self.host).unwrap();
            self.settle();
        }

        /// A click in the HUD pixels of the window, drawn BEFORE the game steps (as the client's frame runs), so what
        /// the mod draws on that frame is the state before the click's action.
        fn tap_then_draw_first(&mut self, x: f64, y: f64) {
            self.relayout();
            let (px, py) = (self.x0 + x * self.k, self.y0 + y * self.k);
            CORE.with(|c| c.borrow_mut().emit("pointer.pressed", json!({"x": px, "y": py, "inside_window": true, "button": "left"})));
            native_events::dispatch(&self.host).unwrap();
            self.frames(1);
            step(&mut self.p, &self.host);
            self.frames(1);
        }
    }

    fn writes(r: &mut Rig, to: &str, subject: &str, body: &str) {
        r.tap(30.0, 58.0);
        r.type_text(to);
        r.key("tab", "");
        r.type_text(subject);
        r.key("tab", "");
        r.type_text(body);
        r.key("escape", "");
    }

    #[test]
    fn fix_h1_the_item_picker_wins_the_click_and_the_item_goes_with_the_mail() {
        let mut r = mail_rig(|p| {
            p.mail.grams.live = true;
            p.mail.grams.attach.offer = Some(Offer { items: 1, postage: 0 });
        });
        r.p.mail_recv(&inbox_reply(&[header(11, "Ann", "One")]));
        r.settle();
        r.tap(110.0, 28.0);
        writes(&mut r, "Bob", "Gift", "yo");
        // Attach an item, then its first row: the picker is in front of the Text box
        r.tap(260.0, 198.0);
        r.tap(200.0, 112.0);
        let n = sent().len();
        r.tap(30.0, 304.0);
        assert_eq!(sent().len(), n, "the question comes first; {}", r.why());
        let q = confirm::current_for_test().expect("the question is up: the item was attached");
        assert!(q.text.contains("Rusty Sword"), "{}", q.text);
        say_yes();
        r.settle();
        assert_eq!(
            sent().last().unwrap(),
            &crate::mailattach::send(&[b"Bob".to_vec()], &crate::ui::wide("Gift"), &crate::ui::wide("yo"), 0, Some((11, 1)))
        );
    }

    #[test]
    fn fix_h4_after_a_failed_send_the_next_send_is_judged_by_its_own_answer_and_never_goes_twice() {
        let mut r = mail_rig(|_| {});
        r.p.mail_recv(&inbox_reply(&[header(11, "Ann", "One")]));
        r.settle();
        r.tap(110.0, 28.0);
        writes(&mut r, "Bbo", "Hi", "yo");
        r.tap(30.0, 304.0);
        assert_eq!(sent().len(), 2, "one inbox request and the first send; {}", r.why());
        // the server: no such name
        r.p.mail_recv(&[1, 9]);
        r.settle();
        // the name is put right and Send is clicked: the window draws BEFORE the client has sent (the real frame order)
        r.tap(30.0, 58.0);
        r.key("backspace", "");
        r.key("backspace", "");
        r.key("backspace", "");
        r.type_text("Bob");
        r.key("escape", "");
        r.tap_then_draw_first(30.0, 304.0);
        assert_eq!(sent().len(), 3, "the second send went; {}", r.why());
        // the server takes it
        r.p.mail_recv(&[1, 1]);
        r.settle();
        // the form is empty and Send is off: another click sends nothing (the old code showed the first failure, kept
        // the form, and a second click sent the mail twice)
        r.tap(30.0, 304.0);
        assert_eq!(sent().len(), 3, "never a second send; {}", r.why());
    }

    #[test]
    fn fix_m5_a_send_the_client_refuses_leaves_send_ready_again() {
        let mut r = mail_rig(|_| {});
        r.p.mail_recv(&inbox_reply(&[header(11, "Ann", "One")]));
        r.settle();
        r.tap(110.0, 28.0);
        // a text of spaces: the client's own check refuses it
        writes(&mut r, "Bob", "Hi", "   ");
        r.tap(30.0, 304.0);
        let n = sent().len();
        // the text corrected: Send works at once (it stayed off before)
        r.tap(90.0, 304.0);
        writes(&mut r, "Bob", "Hi", "yo");
        r.tap(30.0, 304.0);
        assert_eq!(sent().len(), n + 1, "{}", r.why());
    }

    #[test]
    fn fix_m7_a_reply_to_a_mail_with_accented_letters_cuts_by_characters_and_the_window_stays() {
        let accents = "\u{e9}".repeat(31);
        let mut r = mail_rig(|_| {});
        r.p.mail_recv(&inbox_reply(&[header(12, "Bob", &accents)]));
        r.settle();
        r.tap(60.0, 46.0);
        r.p.mail_recv(&message_reply(12, "hello"));
        r.settle();
        r.tap(90.0, 304.0);
        r.frames(3);
        assert!(ws::any_open_of(&["mail_compose"]), "the Write tab is up and the window still draws: {}", r.why());
        r.tap(30.0, 86.0);
        r.type_text("\u{e9}");
        r.type_text("\u{e9}");
        r.frames(2);
        let logs = r.host.drain_logs().unwrap();
        assert!(logs.is_empty(), "no error from a cut letter: {logs:?}");
        assert!(ws::any_open_of(&["mail_compose"]));
    }

    #[test]
    fn fix_m8_a_right_click_ends_the_typing_and_lets_go_of_enter() {
        let mut r = mail_rig(|_| {});
        r.tap(110.0, 28.0);
        r.tap(30.0, 58.0);
        // another mod's claim on Enter is refused while the box is typed in (the control) and given after the click
        let other = Host::with_test_capabilities(&["eqoa.input.claim"]).unwrap();
        let held: bool = other.lua.load("local t, e = eqoa.input.claim('Enter', true); return t == nil").eval().unwrap();
        assert!(held, "control: the mail window holds Enter while a box is typed in");
        r.relayout();
        let (px, py) = (r.x0 + 200.0 * r.k, r.y0 + 150.0 * r.k);
        r.press_px(px, py, "right", false);
        let free: bool = other.lua.load("local t, e = eqoa.input.claim('Enter', true); return t ~= nil").eval().unwrap();
        assert!(free, "a right-click ended the typing: Enter is the game's again");
    }

    // ---- the Bag Viewer: from a closed bank to a sent sale, and a group, through its own window ----

    fn banker(p: &mut Play) {
        p.entities.slots[1] = Some(net::entity::Entity {
            id: 0x3001,
            world: p.player.world as i8,
            kind: net::entity::TYPE_CREATURE,
            pos: [0.0, 0.0, -3.0],
            actor_pos: [0.0, 0.0, -3.0],
            p18: 1,
            p19: 255,
            creature: Some(net::entity::Creature { name: "Banker".into(), p256: 1, npc_flags: 2, ..Default::default() }),
            ..Default::default()
        });
    }

    fn bag_rig() -> Rig {
        let mut r = rig_with("eqoa_bag_viewer", 318.0, 300.0, "shift+b", "b", |p| {
            banker(p);
            p.dump.bank.items = net::dump::ItemList {
                p04: 0,
                items: vec![item(700_001, "Gold Ring", 21, 1), item(700_002, "Silver Ring", 22, 1)],
            };
            p.dump.bank.items.items[1].item.value = 80;
        });
        r.settle();
        r.host.execute("local B = require('bags'); B.select(6)").unwrap();
        for _ in 0..6 {
            r.settle();
        }
        quiet_bank_tab(&mut r);
        let diag = state_of(&r, "local B = require('bags'); return 'open=' .. tostring(B.open) .. ' grid=' .. tostring(B.grid) .. ' tab=' .. tostring(B.tab) .. ' rect=' .. tostring(B.rect) .. ' inv=' .. tostring(eqoa.inventory.get())");
        assert!(state_of(&r, "return tostring(require('bags').rect ~= nil)") == "true", "the Bags window is drawn: {diag}; {}", r.why());
        r
    }

    /// The Bags window's own slot `n` (1-based, first row) in window pixels.
    fn bag_slot(r: &Rig, n: usize) -> (f64, f64) {
        bv_point(r, &format!("local rc = require('bags').rect; return rc.x + (8 + {} * 38 + 18) * rc.k, rc.y + (46 + 18) * rc.k", n - 1))
    }

    fn menu_row(r: &Rig, label: &str) -> (f64, f64) {
        bv_point(r, &format!("for _, h in ipairs(require('bags').menu_hits) do if h.row.label == '{label}' then return h.x + h.w / 2, h.y + h.h / 2 end end error('no menu row {label}: ' .. #require('bags').menu_hits)"))
    }

    fn form_hit(r: &Rig, label: &str) -> (f64, f64) {
        bv_point(r, &format!("for _, h in ipairs(require('auction_list').hits) do if h.label == '{label}' then return h.x + h.w / 2, h.y + h.h / 2 end end error('no form control {label}')"))
    }

    fn state_of(r: &Rig, expr: &str) -> String {
        r.host.lua.load(expr).eval::<String>().unwrap_or_else(|e| format!("({e})"))
    }

    #[test]
    fn fix_h2_listing_opens_the_bank_itself_from_a_closed_bank_and_ends_in_the_games_sale() {
        let mut r = bag_rig();
        let (x, y) = bag_slot(&r, 1);
        r.press_px(x, y, "left", false);
        let (mx, my) = menu_row(&r, "List on auction...");
        // the row is enabled with a banker near and the bank closed (it was greyed before)
        assert_eq!(state_of(&r, "for _, h in ipairs(require('bags').menu_hits) do if h.row.label == 'List on auction...' then return tostring(h.row.enabled) end end"), "true");
        r.press_px(mx, my, "left", false);
        // the client talks to the banker; the server's reply opens the bank hidden for the Bag Viewer
        r.settle();
        r.p.bank_opcode(0x124D, &[]);
        // the game steps, as it does all the time: the form asks the client for what can be put up
        for _ in 0..6 {
            r.settle();
        }
        assert_eq!(state_of(&r, "return tostring(require('auction_list').form and require('auction_list').form.mode)"), "form", "the form is up; {}", r.why());
        // the lowest price is filled in and shown; List it asks the client's question with it
        let n = sent().len();
        let (lx, ly) = form_hit(&r, "List it");
        r.press_px(lx, ly, "left", false);
        r.settle();
        assert_eq!(sent().len(), n, "the form alone lists nothing; {}", r.why());
        let q = confirm::current_for_test().expect("the client's question is up");
        assert!(q.text.contains("Gold Ring") && q.text.contains("from 50 tunar"), "{}", q.text);
        say_yes();
        r.settle();
        assert_eq!(sent().len(), n + 1, "{}", r.why());
        assert_eq!(sent().last().unwrap(), &Auction::sell(0x3001, 21, 1, 50, 100, 720), "the banker the bank was opened with, the row's key");
    }

    #[test]
    fn fix_m4_the_price_is_typed_and_the_games_limits_hold_in_the_form() {
        let mut r = bag_rig();
        let (x, y) = bag_slot(&r, 1);
        r.press_px(x, y, "left", false);
        let (mx, my) = menu_row(&r, "List on auction...");
        r.press_px(mx, my, "left", false);
        r.settle();
        r.p.bank_opcode(0x124D, &[]);
        // the game steps, as it does all the time: the form asks the client for what can be put up
        for _ in 0..6 {
            r.settle();
        }
        // a price under the item's value is not sent: the form says so
        let (px, py) = form_hit(&r, "price");
        r.press_px(px, py, "left", false);
        for _ in 0..3 {
            r.key("backspace", "");
        }
        r.type_text("49");
        r.key("escape", "");
        let n = sent().len();
        let (lx, ly) = form_hit(&r, "List it");
        r.press_px(lx, ly, "left", false);
        assert!(!confirm::is_open() && sent().len() == n, "under the lowest price: nothing asked");
        assert!(state_of(&r, "return require('auction_list').form.msg or ''").contains("lowest starting price"));
        // a price and a time within the limits go
        let (px, py) = form_hit(&r, "price");
        r.press_px(px, py, "left", false);
        r.key("backspace", "");
        r.key("backspace", "");
        r.type_text("120");
        r.key("escape", "");
        let (bx, by) = form_hit(&r, "buyout");
        r.press_px(bx, by, "left", false);
        for _ in 0..4 {
            r.key("backspace", "");
        }
        r.type_text("400");
        r.key("escape", "");
        let (hx, hy) = form_hit(&r, "hours48");
        r.press_px(hx, hy, "left", false);
        let (lx, ly) = form_hit(&r, "List it");
        r.press_px(lx, ly, "left", false);
        let q = confirm::current_for_test().expect("the question");
        assert!(q.text.contains("from 120 tunar") && q.text.contains("buyout 400") && q.text.contains("48 hours"), "{}", q.text);
        say_yes();
        r.settle();
        assert_eq!(sent().last().unwrap(), &Auction::sell(0x3001, 21, 1, 120, 400, 2880));
    }

    #[test]
    fn fix_h3_a_group_opens_its_own_session_asks_each_item_in_turn_and_runs_to_the_end() {
        let mut r = bag_rig();
        // both rings selected (Ctrl+click), then the group menu from a right-click on one
        let (x1, y1) = bag_slot(&r, 1);
        let (x2, y2) = bag_slot(&r, 2);
        r.press_px(x1, y1, "left", true);
        r.press_px(x2, y2, "left", true);
        r.press_px(x1, y1, "right", false);
        let (mx, my) = menu_row(&r, "List on auction (2)");
        r.press_px(mx, my, "left", false);
        r.settle();
        r.p.bank_opcode(0x124D, &[]);
        // the game steps, as it does all the time: the form asks the client for what can be put up
        for _ in 0..6 {
            r.settle();
        }
        assert_eq!(state_of(&r, "return tostring(require('auction_list').form and require('auction_list').form.mode)"), "form", "the first form; {}", r.why());
        assert!(ws::any_open_of(&["auction_search"]), "the group opened the session itself");
        let n = sent().len();
        let (lx, ly) = form_hit(&r, "List it");
        r.press_px(lx, ly, "left", false);
        let q = confirm::current_for_test().expect("the first question");
        assert!(q.text.contains("Gold Ring") && q.text.contains("from 50 tunar"), "{}", q.text);
        // the Yes lands on a frame that is not a game step (the client carries it out on every drawn frame)
        say_yes();
        crate::play::fork_hooks::begin_iteration(&mut r.p, false);
        r.frames(1);
        assert_eq!(sent().len(), n + 1, "the first sale went");
        // the server answers, the second form comes with its own lowest price; nothing is silently listed
        r.p.bank.state = 0;
        for _ in 0..40 {
            r.settle();
        }
        assert_eq!(state_of(&r, "return tostring(require('auction_list').form and require('auction_list').form.mode)"), "form", "the second form; {}", r.why());
        assert!(state_of(&r, "return require('auction_list').form.price").contains("80"), "the second item's own lowest price");
        assert_eq!(sent().len(), n + 1, "nothing for the second until the player says so");
        let (sx, sy) = form_hit(&r, "Skip");
        r.press_px(sx, sy, "left", false);
        r.settle();
        assert!(state_of(&r, "return require('bags').result and require('bags').result.text or ''").contains("1 of 2 put up"), "{}", state_of(&r, "return require('bags').result and require('bags').result.text or ''"));
        assert!(!confirm::is_open());
    }

    #[test]
    fn fix_h3_a_refused_sale_says_why_and_the_group_goes_on_instead_of_hanging() {
        let mut r = bag_rig();
        let (x1, y1) = bag_slot(&r, 1);
        let (x2, y2) = bag_slot(&r, 2);
        r.press_px(x1, y1, "left", true);
        r.press_px(x2, y2, "left", true);
        r.press_px(x1, y1, "right", false);
        let (mx, my) = menu_row(&r, "List on auction (2)");
        r.press_px(mx, my, "left", false);
        r.settle();
        r.p.bank_opcode(0x124D, &[]);
        // the game steps, as it does all the time: the form asks the client for what can be put up
        for _ in 0..6 {
            r.settle();
        }
        // a question of another mod is already up: the sale is refused busy; the form stays with its reason
        r.p.bank.state = 3;
        let (lx, ly) = form_hit(&r, "List it");
        r.press_px(lx, ly, "left", false);
        r.settle();
        assert!(!confirm::is_open(), "refused: no question");
        assert!(state_of(&r, "return require('auction_list').form.msg or ''").contains("Still waiting"), "{}", state_of(&r, "return require('auction_list').form.msg or ''"));
        // the wait ends and the same form goes again
        r.p.bank.state = 0;
        r.settle();
        let (lx, ly) = form_hit(&r, "List it");
        r.press_px(lx, ly, "left", false);
        assert!(confirm::is_open(), "the same item asks now");
    }

    // ---- the recheck fix round (N1, N3, N4, N5) ----

    fn taken_bank(r: &Rig) -> String {
        state_of(r, "return tostring(eqoa.windows.taken('bank'))")
    }

    /// List on auction... of the first bank row, up to the client's talk to the banker (not yet answered).
    fn click_list_on_auction(r: &mut Rig) {
        let (x, y) = bag_slot(r, 1);
        r.press_px(x, y, "left", false);
        let (mx, my) = menu_row(r, "List on auction...");
        r.press_px(mx, my, "left", false);
        r.settle();
    }

    #[test]
    fn fix_n1_a_bank_open_that_is_refused_or_stopped_leaves_nothing_held_and_listing_works_again() {
        let mut r = bag_rig();
        click_list_on_auction(&mut r);
        assert_eq!(state_of(&r, "return tostring(require('bags').pending_open ~= nil)"), "true", "the open is waiting; {}", r.why());
        assert_eq!(taken_bank(&r), "true", "the bank is taken for the open");
        // the server refuses the open (the same call the client's result takes)
        r.host.execute("local B = require('bags'); B.on_action_result({request_id = B.pending_open.req, ok = false, reason = 'too far', kind = 'talk'})").unwrap();
        r.settle();
        assert_eq!(state_of(&r, "return tostring(require('auction_list').busy())"), "false", "no run is left standing; {}", r.why());
        assert_eq!(taken_bank(&r), "false", "the bank is let go");
        // listing works again: the same click opens the bank once more
        click_list_on_auction(&mut r);
        assert_eq!(state_of(&r, "return tostring(require('bags').pending_open ~= nil)"), "true", "a new open; {}", r.why());
        // Stop while it waits
        r.host.execute("require('bags').stop_batch()").unwrap();
        r.settle();
        assert_eq!(state_of(&r, "return tostring(require('auction_list').busy())"), "false", "{}", r.why());
        assert_eq!(taken_bank(&r), "false", "stopped: the bank is let go");
        click_list_on_auction(&mut r);
        assert_eq!(state_of(&r, "return tostring(require('bags').pending_open ~= nil)"), "true", "and again; {}", r.why());
    }

    #[test]
    fn fix_n3_a_tab_change_during_a_listing_still_lets_the_bank_go_when_it_ends() {
        let mut r = bag_rig();
        click_list_on_auction(&mut r);
        r.p.bank_opcode(0x124D, &[]);
        for _ in 0..6 {
            r.settle();
        }
        let (lx, ly) = form_hit(&r, "List it");
        r.press_px(lx, ly, "left", false);
        assert!(confirm::is_open(), "the question; {}", r.why());
        say_yes();
        r.settle();
        // the player goes to another tab while the sale settles: the release is dropped for now
        r.host.execute("require('bags').select(1)").unwrap();
        r.settle();
        assert_eq!(taken_bank(&r), "true", "held while the sale settles");
        r.p.bank.state = 0;
        for _ in 0..40 {
            r.settle();
        }
        assert_eq!(state_of(&r, "return tostring(require('auction_list').busy())"), "false", "{}", r.why());
        assert_eq!(taken_bank(&r), "false", "the listing ended: the hidden bank is let go; {}", r.why());
    }

    #[test]
    fn fix_n5_list_it_refuses_a_row_that_is_no_longer_the_one_shown() {
        let mut r = bag_rig();
        click_list_on_auction(&mut r);
        r.p.bank_opcode(0x124D, &[]);
        for _ in 0..6 {
            r.settle();
        }
        assert_eq!(state_of(&r, "return tostring(require('auction_list').form and require('auction_list').form.mode)"), "form", "{}", r.why());
        // the bank row the form was opened for is now another item (same place)
        r.p.dump.bank.items.items[0] = item(700_009, "Copper Ring", 21, 1);
        for _ in 0..4 {
            r.settle();
        }
        let n = sent().len();
        let (lx, ly) = form_hit(&r, "List it");
        r.press_px(lx, ly, "left", false);
        r.settle();
        assert!(!confirm::is_open(), "no question for another item; {}", r.why());
        assert_eq!(sent().len(), n);
        let msg = state_of(&r, "return require('auction_list').form.msg or ''");
        assert!(msg.contains("changed"), "a plain message: {msg}");
        assert_eq!(state_of(&r, "return require('auction_list').form.mode"), "form", "the form stays for Cancel");
    }

    #[test]
    fn fix_n4_closing_mail_before_the_answer_never_leaves_the_sent_mail_to_go_twice() {
        let mut r = mail_rig(|_| {});
        r.p.mail_recv(&inbox_reply(&[header(11, "Ann", "One")]));
        r.settle();
        r.tap(110.0, 28.0);
        writes(&mut r, "Bob", "Hi", "yo");
        let n = sent().len();
        r.tap(30.0, 304.0);
        assert_eq!(sent().len(), n + 1, "the mail went; {}", r.why());
        // Escape closes the window before the answer; the server then takes the mail
        r.key("escape", "");
        r.p.mail_recv(&[1, 1]);
        r.settle();
        // reopened: the draft is not the sent mail, and a click on Send sends nothing
        CORE.with(|c| {
            c.borrow_mut().emit("input.key", json!({"key": "m", "combo": "shift+m", "down": true, "repeat": false, "synthetic": false, "text": ""}))
        });
        native_events::dispatch(&r.host).unwrap();
        r.settle();
        // (opening asks for the inbox again: count from here)
        let m = sent().len();
        r.p.mail_recv(&inbox_reply(&[header(11, "Ann", "One")]));
        r.settle();
        r.tap(110.0, 28.0);
        r.tap(30.0, 304.0);
        assert_eq!(sent().len(), m, "never a second send of the same mail; {}", r.why());
    }

    #[test]
    fn fix_n4_text_typed_while_a_send_is_pending_is_kept_when_the_mail_goes() {
        let mut r = mail_rig(|_| {});
        r.p.mail_recv(&inbox_reply(&[header(11, "Ann", "One")]));
        r.settle();
        r.tap(110.0, 28.0);
        writes(&mut r, "Bob", "Hi", "yo");
        let n = sent().len();
        r.tap(30.0, 304.0);
        assert_eq!(sent().len(), n + 1, "the first mail went; {}", r.why());
        // the name is changed while the first waits for its answer
        r.tap(30.0, 58.0);
        r.key("backspace", "");
        r.key("backspace", "");
        r.key("backspace", "");
        r.type_text("Cat");
        r.key("escape", "");
        r.p.mail_recv(&[1, 1]);
        r.settle();
        // the form still holds the new mail (the old code emptied it): Send now sends it
        r.tap(30.0, 304.0);
        assert_eq!(sent().len(), n + 2, "the text typed meanwhile was kept; {}", r.why());
        let last = sent().last().unwrap().clone();
        assert!(last.windows(3).any(|w| w == b"Cat"), "the second mail is to Cat: {last:?}");
    }

    // ---- owner's 2026-11-25 round: bank tunar in the Bag Viewer, class and race in the Auction, the header columns ----

    /// A control of the Auction window, by label (window pixels).
    fn au_hit(r: &Rig, label: &str) -> (f64, f64) {
        bv_point(r, &format!("for _, h in ipairs(require('eqoa_auction_state').hits) do if h.label == '{label}' then return h.x + h.w / 2, h.y + h.h / 2 end end error('no control {label}')"))
    }

    fn au_query(kind: u8, p9c: u32, pa0: u32, pb4: i32) -> Query {
        Query {
            kind,
            text: String::new(),
            name: String::new(),
            p9c,
            pa0,
            pa4: 1,
            pa8: crate::s2::get().auction_max_level,
            pac: 0,
            pb0: 0x7FFF_FFFF,
            pb4,
            pb8: -1,
            pbc: true,
        }
    }

    #[test]
    fn owner1125_auction_class_race_and_type_pickers_put_their_values_in_the_search() {
        // (the picker's button, the row to pick (None: the first wear slot), what the game's own form would send)
        for case in 0..3 {
            let mut r = auction_rig_mod();
            let mut want = r.p.bank.auction.clone();
            let (button, pick, query) = match case {
                0 => ("Class: Any class", "pick Cleric".to_string(), au_query(0, 1 << 9, 0xFFFF_FFFF, -2)),
                1 => ("Race: Any race", "pick Dwarf".to_string(), au_query(0, 0xFFFF_FFFF, 1 << 4, -2)),
                _ => {
                    let (x, y) = au_hit(&r, "Type: Any type");
                    r.press_px(x, y, "left", false);
                    let first = state_of(&r, "return require('eqoa_auction_state').opts[2].label");
                    r.press_px(x, y, "left", false); // the same button again closes the list
                    ("Type: Any type", format!("pick {first}"), au_query(2, 0xFFFF_FFFF, 0xFFFF_FFFF, 1))
                }
            };
            let (x, y) = au_hit(&r, button);
            r.press_px(x, y, "left", false);
            if case == 0 {
                let labels = state_of(&r, "local t = {} for _, o in ipairs(require('eqoa_auction_state').opts) do t[#t + 1] = o.label end return table.concat(t, ',')");
                assert!(labels.starts_with("Any class,"), "{labels}");
                for name in ["Alchemist", "Bard", "Cleric", "Warrior", "Wizard"] {
                    assert!(labels.contains(name), "{name} in {labels}");
                }
                assert_eq!(labels.split(',').filter(|l| !l.starts_with("Any") && !l.starts_with("My")).count(), 15, "{labels}");
            }
            let (x, y) = au_hit(&r, &pick);
            r.press_px(x, y, "left", false);
            let (x, y) = au_hit(&r, "Search");
            r.press_px(x, y, "left", false);
            let expected = want.search(query);
            assert_eq!(sent(), vec![expected], "case {case} ({button} -> {pick}): the game form's own bytes; {}", r.why());
        }
    }

    #[test]
    fn owner1125_auction_header_columns_fit_their_titles_and_values_on_every_tab() {
        let mut r = auction_rig_mod();
        // the tabs: Search is the first, My lots and My bids the others (their own clicks)
        for (tab, x) in [("search", 28.0), ("lots", 112.0), ("bids", 196.0)] {
            r.tap(x, 28.0);
            r.frames(2);
            let ok = state_of(
                &r,
                &format!(
                    "local L = require('eqoa_auction_state').layout['{tab}']; if not L then return 'no layout' end; \
                     for i, c in ipairs(L) do \
                       if c.w + 1e-6 < c.title_w or c.w + 1e-6 < c.value_w then return 'column ' .. i .. ' narrower than its text' end \
                       if L[i + 1] and c.x + c.w > L[i + 1].x then return 'column ' .. i .. ' runs into the next' end \
                     end; \
                     if L.name_w < 40 then return 'no room for the name' end; return 'ok'"
                ),
            );
            assert_eq!(ok, "ok", "{tab}: {}", r.why());
        }
        // the Search tab's own title that ran into the next one: wider than the 60 pixels it used to get
        r.tap(28.0, 28.0);
        r.frames(2);
        assert_eq!(state_of(&r, "return tostring(require('eqoa_auction_state').layout.search[2].w > 60)"), "true");
    }

    /// Owner 2026-11-26: the Search tab has Seller and Price (from, to) boxes that reach the search, typed in and released
    /// as the Name and Level boxes are (Tab moves on, Enter searches, a click elsewhere ends the typing).
    #[test]
    fn owner1126_auction_seller_and_price_boxes_reach_the_search() {
        // what the action itself sends for the same fields, from a fresh rig (the same page number)
        let (mut p, host) = super::auction_rig();
        let (ok, code, reason) = act(&mut p, &host, "return eqoa.actions.auction_search({name = 'Iron', seller = 'Zed', min_price = 100, max_price = 900})");
        assert!(ok, "{code} {reason}");
        let want = sent().last().cloned().unwrap();
        let (mut p2, host2) = super::auction_rig();
        let (ok, code, reason) = act(&mut p2, &host2, "return eqoa.actions.auction_search({name = 'Iron'})");
        assert!(ok, "{code} {reason}");
        assert_ne!(sent().last().unwrap(), &want, "control: the seller and the prices change the bytes");
        let mut r = auction_rig_mod();
        let n = sent().len();
        let (x, y) = au_hit(&r, "box name");
        r.press_px(x, y, "left", false);
        r.type_text("Iron");
        r.key("tab", "");
        r.type_text("Zed");
        assert_eq!(state_of(&r, "return require('eqoa_auction_state').field"), "seller", "Tab went Name -> Seller");
        let (x, y) = au_hit(&r, "box min_price");
        r.press_px(x, y, "left", false);
        r.type_text("10a0"); // digits only
        r.key("tab", "");
        r.type_text("900");
        assert_eq!(state_of(&r, "local s = require('eqoa_auction_state'); return s.seller .. '|' .. s.min_price .. '|' .. s.max_price"), "Zed|100|900");
        r.key("enter", "");
        assert_eq!(sent().len(), n + 1, "Enter in a box searches; {}", r.why());
        assert_eq!(sent().last().unwrap(), &want, "the seller and the price range reach the game's search");
        assert_eq!(state_of(&r, "return tostring(require('eqoa_auction_state').field)"), "nil", "the typing is released");
    }

    /// Clicking a column title orders the rows shown, ascending then descending; a click on a row acts on the lot shown there;
    /// the line under the list says the order is within this page; the header still fits at the window's width.
    #[test]
    fn owner1126_auction_columns_order_the_rows_shown_and_a_click_acts_on_the_lot_shown() {
        let mut r = auction_rig_mod();
        let (x, y) = au_hit(&r, "Search");
        r.press_px(x, y, "left", false);
        let page = r.p.bank.auction.page as i64;
        r.p.bank_opcode(0x1206, &page_reply(page, &[(501, 1, 400), (502, 1, 100), (503, 1, 900), (504, 1, 0)]));
        r.settle();
        let shown = |r: &Rig| state_of(r, "local t = {} for _, id in ipairs(require('eqoa_auction_state').shown) do t[#t + 1] = id end return table.concat(t, ',')");
        assert_eq!(shown(&r), "501,502,503,504", "the server's order until a title is clicked");
        let (x, y) = au_hit(&r, "order buy");
        r.press_px(x, y, "left", false);
        assert_eq!(shown(&r), "502,501,503,504", "Buy now ascending, a lot with no price last");
        let (x, y) = au_hit(&r, "order buy");
        r.press_px(x, y, "left", false);
        assert_eq!(shown(&r), "503,501,502,504", "Buy now descending, still last");
        // the mark on the active title is measured into its column; the header still fits at this width
        let ok = state_of(
            &r,
            "local L = require('eqoa_auction_state').layout.search;              for i, c in ipairs(L) do                if c.w + 1e-6 < c.title_w or c.w + 1e-6 < c.value_w then return 'column ' .. i .. ' narrower than its text' end                if L[i + 1] and c.x + c.w > L[i + 1].x then return 'column ' .. i .. ' runs into the next' end              end;              if L.name_w < 40 then return 'no room for the name' end; return 'ok'",
        );
        assert_eq!(ok, "ok", "{}", r.why());
        // a click on the first row shown opens THAT lot (503 first), not the lot that was there by position (501)
        let (x, y) = bv_point(&r, "for _, h in ipairs(require('eqoa_auction_state').row_hits) do if h.label == 'lot 503' then return h.x + h.w / 2, h.y + h.h / 2 end end error('no row lot 503')");
        r.press_px(x, y, "left", false);
        assert_eq!(state_of(&r, "return tostring(require('eqoa_auction_state').sel)"), "503");
        // another title starts ascending again; Time left orders by minutes
        let (x, y) = au_hit(&r, "order high");
        r.press_px(x, y, "left", false);
        assert_eq!(state_of(&r, "return require('eqoa_auction_state').sort.key .. require('eqoa_auction_state').sort.dir"), "high1");
    }

    fn bm_hit(r: &Rig, label: &str) -> (f64, f64) {
        bv_point(r, &format!("for _, h in ipairs(require('bags').action_hits) do if h.label == '{label}' then return h.x + h.w / 2, h.y + h.h / 2 end end error('no action {label}')"))
    }

    fn bm_text(r: &Rig) -> String {
        state_of(r, "return require('bank_money_state').text")
    }

    fn bm_result(r: &Rig) -> String {
        state_of(r, "local x = require('bags').result; return x and x.text or ''")
    }

    fn bm_amounts(r: &Rig) -> String {
        state_of(r, "return eqoa.inventory.get().money .. '/' .. eqoa.bank.get().money")
    }

    fn bm_calls(r: &Rig) -> String {
        state_of(r, "return table.concat(bm_calls, ',')")
    }

    /// The Bag Viewer on its Bank tab with a banker near, carrying 1000 tunar and banking 500; `open`: the bank is open.
    fn bank_money_rig(open: bool) -> Rig {
        let mut r = rig_with("eqoa_bag_viewer", 318.0, 300.0, "shift+b", "b", |p| {
            banker(p);
            p.dump.player.p34 = 1000;
            p.dump.player.p38 = 500;
            p.dump.bank.items = net::dump::ItemList { p04: 0, items: vec![item(700_001, "Gold Ring", 21, 1)] };
        });
        r.settle();
        r.host.execute("require('bags').select(6)").unwrap();
        for _ in 0..6 {
            r.settle();
        }
        if !open {
            quiet_bank_tab(&mut r);
        }
        if open {
            bank_money_open(&mut r);
        }
        r.host
            .execute(
                "bm_calls = {}; local B = require('bags'); local real = B.run_single; \
                 B.run_single = function(label, fn) bm_calls[#bm_calls + 1] = label; return real(label, fn) end",
            )
            .unwrap();
        r
    }

    /// The Bank tab now asks the banker itself when it is shown (no "Open bank" button). The tests that start from a closed
    /// bank (to test what opens it) let the session go and say the tab has asked already.
    fn quiet_bank_tab(r: &mut Rig) {
        r.host
            .execute("eqoa.windows.release('bank'); local H = require('helpers'); H.bank_near_seen, H.bank_refresh_wanted, H.bank_was_open = true, false, nil")
            .unwrap();
        for _ in 0..3 {
            r.settle();
        }
    }

    fn bank_money_open(r: &mut Rig) {
        // the tab asks the banker by itself when it is shown: the reply opens the mod's hidden bank
        r.settle();
        r.p.bank_opcode(0x124D, &[]);
        for _ in 0..6 {
            r.settle();
        }
        assert_eq!(state_of(r, "return tostring(eqoa.bank.get().open)"), "true", "the bank is open; {}", r.why());
    }

    fn bm_type(r: &mut Rig, text: &str) {
        let (x, y) = bm_hit(r, "Tunar amount");
        r.press_px(x, y, "left", false);
        for _ in 0..12 {
            r.key("backspace", "");
        }
        r.type_text(text);
    }

    #[test]
    fn owner1125_bank_money_deposit_and_withdraw_send_the_typed_amount_and_refuse_too_much() {
        let mut r = bank_money_rig(true);
        assert_eq!(bm_amounts(&r), "1000/500", "carried and banked, from the game's own state");
        // an empty box: nothing is sent, the reason is shown
        let (x, y) = bm_hit(&r, "Deposit tunar");
        r.press_px(x, y, "left", false);
        assert_eq!(bm_calls(&r), "", "an empty amount sends nothing");
        assert!(bm_result(&r).contains("type an amount"), "{}", bm_result(&r));
        // digits only
        bm_type(&mut r, "1a2");
        assert_eq!(bm_text(&r), "12", "letters are not taken");
        // more than is carried: refused with the reason, nothing sent
        bm_type(&mut r, "2000");
        let (x, y) = bm_hit(&r, "Deposit tunar");
        r.press_px(x, y, "left", false);
        assert_eq!(bm_calls(&r), "", "more than carried sends nothing");
        assert!(bm_result(&r).contains("you carry only 1,000"), "{}", bm_result(&r));
        let (x, y) = bm_hit(&r, "Withdraw tunar");
        r.press_px(x, y, "left", false);
        assert_eq!(bm_calls(&r), "", "more than banked sends nothing");
        assert!(bm_result(&r).contains("the bank holds only 500"), "{}", bm_result(&r));
        // 250: the deposit goes, with that amount
        bm_type(&mut r, "250");
        let (x, y) = bm_hit(&r, "Deposit tunar");
        r.press_px(x, y, "left", false);
        assert_eq!(bm_calls(&r), "Deposit 250 tunar", "{}", r.why());
        // the API took the call (a refused one leaves no request); the 0x1255 bytes themselves are items_tests' own check
        assert_eq!(state_of(&r, "return tostring(require('bags').last_req ~= nil)"), "true", "the request was accepted; {}", r.why());
        // the server's answer: the bank now holds 750 and the pack 750 (the game's own money events)
        let mut w = Writer::default();
        w.signed(750);
        r.p.bank_opcode(0x1253, &w.data);
        r.p.bags_tunar(750);
        for _ in 0..3 {
            r.settle();
        }
        assert_eq!(bm_amounts(&r), "750/750");
        assert!(bm_result(&r).contains("Deposit 250 tunar"), "{}", bm_result(&r));
        // the bank may have closed with the answer: open it again if so, then take everything out with All
        if state_of(&r, "return tostring(eqoa.bank.get().open)") != "true" {
            bank_money_open(&mut r);
        }
        let (x, y) = bm_hit(&r, "Withdraw all tunar");
        r.press_px(x, y, "left", false);
        assert_eq!(bm_calls(&r), "Deposit 250 tunar,Withdraw 750 tunar", "All withdraws the whole bank balance; {}", r.why());
    }

    #[test]
    fn owner1125b_bank_money_works_near_a_banker_with_no_window_open() {
        let mut r = bank_money_rig(false);
        assert_eq!(state_of(&r, "return tostring(eqoa.bank.get().open)"), "false", "no bank window is open");
        // a banker near: Deposit opens the mod's own hidden bank itself, then moves the tunar
        bm_type(&mut r, "10");
        let (x, y) = bm_hit(&r, "Deposit tunar");
        r.press_px(x, y, "left", false);
        assert_eq!(state_of(&r, "return tostring(require('bags').pending_open ~= nil)"), "true", "the bank is being opened; {}", r.why());
        assert_eq!(bm_calls(&r), "", "nothing is sent before the bank answers");
        r.p.bank_opcode(0x124D, &[]);
        for _ in 0..6 {
            r.settle();
        }
        assert_eq!(bm_calls(&r), "Deposit 10 tunar", "the deposit went once the hidden bank opened; {}", r.why());
        assert!(!bm_result(&r).contains("not open"), "{}", bm_result(&r));
    }

    #[test]
    fn owner1125b_bank_money_too_far_from_the_banker_is_refused_with_the_reason() {
        let mut r = bank_money_rig(false);
        r.host.execute("require('bags').range.bank = {fail = 0, misses = {0, 0}}").unwrap();
        bm_type(&mut r, "10");
        let (x, y) = bm_hit(&r, "Deposit tunar");
        r.press_px(x, y, "left", false);
        assert_eq!(bm_calls(&r), "");
        assert!(bm_result(&r).contains("too far from the banker"), "{}", bm_result(&r));
        assert_eq!(state_of(&r, "return tostring(require('bags').pending_open ~= nil)"), "false");
    }

    #[test]
    fn owner1125_the_amount_box_takes_the_keyboard_only_while_typed_in() {
        let mut r = bank_money_rig(true);
        let other = Host::with_test_capabilities(&["eqoa.input.claim"]).unwrap();
        let free = |o: &Host| -> bool { o.lua.load("local t = eqoa.input.claim('Enter', true); if t then eqoa.input.release(t) end; return t ~= nil").eval().unwrap() };
        assert!(free(&other), "control: Enter is free before the box is used");
        let (x, y) = bm_hit(&r, "Tunar amount");
        r.press_px(x, y, "left", false);
        r.frames(1);
        assert!(!free(&other), "Enter is the Bag Viewer's while the amount is typed in");
        r.key("escape", "");
        assert!(free(&other), "Escape ended the typing: Enter is the game's again");
        r.press_px(x, y, "left", false);
        assert!(!free(&other));
        // a click elsewhere in the window ends it too
        let (gx, gy) = bag_slot(&r, 2);
        r.press_px(gx, gy, "left", false);
        assert!(free(&other), "a click elsewhere ended the typing");
    }
}
