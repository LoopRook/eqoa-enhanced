// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A10: mail and the auction for a mod without a running game: the capability gates, the argument checks, the
//! three window kinds, the Mods & Addons lines, the questions' texts, and the records the reads return. The
//! `Play`-driven parts (the requests, the questions' Yes, the one pending request, the replies read for the mod's
//! window) are in `fork_ext/a10_play_tests.rs`.
//!
//! Each test names its negative control: what it would see if the change were not there.
use super::{
    action_broker, confirm,
    host::Host,
    native_a10,
    native_core::{self, CORE, Core},
    window_sessions as ws, window_takeover,
};
use serde_json::json;

fn fresh() {
    CORE.with(|c| *c.borrow_mut() = Core::default());
    native_core::fixture_demand(true);
    action_broker::reset_all_for_test();
    ws::reset_for_test();
    window_takeover::reset_all_for_test();
    window_takeover::forget_switch_for_test();
    native_a10::clear_mail();
    native_a10::clear_auction();
    CORE.with(|c| c.borrow_mut().view.in_world = true);
}

fn host(caps: &[&str]) -> Host {
    Host::with_test_capabilities(caps).unwrap()
}

fn caps(c: &[&str]) -> Vec<String> {
    c.iter().map(|s| s.to_string()).collect()
}

#[test]
fn a10_every_name_needs_its_own_declaration_and_a_mod_without_it_gets_nothing() {
    fresh();
    native_a10::publish_mail(json!({"revision": 0, "status": "idle", "entries": []}));
    native_a10::publish_auction(json!({"revision": 0, "pending": "none", "results": []}));
    let bare = host(&["eqoa.events.on"]);
    for read in ["mail", "auction"] {
        bare.execute(&format!("local c, e = eqoa.{read}.get(); assert(c == nil and (e.code == 'not_declared' or e.code == 'permission_denied'), e and e.code)")).unwrap();
    }
    for name in native_a10::NAMES {
        bare.execute(&format!(
            "local r, e = eqoa.actions.{name}({{lot = 1, id = 1, slot = 1}}); assert(r == nil and e ~= nil and (e.code == 'not_declared' or e.code == 'permission_denied'), '{name}: ' .. tostring(e and e.code))"
        ))
        .unwrap();
    }
    // the control: declared, the read and the action are accepted
    let granted = host(&["eqoa.mail.get", "eqoa.auction.get", "eqoa.actions.mail_refresh"]);
    granted.execute("local c = assert(eqoa.mail.get()); assert(c.status == 'idle')").unwrap();
    granted.execute("local c = assert(eqoa.auction.get()); assert(c.pending == 'none')").unwrap();
    granted.execute("local r, e = eqoa.actions.mail_refresh(); assert(r ~= nil and e == nil)").unwrap();
    // one action's grant is not another's
    granted.execute("local r, e = eqoa.actions.mail_delete(3); assert(r == nil and e.code == 'permission_denied', e and e.code)").unwrap();
    // the wildcard grants every action but not the reads
    let wild = host(&["eqoa.actions.*"]);
    wild.execute("assert(eqoa.actions.auction_my_lots() ~= nil)").unwrap();
    wild.execute("local c, e = eqoa.mail.get(); assert(c == nil and e.code == 'permission_denied')").unwrap();
}

#[test]
fn a10_arguments_are_checked_before_anything_is_queued() {
    fresh();
    let h = host(&["eqoa.actions.*"]);
    for (call, code) in [
        ("eqoa.actions.mail_read('a')", "invalid_argument"),
        ("eqoa.actions.mail_read(-1)", "invalid_argument"),
        ("eqoa.actions.mail_read(1.5)", "invalid_argument"),
        ("eqoa.actions.mail_delete(nil)", "invalid_argument"),
        ("eqoa.actions.mail_take(2^40)", "invalid_argument"),
        ("eqoa.actions.mail_read(4294967296)", "invalid_argument"),
        ("eqoa.actions.mail_send({to = 'Bob', subject = 'a\\nb', body = 'b'})", "invalid_argument"),
        ("eqoa.actions.mail_refresh(1)", "invalid_argument"),
        ("eqoa.actions.mail_send('Bob')", "invalid_argument"),
        ("eqoa.actions.mail_send({to = 5})", "invalid_argument"),
        ("eqoa.actions.mail_send({to = 'Bob', subject = string.rep('x', 40), body = 'b'})", "out_of_range"),
        ("eqoa.actions.mail_send({to = 'Bob', subject = 's', body = string.rep('x', 600)})", "out_of_range"),
        ("eqoa.actions.mail_send({to = 'Bob', subject = 's', body = 'b', tunar = -5})", "out_of_range"),
        ("eqoa.actions.mail_send({to = 'Bob', subject = 's', body = 'b', item = {slot = 0, id = 1}})", "out_of_range"),
        ("eqoa.actions.mail_send({to = 'Bob', subject = 's', body = 'b', item = {id = 1}})", "invalid_argument"),
        ("eqoa.actions.mail_send({to = 'Bob', subject = 's', body = 'b', item = 3})", "invalid_argument"),
        ("eqoa.actions.auction_search('Iron')", "invalid_argument"),
        ("eqoa.actions.auction_search({min_level = 'a'})", "invalid_argument"),
        ("eqoa.actions.auction_search({class = 40})", "out_of_range"),
        ("eqoa.actions.auction_search({name = 5})", "invalid_argument"),
        ("eqoa.actions.auction_page('next')", "invalid_argument"),
        ("eqoa.actions.auction_page()", "invalid_argument"),
        ("eqoa.actions.auction_my_lots(1)", "invalid_argument"),
        ("eqoa.actions.auction_bidders({})", "invalid_argument"),
        ("eqoa.actions.auction_buyout(-3)", "invalid_argument"),
        ("eqoa.actions.auction_bid(5)", "invalid_argument"),
        ("eqoa.actions.auction_bid({lot = 5})", "invalid_argument"),
        ("eqoa.actions.auction_bid({lot = 5, price = 0})", "out_of_range"),
        ("eqoa.actions.auction_bid({lot = 5, price = 2^40})", "out_of_range"),
        ("eqoa.actions.auction_sell({slot = 1, id = 1, price = 5, buyout = 9})", "invalid_argument"),
        ("eqoa.actions.auction_sell({slot = 1, id = 1, price = 5, buyout = 9, hours = 0})", "out_of_range"),
        ("eqoa.actions.auction_sell({slot = 1, id = 1, price = -5, buyout = 9, hours = 1})", "out_of_range"),
    ] {
        h.execute(&format!("local r, e = {call}; assert(r == nil and e ~= nil and e.code == '{code}', [=[{call}: ]=] .. tostring(e and e.code))")).unwrap();
    }
    assert!(action_broker::queued_kinds_for_test().is_empty(), "nothing was queued by a bad call");
    // the controls: well-formed calls queue, each under its own name
    h.execute(
        r#"
        assert(eqoa.actions.mail_refresh()); assert(eqoa.actions.mail_read(12)); assert(eqoa.actions.mail_read(4294967295)); assert(eqoa.actions.mail_delete(12)); assert(eqoa.actions.mail_take(12))
        assert(eqoa.actions.mail_send({to = 'Bob', subject = 's', body = 'b', tunar = 5, item = {slot = 1, id = 2, count = 3}}))
        assert(eqoa.actions.auction_search()); assert(eqoa.actions.auction_search({name = 'Iron', class = 3, wear = -1}))
        assert(eqoa.actions.auction_page(true)); assert(eqoa.actions.auction_my_lots()); assert(eqoa.actions.auction_my_bids())
        assert(eqoa.actions.auction_bidders(7)); assert(eqoa.actions.auction_bid({lot = 7, price = 100, count = 2}))
        assert(eqoa.actions.auction_buyout(7))
        assert(eqoa.actions.auction_sell({slot = 1, id = 9, count = 1, price = 5, buyout = 9, hours = 12}))
        "#,
    )
    .unwrap();
    assert_eq!(
        action_broker::queued_kinds_for_test(),
        vec![
            "mail_refresh", "mail_read", "mail_read", "mail_delete", "mail_take", "mail_send", "auction_search", "auction_search", "auction_page",
            "auction_my_lots", "auction_my_bids", "auction_bidders", "auction_bid", "auction_buyout", "auction_sell"
        ]
    );
}

#[test]
fn a10_a_mod_may_open_the_three_windows_and_the_player_s_switch_closes_them() {
    fresh();
    let h = host(&["eqoa.windows.session", "eqoa.events.on"]);
    h.execute(
        r#"
        local a = assert(eqoa.windows.open_session('mail_inbox'))
        local b = assert(eqoa.windows.open_session('mail_compose'))
        local c = assert(eqoa.windows.open_session('auction_search', {opened_from = 'key'}))
        assert(a.kind == 'mail_inbox' and b.kind == 'mail_compose' and c.kind == 'auction_search' and c.title == 'Auction Search')
        assert(eqoa.windows.session('mail_compose').ref == b.ref)
        -- the game's other mail windows are still not a mod's to open
        local t, e = eqoa.windows.open_session('mail_tell')
        assert(t == nil and e.code == 'unsupported', e and e.code)
        "#,
    )
    .unwrap();
    assert!(ws::any_open_of(&["mail_inbox", "mail_compose"]) && ws::any_open_of(&["auction_search"]));
    h.execute("assert(eqoa.windows.close_session(eqoa.windows.session('auction_search').ref))").unwrap();
    assert!(!ws::any_open_of(&["auction_search"]), "control: closed, the client's reading of the auction ends");
    window_takeover::set_use_game_windows(true);
    assert!(!ws::any_open_of(&["mail_inbox", "mail_compose"]), "Use the game's windows ends them");
    h.execute("local s, e = eqoa.windows.open_session('mail_inbox'); assert(s == nil and e.code == 'unavailable', e and e.code)").unwrap();
    window_takeover::set_use_game_windows(false);
}

#[test]
fn a10_mods_and_addons_says_what_a_mod_can_do_and_what_asks_first() {
    // plain mail, a search and the mail's Take go without the question: they are named, plainly
    assert_eq!(
        native_a10::mail_label(&caps(&["eqoa.actions.mail_send", "eqoa.actions.auction_search", "eqoa.actions.mail_take"])).as_deref(),
        Some("Mail and auction: can send plain mail, take what a mail carries, search the auction")
    );
    // reading, deleting, bidding and selling are not on that line: they ask first
    assert_eq!(native_a10::mail_label(&caps(&["eqoa.mail.get", "eqoa.actions.mail_read", "eqoa.actions.mail_delete", "eqoa.actions.auction_bid", "eqoa.actions.auction_sell"])), None);
    assert_eq!(native_a10::mail_label(&caps(&["eqoa.actions.chat"])), None);
    // the wildcard names every one
    let all = native_a10::mail_label(&caps(&["eqoa.actions.*"])).expect("all actions");
    assert!(all.contains("send plain mail") && all.contains("search the auction"), "{all}");
    let asks = super::runtime::asks_first_label(&caps(&["eqoa.actions.mail_delete", "eqoa.actions.mail_send", "eqoa.actions.auction_bid", "eqoa.actions.auction_buyout", "eqoa.actions.auction_sell"]));
    assert_eq!(
        asks.as_deref(),
        Some("Asks you first: delete a mail, send mail that carries tunar or an item, bid at the auction, buy out an auction lot, put an item up at the auction")
    );
    assert_eq!(super::runtime::asks_first_label(&caps(&["eqoa.actions.mail_read", "eqoa.actions.auction_search"])), None);
    // no A10 name is also listed as acting on its own
    assert_eq!(super::runtime::autonomous_action_label(&caps(&["eqoa.actions.mail_send", "eqoa.actions.auction_sell", "eqoa.actions.auction_page"])), None);
    assert_eq!(
        super::runtime::autonomous_action_label(&caps(&["eqoa.actions.mail_send", "eqoa.actions.chat"])).as_deref(),
        Some("Actions: can act on its own (chat (ordinary lines))"),
        "control: another action still is"
    );
}

#[test]
fn a10_the_questions_name_what_is_at_stake() {
    let t = confirm::mail_delete_text("Bob", "Hi there", Some("100 tunar and 2 Iron Ore"));
    assert!(t.contains("\"Hi there\"") && t.contains("Bob") && t.contains("cannot be undone") && t.contains("100 tunar and 2 Iron Ore"), "{t}");
    assert!(!confirm::mail_delete_text("Bob", "Hi", None).contains("carries"));
    let t = confirm::mail_send_text("Bob", "Gift", 100, Some(("Iron Ore", 2)), 15);
    assert!(t.contains("100 tunar and 2 x \"Iron Ore\"") && t.contains("Bob") && t.contains("postage is 15"), "{t}");
    assert!(!confirm::mail_send_text("Bob", "Gift", 100, None, 0).contains("postage"));
    let t = confirm::auction_bid_text("Iron Ore", 150, 5, false, 400);
    assert!(t.starts_with("Bid 150 tunar each on 5 x \"Iron Ore\" (750 in all)") && t.ends_with("Its buyout is 400 tunar each."), "{t}");
    assert!(!confirm::auction_bid_text("Iron Ore", 150, 1, false, 0).contains("buyout"));
    let t = confirm::auction_bid_text("Iron Ore", 400, 1, true, 400);
    assert!(t.starts_with("Buy out \"Iron Ore\" for 400 tunar each"), "{t}");
    let t = confirm::auction_sell_text("Gold Ring", 1, 100, 300, 720);
    assert!(t.contains("\"Gold Ring\"") && t.contains("from 100 tunar") && t.contains("buyout 300") && t.contains("12 hours"), "{t}");
    assert!(confirm::auction_sell_text("Arrow", 20, 5, 9, 720).contains("from 5 tunar each, buyout 9 tunar each"));
    // every one has its own kind name, so a mod hears which question closed
    for (act, kind) in [
        (confirm::Act::MailDelete { id: 1, subject: String::new() }, "mail_delete"),
        (confirm::Act::MailSend { to: String::new(), subject: String::new(), body: String::new(), tunar: 1, item: None }, "mail_send"),
        (confirm::Act::AuctionBid { lot: 1, price: 1, count: 1, buyout: false, name: String::new() }, "auction_bid"),
        (confirm::Act::AuctionBid { lot: 1, price: 1, count: 1, buyout: true, name: String::new() }, "auction_buyout"),
        (confirm::Act::AuctionSell { key: 1, id: 1, name: String::new(), count: 1, price: 1, buyout: 1, minutes: 60 }, "auction_sell"),
    ] {
        assert_eq!(act.kind(), kind);
    }
}

#[test]
fn a10_the_reads_return_the_declared_fields_and_change_revision_only_when_the_state_does() {
    fresh();
    let h = host(&["eqoa.mail.get", "eqoa.auction.get"]);
    h.execute("assert(eqoa.mail.get() == nil and eqoa.auction.get() == nil, 'no window, nothing to read')").unwrap();
    let mail = json!({
        "revision": 0, "offered": true, "status": "idle", "count": 1, "truncated": false, "new": false,
        "entries": [{"id": 3, "from": "Ann", "subject": "Hi", "date": "2026-11-24 10:00 PST", "read": false, "carries": true}],
        "message": {"id": 3, "from": "Ann", "subject": "Hi", "date": "d", "text": "hello", "to": "", "tunar": 5, "items": ["Iron Ore"], "returned": false},
        "pack": [{"slot": 1, "id": 9, "name": "Iron Ore", "quantity": 5}],
        "carries_offer": true, "postage": 15, "max_items": 1, "tunar": 700, "names": ["Zed"], "delivered": false, "refusal": null,
    });
    native_a10::publish_mail(mail.clone());
    h.execute(
        r#"
        local m = assert(eqoa.mail.get())
        for _, f in ipairs({'revision','offered','status','count','truncated','new','entries','message','pack','carries_offer','postage','max_items','tunar','names','delivered'}) do
            assert(m[f] ~= nil, f)
        end
        for _, f in ipairs({'id','from','subject','date','read','carries'}) do assert(m.entries[1][f] ~= nil, f) end
        for _, f in ipairs({'id','from','subject','date','text','to','tunar','items','returned'}) do assert(m.message[f] ~= nil, f) end
        for _, f in ipairs({'slot','id','name','quantity'}) do assert(m.pack[1][f] ~= nil, f) end
        assert(m.entries[1].carries == true and m.message.items[1] == 'Iron Ore' and m.pack[1].slot == 1)
        first_mail = m.revision
        "#,
    )
    .unwrap();
    native_a10::publish_mail(mail.clone());
    h.execute("assert(eqoa.mail.get().revision == first_mail, 'the same state, the same revision')").unwrap();
    let mut changed = mail;
    changed["status"] = json!("waiting_send");
    native_a10::publish_mail(changed);
    h.execute("local m = eqoa.mail.get(); assert(m.revision > first_mail and m.status == 'waiting_send')").unwrap();
    let auction = json!({
        "revision": 0, "offered": true, "mode": "search", "pending": "none", "page": 4, "minutes": 30,
        "results": [{"lot": 501, "name": "Iron Ring", "quantity": 1, "seller": "Zed", "min_bid": 10, "buyout": 400, "minutes_left": 470, "high_bid": 20, "leader": 7, "bids": 3, "mine": false}],
        "lots": [{"lot": 31, "name": "Mine", "quantity": 1, "buyout": 5, "minutes_left": 9}],
        "bids": [{"lot": 41, "price": 100, "count": 2, "escrow": 200}],
        "bidders_lot": 501, "bidders": [{"name": "Ann", "bid": 20}],
        "sellable": [{"slot": 1, "id": 700001, "name": "Gold Ring", "quantity": 1}],
        "tunar": 700, "banked": 5000, "timed_out": false,
    });
    native_a10::publish_auction(auction);
    h.execute(
        r#"
        local a = assert(eqoa.auction.get())
        for _, f in ipairs({'revision','offered','mode','pending','page','results','minutes','lots','bids','bidders_lot','bidders','sellable','tunar','banked','timed_out'}) do
            assert(a[f] ~= nil, f)
        end
        for _, f in ipairs({'lot','name','quantity','seller','min_bid','buyout','minutes_left','high_bid','leader','bids','mine'}) do assert(a.results[1][f] ~= nil, f) end
        for _, f in ipairs({'lot','name','quantity','buyout','minutes_left'}) do assert(a.lots[1][f] ~= nil, f) end
        for _, f in ipairs({'lot','price','count','escrow'}) do assert(a.bids[1][f] ~= nil, f) end
        for _, f in ipairs({'name','bid'}) do assert(a.bidders[1][f] ~= nil, f) end
        for _, f in ipairs({'slot','id','name','quantity'}) do assert(a.sellable[1][f] ~= nil, f) end
        assert(a.results[1].next_bid == nil and a.results[1].leader == 7)
        "#,
    )
    .unwrap();
    native_a10::clear_mail();
    native_a10::clear_auction();
    h.execute("assert(eqoa.mail.get() == nil and eqoa.auction.get() == nil)").unwrap();
}

#[test]
fn a10_no_mod_has_the_answer_to_a_question() {
    fresh();
    let h = host(&["eqoa.actions.*", "eqoa.windows.session", "eqoa.events.on"]);
    h.execute(
        r#"for _, name in ipairs({'answer', 'confirm_yes', 'accept', 'answer_confirm', 'press', 'click', 'mail_delete_yes', 'auction_confirm', 'auction_yes'}) do
            assert(eqoa.windows[name] == nil and eqoa.actions[name] == nil, name .. ' must not exist')
        end"#,
    )
    .unwrap();
}
