// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A9's two bundled mods: EQOA Trade (`eqoa_trade`) and the Bag Viewer's repair panel. Headless: the mods run
//! on a real host and a real ImGui context, the game's state is what `native_a9` is given, and what a mod asks for is
//! read from the action broker's queue. Each test says what its control is.
use super::{
    a1_mod_tests::{addon as plain_addon, context, fixture, frames, manifest, root},
    action_broker,
    bridge::Bridge,
    host::Host,
    native_a9::{self, View},
    native_events, runtime, window_sessions as ws,
    window_takeover::{self, TakeoverKind},
};
use dear_imgui_rs::sys;
use serde_json::json;
use std::time::{Duration, Instant};

const TRADE: &str = "eqoa_trade";
const BV: &str = "eqoa_bag_viewer";

/// The package as the client discovers it: its manifest's `provides` and `replaces` too (a claim needs `replaces`).
fn addon(id: &str) -> super::discovery::DiscoveredAddon {
    let mut a = plain_addon(id);
    let m = manifest(id);
    let list = |k: &str| -> Vec<String> {
        m[k].as_array().map(|v| v.iter().filter_map(|s| s.as_str().map(str::to_owned)).collect()).unwrap_or_default()
    };
    a.extensions.provides = list("provides");
    a.extensions.replaces = list("replaces");
    a
}

fn used_names(src: &str, prefix: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = src;
    while let Some(i) = rest.find(prefix) {
        rest = &rest[i + prefix.len()..];
        let name: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
        if !name.is_empty() && !found.contains(&name) {
            found.push(name);
        }
    }
    found
}

fn assert_window_fits(name: &str, label: &str) {
    let cname = std::ffi::CString::new(name).unwrap();
    let w = unsafe { sys::igFindWindowByName(cname.as_ptr()) };
    assert!(!w.is_null(), "{label}: window {name:?} was not drawn");
    let (content_x, inner_w) = unsafe {
        let win = &*w;
        (win.ContentSize.x, win.InnerRect.Max.x - win.InnerRect.Min.x)
    };
    assert!(content_x <= inner_w + 2.0, "{label}: content ({content_x:.1}px) exceeds the window ({inner_w:.1}px) in {name:?}");
}

fn window_drawn(name: &str) -> bool {
    let cname = std::ffi::CString::new(name).unwrap();
    let w = unsafe { sys::igFindWindowByName(cname.as_ptr()) };
    !w.is_null() && unsafe { (*w).WasActive }
}

fn start(id: &str) -> (Host, Bridge, dear_imgui_rs::Context, u64) {
    fixture();
    native_a9::clear_all();
    let mut ctx = context();
    let a = addon(id);
    let host = Host::for_addon(&a).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.run_addon(&a).unwrap();
    let vm = native_events::bus(&host.lua).borrow().vm;
    frames(&host, &bridge, &mut ctx, 2);
    (host, bridge, ctx, vm)
}

fn hold(vm: u64, kind: TakeoverKind) {
    ws::sync_held_request(kind, Some((vm, TRADE.to_string())), "ended");
}

fn queued() -> Vec<&'static str> {
    action_broker::queued_kinds_for_test()
}

fn request_state() -> serde_json::Value {
    json!({"kind": "trade", "phase": "request", "held": true, "partner": {"id": 77, "name": "Bobbington the Long-Named Trader"}, "initiated": false})
}

fn open_state(accepted_mine: bool) -> serde_json::Value {
    let bag = |slot: i64, id: i64, name: &str, qty: i64, worn: bool| {
        json!({"slot": slot, "id": id.to_string(), "name": name, "quantity": qty, "worn": worn, "no_trade": false, "can_offer": !worn, "text": format!("{name}\nDamage 12 / Delay 30\nLevel 20")})
    };
    json!({
        "kind": "trade", "phase": "open", "held": true,
        "partner": {"id": 77, "name": "Bobbington the Long-Named Trader"},
        "initiated": false,
        "mine": {"items": [{"slot": 1, "id": "10", "name": "A Very Long Item Name Of Great Rarity", "quantity": 5}], "money": 120, "accepted": accepted_mine},
        "theirs": {"items": [{"slot": 1, "id": "99", "name": "Fine Steel Sword of the Northern Wastes", "quantity": 1}], "money": 5000, "accepted": false},
        "bags": [
            bag(1, 10, "A Very Long Item Name Of Great Rarity", 5, false),
            bag(2, 11, "Rusty Sword", 1, false),
            bag(3, 12, "Worn Helm", 1, true),
        ],
        "carried": 880, "max_items": 4, "can_accept": !accepted_mine, "accept_asked": false,
    })
}

// ---- manifests, labels, packaging ----

#[test]
fn a9_mods_trade_ships_off_declares_what_it_uses_and_says_what_it_can_do() {
    fixture();
    let m = manifest(TRADE);
    assert_eq!(m["enabled"], false, "eqoa_trade must ship off");
    assert!(crate::fork_ext::bundled::PACKAGES.contains(&TRADE));
    for file in ["mod.json", "main.lua", "README.md", "modules/tradeui.lua"] {
        assert!(
            crate::fork_ext::bundled::EMBEDDED_FILES.iter().any(|f| f.path == format!("{TRADE}/{file}")),
            "{file} must be embedded"
        );
        let bytes = std::fs::read(root(TRADE).join(file)).unwrap();
        assert!(!bytes.contains(&b'\r'), "{file} must be saved with LF line endings");
    }
    let mut used = Vec::new();
    for file in ["main.lua", "modules/tradeui.lua"] {
        for n in used_names(&std::fs::read_to_string(root(TRADE).join(file)).unwrap(), "eqoa.actions.") {
            if !used.contains(&n) {
                used.push(n);
            }
        }
    }
    let declared: Vec<String> = m["capabilities"].as_array().unwrap().iter().map(|c| c.as_str().unwrap().to_string()).collect();
    for n in &used {
        assert!(declared.contains(&format!("eqoa.actions.{n}")), "calls eqoa.actions.{n} without declaring it");
    }
    for cap in declared.iter().filter(|c| c.starts_with("eqoa.actions.")) {
        assert!(used.iter().any(|u| cap.ends_with(u.as_str())), "declares {cap} without calling it");
    }
    let lines = runtime::disclosure_lines(&addon(TRADE)).join("\n");
    assert!(lines.contains("Asks you first: accept a trade or a duel wager"), "{lines}");
    assert!(lines.contains("Trading: can answer trade requests, offer items in a trade, offer coin in a trade"), "{lines}");
    // the Bag Viewer says it can repair on its own (its manifest names the action), and the label is the existing one
    let bv = runtime::disclosure_lines(&addon(BV)).join("\n");
    assert!(bv.contains("repair items (spends your coin)"), "{bv}");
    assert!(!lines.contains("can act on its own"), "the trade mod acts on nothing without a click or a Yes: {lines}");
    // the Bag Viewer's new file has LF endings too
    let bytes = std::fs::read(root(BV).join("modules/repair.lua")).unwrap();
    assert!(!bytes.contains(&b'\r'));
}

// ---- the Trade mod ----

#[test]
fn a9_trade_mod_holds_the_windows_only_while_on_and_a_request_is_answered_with_one_click() {
    // control: before the mod runs, nothing is held (the game's windows are the game's)
    fixture();
    for k in TakeoverKind::A9 {
        assert!(!window_takeover::is_taken(k), "control: {k:?} held with no mod");
    }
    let (host, bridge, mut ctx, vm) = start(TRADE);
    for k in [TakeoverKind::TradeAsk, TakeoverKind::Trade, TakeoverKind::DuelInvite, TakeoverKind::Duel] {
        assert!(window_takeover::is_taken(k), "the mod holds {k:?}");
    }
    assert!(!window_takeover::is_taken(TakeoverKind::Blacksmith), "the smith is not the trade mod's");
    // nothing up: nothing drawn
    assert_eq!(frames(&host, &bridge, &mut ctx, 2), 0);

    // a held request: the prompt draws, fits at the default size, and one click's worth of call answers it
    native_a9::publish(View::Trade, Some(request_state()), Some((Instant::now(), Duration::from_secs(45))));
    hold(vm, TakeoverKind::TradeAsk);
    assert!(frames(&host, &bridge, &mut ctx, 3) > 0);
    assert_window_fits("Request##eqoa_trade_prompt", "trade request");
    assert!(queued().is_empty(), "control: showing a request answers nothing");
    host.execute("require('tradeui').answer_trade(true)").unwrap();
    assert_eq!(queued(), vec!["trade_answer"]);
    host.execute("require('tradeui').answer_trade(false)").unwrap();
    assert_eq!(queued(), vec!["trade_answer", "trade_answer"]);
    let logs = host.drain_logs().unwrap();
    assert!(logs.is_empty(), "{logs:?}");
    assert!(bridge.drain_diagnostics().is_empty());

    // a duel invitation uses the same prompt
    native_a9::publish(View::Trade, None, None);
    action_broker::reset_all_for_test();
    ws::sync_held_request(TakeoverKind::TradeAsk, None, "ended");
    let duel = json!({"enabled": true, "state": "invited", "queued": 0,
        "invite": {"id": 5, "name": "Lanerender", "race_id": 6, "race_name": "Human", "class_id": 0, "class_name": "Warrior", "level": 40, "guild": "", "title": ""}});
    native_a9::publish(View::Duel, Some(duel), Some((Instant::now(), Duration::from_secs(20))));
    hold(vm, TakeoverKind::DuelInvite);
    assert!(frames(&host, &bridge, &mut ctx, 3) > 0);
    assert_window_fits("Request##eqoa_trade_prompt", "duel invitation");
    host.execute("require('tradeui').answer_duel(true)").unwrap();
    assert_eq!(queued(), vec!["duel_answer"]);

    // turning the mod off gives the windows back at once
    host.execute("require('tradeui')").unwrap();
    window_takeover::release_vm(vm);
    for k in TakeoverKind::A9 {
        assert!(!window_takeover::is_taken(k), "mod gone: {k:?} released");
    }
}

#[test]
fn a9_trade_mod_draws_the_answering_players_open_trade_and_never_accepts_by_itself() {
    let (host, bridge, mut ctx, vm) = start(TRADE);
    // the answering player's trade (initiated: false), long names, three bag rows, one of them worn
    native_a9::publish(View::Trade, Some(open_state(false)), None);
    hold(vm, TakeoverKind::Trade);
    assert!(frames(&host, &bridge, &mut ctx, 3) > 0);
    assert!(window_drawn("Trade with Bobbington the Lon..##eqoa_trade_window"), "the answerer's trade window is drawn");
    assert_window_fits("Trade with Bobbington the Lon..##eqoa_trade_window", "open trade at the default size");
    let logs = host.drain_logs().unwrap();
    assert!(logs.is_empty(), "{logs:?}");
    assert!(bridge.drain_diagnostics().is_empty(), "no ImGui recovery diagnostics");
    assert!(queued().is_empty(), "control: a drawn trade sends and accepts nothing");

    // the right-click offer: a row that can be offered, one that is worn (the core refuses it too), and the whole stack
    host.execute("local T = require('tradeui'); local t = eqoa.trade.get(); T.offer_row(t.bags[2], false)").unwrap();
    assert_eq!(queued(), vec!["trade_add"]);
    host.execute("local T = require('tradeui'); local t = eqoa.trade.get(); T.offer_row(t.bags[3], false)").unwrap();
    assert_eq!(queued(), vec!["trade_add"], "a worn row is not offered");
    host.execute("local T = require('tradeui'); local t = eqoa.trade.get(); T.take_back(t.mine.items[1]); T.set_money('250')").unwrap();
    assert_eq!(queued(), vec!["trade_add", "trade_remove", "trade_money"]);
    // Accept only asks (the client's own question); it does not accept
    host.execute("require('tradeui').accept()").unwrap();
    assert_eq!(queued().last(), Some(&"trade_accept"));
    assert!(!queued().contains(&"trade_answer") && !queued().contains(&"trade_unaccept"));

    // an accept that stands shows "Take back accept"; the window still fits
    native_a9::publish(View::Trade, Some(open_state(true)), None);
    frames(&host, &bridge, &mut ctx, 3);
    assert_window_fits("Trade with Bobbington the Lon..##eqoa_trade_window", "accepted trade");
    host.execute("require('tradeui').unaccept()").unwrap();
    assert_eq!(queued().last(), Some(&"trade_unaccept"));

    // the end of the trade is told for a few seconds, then gone
    native_a9::publish(View::Trade, None, None);
    host.execute("local S = require('tradeui').state; S.notice = {text = 'Trade completed', finish = os.clock() + 5}").unwrap();
    assert!(frames(&host, &bridge, &mut ctx, 2) > 0);
    assert_window_fits("Trade##eqoa_trade_notice", "notice");
    host.execute("require('tradeui').state.notice.finish = os.clock() - 1").unwrap();
    assert_eq!(frames(&host, &bridge, &mut ctx, 2), 0, "the notice goes away");
}

#[test]
fn a9_trade_mod_coin_box_holds_enter_by_its_exact_name_and_renews_text_capture_every_frame() {
    let (host, bridge, mut ctx, vm) = start(TRADE);
    native_a9::publish(View::Trade, Some(open_state(false)), None);
    hold(vm, TakeoverKind::Trade);
    // the client lets the capture lapse after about a second unless it is asked for again: its last-asked time is read
    // after every frame
    // control: the box is not being typed in
    frames(&host, &bridge, &mut ctx, 5);
    assert!(super::input::native::text_capture_stamp().is_none(), "control: no capture while the box is idle");
    assert!(host.lua.load("return require('tradeui').state.enter == nil").eval::<bool>().unwrap(), "control: Enter is not held while idle");
    // typing: capture asked for in each of 6 frames, Enter held under its exact name
    host.execute("require('tradeui').force_typing = true").unwrap();
    frames(&host, &bridge, &mut ctx, 1);
    let mut last = super::input::native::text_capture_stamp().expect("capture asked for");
    for i in 0..5 {
        std::thread::sleep(Duration::from_millis(3));
        frames(&host, &bridge, &mut ctx, 1);
        let now = super::input::native::text_capture_stamp().expect("still held");
        assert!(now > last, "frame {i}: the capture was asked for again on this frame");
        last = now;
    }
    assert!(host.lua.load("return require('tradeui').state.enter ~= nil").eval::<bool>().unwrap(), "Enter claimed (a refused name leaves nil)");
    assert!(host.drain_logs().unwrap().is_empty(), "no 'unknown app key name' diagnostic");
    // leaving the box lets both go
    host.execute("require('tradeui').force_typing = false").unwrap();
    frames(&host, &bridge, &mut ctx, 2);
    assert!(host.lua.load("return require('tradeui').state.enter == nil and not require('tradeui').state.capturing").eval::<bool>().unwrap());
    // Enter in the box sets the coins
    host.execute("local S = require('tradeui').state; S.typing = true; S.money_text = '300'").unwrap();
    CORE_press_enter(&host);
    assert_eq!(queued().last(), Some(&"trade_money"));
}

#[allow(non_snake_case)]
fn CORE_press_enter(host: &Host) {
    super::native_core::CORE.with(|c| {
        c.borrow_mut().emit("input.key", json!({"key": "Enter", "combo": "enter", "down": true, "repeat": false, "synthetic": false, "text": ""}))
    });
    native_events::dispatch(host).unwrap();
}

// ---- the Bag Viewer's repair panel ----

fn smith_state(costs: &[(i64, i64)], pending: bool, coin: i64) -> serde_json::Value {
    let items: Vec<_> = costs
        .iter()
        .enumerate()
        .map(|(i, (key, cost))| {
            json!({"slot": i + 1, "id": (100 + key).to_string(), "inventory_key": key, "name": format!("Damaged Item Number {key} of Great Length"),
                "quantity": 1, "condition": 0.5, "cost": cost, "can_repair": *cost > 0, "worn": false})
        })
        .collect();
    let total: i64 = costs.iter().map(|c| c.1).sum();
    json!({"coin": coin, "repair_pending": pending, "held": false, "total": total, "items": items})
}

fn result_line(host: &Host) -> String {
    host.lua.load("return require('bags').result and require('bags').result.text or ''").eval().unwrap()
}

fn sent_ok(host: &Host) {
    host.execute("local R = require('repair'); R.on_action_result({request_id = R.run.wait, ok = true, phase = 'completed'})").unwrap();
}

#[test]
fn a9_bag_viewer_repair_lists_costs_asks_once_for_all_and_repairs_one_at_a_time() {
    let (host, bridge, mut ctx, _vm) = start(BV);
    // control: no smith open, no panel
    assert!(!window_drawn("Repair##eqoa_bag_viewer_repair"));
    assert_eq!(frames(&host, &bridge, &mut ctx, 2), 0);
    native_a9::publish(View::Smith, Some(smith_state(&[(1, 50), (2, 70), (3, 0)], false, 500)), None);
    frames(&host, &bridge, &mut ctx, 3);
    assert!(window_drawn("Repair##eqoa_bag_viewer_repair"), "the panel is up while a smith is open");
    assert_window_fits("Repair##eqoa_bag_viewer_repair", "repair panel at the default size");
    // the smith's own diagnostics: only the Bag Viewer's start-up note (auto-loot) is allowed
    let logs: Vec<String> = host.drain_logs().unwrap().into_iter().filter(|l| !l.contains("auto-loot")).collect();
    assert!(logs.is_empty(), "{logs:?}");
    assert!(bridge.drain_diagnostics().is_empty());
    // one click repairs one item, and it is told as repaired only once the smith's list shows it so
    host.execute("local R = require('repair'); R.repair_one(R.damaged(eqoa.blacksmith.get())[1])").unwrap();
    assert_eq!(queued(), vec!["blacksmith_repair"]);
    sent_ok(&host);
    // the smith's read after the send: its repair is pending, the item still damaged
    native_a9::publish(View::Smith, Some(smith_state(&[(1, 50), (2, 70), (3, 0)], true, 500)), None);
    frames(&host, &bridge, &mut ctx, 2);
    assert!(host.lua.load("return require('repair').run ~= nil").eval::<bool>().unwrap(), "sent is not repaired: the run waits for the list");
    native_a9::publish(View::Smith, Some(smith_state(&[(1, 0), (2, 70), (3, 0)], false, 450)), None);
    frames(&host, &bridge, &mut ctx, 2);
    assert!(host.lua.load("return require('repair').status.text:find('Repaired 1 item for 50 coins') ~= nil").eval::<bool>().unwrap());
    action_broker::reset_all_for_test();

    // "Repair all": the question names the total and nothing is sent before the Yes (the control)
    host.execute("local R = require('repair'); R.begin_ask(eqoa.blacksmith.get())").unwrap();
    frames(&host, &bridge, &mut ctx, 3);
    assert_window_fits("Repair##eqoa_bag_viewer_repair", "repair question");
    assert!(queued().is_empty(), "the question sends nothing");
    host.execute("local R = require('repair'); R.answer(eqoa.blacksmith.get(), false)").unwrap();
    frames(&host, &bridge, &mut ctx, 2);
    assert!(queued().is_empty(), "No repairs nothing");
    // Yes: one repair goes, and no second until the smith's own list shows the first done
    native_a9::publish(View::Smith, Some(smith_state(&[(1, 50), (2, 70), (3, 0)], false, 500)), None);
    host.execute("local R = require('repair'); local s = eqoa.blacksmith.get(); R.begin_ask(s); R.answer(s, true)").unwrap();
    frames(&host, &bridge, &mut ctx, 4);
    assert_eq!(queued(), vec!["blacksmith_repair"], "one at a time");
    sent_ok(&host);
    native_a9::publish(View::Smith, Some(smith_state(&[(1, 0), (2, 70), (3, 0)], true, 450)), None);
    frames(&host, &bridge, &mut ctx, 3);
    assert_eq!(queued(), vec!["blacksmith_repair"], "no second repair while the smith's last one is pending");
    native_a9::publish(View::Smith, Some(smith_state(&[(1, 0), (2, 70), (3, 0)], false, 450)), None);
    frames(&host, &bridge, &mut ctx, 3);
    assert_eq!(queued(), vec!["blacksmith_repair", "blacksmith_repair"], "the next goes after the answer");
    sent_ok(&host);
    native_a9::publish(View::Smith, Some(smith_state(&[(1, 0), (2, 0), (3, 0)], false, 380)), None);
    frames(&host, &bridge, &mut ctx, 3);
    assert_eq!(queued().len(), 2, "nothing is sent twice, and the run is over");
    assert!(result_line(&host).starts_with("Repaired 2 items for 120 coins"), "{}", result_line(&host));
    // the smith closes: the result stays up a few seconds, then goes
    native_a9::publish(View::Smith, None, None);
    assert!(frames(&host, &bridge, &mut ctx, 2) > 0, "the last result stays up after the smith closes");
    assert!(window_drawn("Repair##eqoa_bag_viewer_repair_done"));
    host.execute("require('repair').last.finish = os.clock() - 1").unwrap();
    assert_eq!(frames(&host, &bridge, &mut ctx, 2), 0, "and goes away");

    // an item the smith never answers is not told as repaired: the run stops with the reason, after one send
    action_broker::reset_all_for_test();
    native_a9::publish(View::Smith, Some(smith_state(&[(1, 50), (2, 70)], false, 500)), None);
    frames(&host, &bridge, &mut ctx, 2);
    host.execute("local R = require('repair'); local s = eqoa.blacksmith.get(); R.begin_ask(s); R.answer(s, true)").unwrap();
    frames(&host, &bridge, &mut ctx, 3);
    sent_ok(&host);
    native_a9::publish(View::Smith, Some(smith_state(&[(1, 50), (2, 70)], true, 500)), None);
    frames(&host, &bridge, &mut ctx, 2);
    native_a9::publish(View::Smith, Some(smith_state(&[(1, 50), (2, 70)], false, 500)), None);
    frames(&host, &bridge, &mut ctx, 3);
    assert_eq!(queued(), vec!["blacksmith_repair"], "nothing more is sent after an unanswered one");
    let status: String = host.lua.load("return require('repair').status.text").eval().unwrap();
    assert!(status.contains("no answer from the smith") && status.starts_with("Repaired 0 items for 0 coins"), "{status}");

    // a changed price makes the Yes ask again instead of repairing at a price the player did not see
    action_broker::reset_all_for_test();
    native_a9::publish(View::Smith, Some(smith_state(&[(1, 50)], false, 500)), None);
    frames(&host, &bridge, &mut ctx, 2);
    host.execute("local R = require('repair'); R.status = nil; R.begin_ask(eqoa.blacksmith.get())").unwrap();
    native_a9::publish(View::Smith, Some(smith_state(&[(1, 90)], false, 500)), None);
    host.execute("local R = require('repair'); R.answer(eqoa.blacksmith.get(), true)").unwrap();
    frames(&host, &bridge, &mut ctx, 3);
    assert!(queued().is_empty(), "a Yes for a different total repairs nothing");

    // what cannot be paid for is skipped, the rest repaired, and the line says how many were skipped
    action_broker::reset_all_for_test();
    native_a9::publish(View::Smith, Some(smith_state(&[(1, 50), (2, 70)], false, 60)), None);
    frames(&host, &bridge, &mut ctx, 2);
    host.execute("local R = require('repair'); local s = eqoa.blacksmith.get(); R.begin_ask(s); assert(R.ask.n == 1 and R.ask.total == 50 and R.ask.skipped == 1); R.answer(s, true)").unwrap();
    frames(&host, &bridge, &mut ctx, 3);
    assert_eq!(queued(), vec!["blacksmith_repair"], "only the item that can be paid for");
    sent_ok(&host);
    native_a9::publish(View::Smith, Some(smith_state(&[(1, 0), (2, 70)], false, 10)), None);
    frames(&host, &bridge, &mut ctx, 3);
    assert!(result_line(&host).contains("Repaired 1 item for 50 coins; 1 skipped (not enough coin)"), "{}", result_line(&host));
}

#[test]
fn a9_bag_viewer_repair_automatically_is_off_by_default_per_character_and_repairs_all_when_the_smith_opens() {
    // control: the option is off, a smith opens, nothing is sent
    {
        let (host, bridge, mut ctx, _vm) = start(BV);
        assert!(host.lua.load("return require('repair').auto_on() == false").eval::<bool>().unwrap(), "off by default");
        native_a9::publish(View::Smith, Some(smith_state(&[(1, 50), (2, 70)], false, 500)), None);
        frames(&host, &bridge, &mut ctx, 4);
        assert!(queued().is_empty(), "with the option off the panel waits for a click");
    }

    // ticked for this character: it repairs as the smith opens, with no question, one at a time, and says the total
    let (host, bridge, mut ctx, _vm) = start(BV);
    host.execute("require('repair').set_auto(true)").unwrap();
    native_a9::publish(View::Smith, Some(smith_state(&[(1, 50), (2, 70)], false, 500)), None);
    frames(&host, &bridge, &mut ctx, 4);
    assert_eq!(queued(), vec!["blacksmith_repair"], "the first repair goes at once, no question");
    sent_ok(&host);
    native_a9::publish(View::Smith, Some(smith_state(&[(1, 0), (2, 70)], false, 450)), None);
    frames(&host, &bridge, &mut ctx, 3);
    sent_ok(&host);
    native_a9::publish(View::Smith, Some(smith_state(&[(1, 0), (2, 0)], false, 380)), None);
    frames(&host, &bridge, &mut ctx, 3);
    assert_eq!(queued().len(), 2);
    assert!(result_line(&host).starts_with("Repaired 2 items for 120 coins"), "{}", result_line(&host));
    // it does not repair again for the same visit
    frames(&host, &bridge, &mut ctx, 3);
    assert_eq!(queued().len(), 2);
    // another character on the same account has its own switch: off
    super::native_core::CORE.with(|c| c.borrow_mut().view.player = json!({"id": 257, "name": "Alt", "level": 20}));
    native_a9::publish(View::Smith, None, None);
    frames(&host, &bridge, &mut ctx, 2);
    action_broker::reset_all_for_test();
    native_a9::publish(View::Smith, Some(smith_state(&[(1, 50)], false, 500)), None);
    frames(&host, &bridge, &mut ctx, 4);
    assert!(queued().is_empty(), "the option is per character: this one has it off");
    // and a refusal stops the run with the reason (back on the first character, too little coin)
    super::native_core::CORE.with(|c| c.borrow_mut().view.player = json!({"id": 256, "name": "Me", "level": 20}));
    native_a9::publish(View::Smith, None, None);
    frames(&host, &bridge, &mut ctx, 2);
    native_a9::publish(View::Smith, Some(smith_state(&[(1, 50)], false, 500)), None);
    frames(&host, &bridge, &mut ctx, 3);
    host.execute("local R = require('repair'); R.on_action_result({request_id = R.run.wait, ok = false, code = 'not_ready', reason = 'you carry 10', phase = 'completed'})").unwrap();
    assert!(result_line(&host).contains("stopped: you carry 10"), "{}", result_line(&host));
}

#[test]
fn a9_trade_mod_takes_its_windows_again_after_zoning() {
    let (host, bridge, mut ctx, _vm) = start(TRADE);
    let kinds = [TakeoverKind::TradeAsk, TakeoverKind::Trade, TakeoverKind::DuelInvite, TakeoverKind::Duel];
    for k in kinds {
        assert!(window_takeover::is_taken(k));
    }
    // zoning: the client drops every claim (the mod keeps running)
    window_takeover::release_all();
    for k in kinds {
        assert!(!window_takeover::is_taken(k), "dropped by the world change");
    }
    // control: frames alone do not take them back (the mod would stay without them for the rest of the session)
    frames(&host, &bridge, &mut ctx, 3);
    assert!(!window_takeover::is_taken(TakeoverKind::Trade), "control: nothing re-claims without the lifecycle event");
    // the lifecycle: the state is reset (or the world is entered) and the mod takes them again
    super::native_core::CORE.with(|c| c.borrow_mut().emit("state.reset", json!({"in_world": true, "initial": false})));
    frames(&host, &bridge, &mut ctx, 3);
    for k in kinds {
        assert!(window_takeover::is_taken(k), "taken again after the reset: {k:?}");
    }
    window_takeover::release_all();
    super::native_core::CORE.with(|c| c.borrow_mut().emit("world.entered", json!({"world_id": 2, "initial": false})));
    frames(&host, &bridge, &mut ctx, 3);
    assert!(window_takeover::is_taken(TakeoverKind::TradeAsk), "and on world.entered");
}

// ---- real input on the Trade window (the fix round after the mods review) ----

fn window_ptr(name_part: &str) -> Option<*mut sys::ImGuiWindow> {
    unsafe {
        let ctx = &*sys::igGetCurrentContext();
        let windows = &ctx.Windows;
        for i in 0..windows.Size {
            let w = *windows.Data.add(i as usize);
            if !w.is_null() && (*w).WasActive {
                let name = std::ffi::CStr::from_ptr((*w).Name).to_string_lossy();
                if name.contains(name_part) {
                    return Some(w);
                }
            }
        }
    }
    None
}

/// What a scan of an item found: its Dear ImGui id and every place (sampled every few pixels) where it was hovered.
struct Scan {
    want: u32,
    hits: Vec<(f32, f32)>,
}

/// Every place where Dear ImGui says the item `label` of the window (or child) whose name contains `window` is
/// hovered: its drawn rectangle as the player can reach it.
fn hovered_points(host: &Host, bridge: &Bridge, ctx: &mut dear_imgui_rs::Context, window: &str, label: &str) -> Scan {
    let w = window_ptr(window).unwrap_or_else(|| panic!("window {window:?} is not drawn"));
    let (pos, size, seed) = unsafe { ((*w).Pos, (*w).Size, (*w).ID) };
    let label_c = std::ffi::CString::new(label).unwrap();
    let want = unsafe { sys::igImHashStr(label_c.as_ptr(), 0, seed) };
    let mut hits = Vec::new();
    let mut y = pos.y + 2.0;
    while y < pos.y + size.y {
        let mut x = pos.x + 2.0;
        while x < pos.x + size.x {
            ctx.io_mut().add_mouse_pos_event([x, y]);
            frames(host, bridge, ctx, 1);
            if unsafe { (*sys::igGetCurrentContext()).HoveredId } == want {
                hits.push((x, y));
            }
            x += 5.0;
        }
        y += 3.0;
    }
    Scan { want, hits }
}

/// The points a player clicks on a button: its centre and four points inside its corners. Each is moved to for real
/// and must hover the item now (the nearest sampled place that does). A button something else lies over hovers only a
/// thin strip: its height fails the size check.
fn click_points(host: &Host, bridge: &Bridge, ctx: &mut dear_imgui_rs::Context, scan: &Scan, label: &str) -> Vec<(f32, f32)> {
    let hits = &scan.hits;
    assert!(!hits.is_empty(), "no place hovers {label:?}");
    let x0 = hits.iter().map(|p| p.0).fold(f32::MAX, f32::min);
    let x1 = hits.iter().map(|p| p.0).fold(f32::MIN, f32::max);
    let y0 = hits.iter().map(|p| p.1).fold(f32::MAX, f32::min);
    let y1 = hits.iter().map(|p| p.1).fold(f32::MIN, f32::max);
    assert!(y1 - y0 >= 10.0, "{label:?} is hovered over only {:.0}px of height: it is covered by something", y1 - y0);
    let (w, h) = (x1 - x0, y1 - y0);
    let targets = [
        ((x0 + x1) / 2.0, (y0 + y1) / 2.0),
        (x0 + w * 0.15, y0 + h * 0.15),
        (x1 - w * 0.15, y0 + h * 0.15),
        (x0 + w * 0.15, y1 - h * 0.15),
        (x1 - w * 0.15, y1 - h * 0.15),
    ];
    let mut out = Vec::new();
    for (tx, ty) in targets {
        let mut sorted = hits.clone();
        sorted.sort_by(|a, b| ((a.0 - tx).powi(2) + (a.1 - ty).powi(2)).total_cmp(&((b.0 - tx).powi(2) + (b.1 - ty).powi(2))));
        let mut found = None;
        for (x, y) in sorted {
            ctx.io_mut().add_mouse_pos_event([x, y]);
            frames(host, bridge, ctx, 1);
            if unsafe { (*sys::igGetCurrentContext()).HoveredId } == scan.want {
                found = Some((x, y));
                break;
            }
        }
        out.push(found.unwrap_or_else(|| panic!("no place near ({tx:.0}, {ty:.0}) of {label:?} hovers it when the pointer goes there")));
    }
    out
}

/// Moves the pointer along a scan until the item is hovered, and stops there (for items whose whole rectangle is not
/// the point of the test: the coin box takes its focus from the first place it is reached).
fn hover_first(host: &Host, bridge: &Bridge, ctx: &mut dear_imgui_rs::Context, window: &str, label: &str) -> (f32, f32) {
    let w = window_ptr(window).unwrap_or_else(|| panic!("window {window:?} is not drawn"));
    let (pos, size, seed) = unsafe { ((*w).Pos, (*w).Size, (*w).ID) };
    let label_c = std::ffi::CString::new(label).unwrap();
    let want = unsafe { sys::igImHashStr(label_c.as_ptr(), 0, seed) };
    let mut y = pos.y + 2.0;
    while y < pos.y + size.y {
        let mut x = pos.x + 2.0;
        while x < pos.x + size.x {
            ctx.io_mut().add_mouse_pos_event([x, y]);
            frames(host, bridge, ctx, 1);
            if unsafe { (*sys::igGetCurrentContext()).HoveredId } == want {
                return (x, y);
            }
            x += 6.0;
        }
        y += 4.0;
    }
    panic!("no place in {window:?} hovers {label:?}");
}

/// Moves the pointer to the item's centre (a place that really hovers it).
fn hover_item(host: &Host, bridge: &Bridge, ctx: &mut dear_imgui_rs::Context, window: &str, label: &str) -> (f32, f32) {
    let scan = hovered_points(host, bridge, ctx, window, label);
    let (x, y) = click_points(host, bridge, ctx, &scan, label)[0];
    ctx.io_mut().add_mouse_pos_event([x, y]);
    frames(host, bridge, ctx, 1);
    (x, y)
}

fn click(host: &Host, bridge: &Bridge, ctx: &mut dear_imgui_rs::Context, button: dear_imgui_rs::MouseButton) {
    ctx.io_mut().add_mouse_button_event(button, true);
    frames(host, bridge, ctx, 1);
    ctx.io_mut().add_mouse_button_event(button, false);
    frames(host, bridge, ctx, 2);
}

const TRADE_WIN: &str = "eqoa_trade_window";

#[test]
fn a9_trade_mod_set_button_sends_what_was_typed_by_a_real_click() {
    let (host, bridge, mut ctx, vm) = start(TRADE);
    native_a9::publish(View::Trade, Some(open_state(false)), None);
    hold(vm, TakeoverKind::Trade);
    frames(&host, &bridge, &mut ctx, 3);
    // the coin box: click it, type 500
    hover_first(&host, &bridge, &mut ctx, TRADE_WIN, "##coins");
    click(&host, &bridge, &mut ctx, dear_imgui_rs::MouseButton::Left);
    for c in "500".chars() {
        ctx.io_mut().add_input_character(c);
        frames(&host, &bridge, &mut ctx, 1);
    }
    assert_eq!(host.lua.load("return require('tradeui').state.money_text").eval::<String>().unwrap(), "500");
    assert!(queued().is_empty(), "typing offers nothing");
    // the click on Set: the box loses the keyboard on the press, the button fires on the release
    hover_first(&host, &bridge, &mut ctx, TRADE_WIN, "Set##coins_set");
    click(&host, &bridge, &mut ctx, dear_imgui_rs::MouseButton::Left);
    assert_eq!(queued(), vec!["trade_money"], "Set sends the offer");
    assert_eq!(host.lua.load("return require('tradeui').state.last_money_sent").eval::<i64>().unwrap(), 500, "the typed amount, not the current offer (120)");
    assert!(host.lua.load("return require('tradeui').state.money_text == nil").eval::<bool>().unwrap(), "and the box is empty again");
}

#[test]
fn a9_trade_mod_offers_by_button_and_by_right_click_with_the_window_in_front() {
    let (host, bridge, mut ctx, vm) = start(TRADE);
    native_a9::publish(View::Trade, Some(open_state(false)), None);
    hold(vm, TakeoverKind::Trade);
    frames(&host, &bridge, &mut ctx, 3);
    // the mod no longer depends on the pointer event another mod's claim can swallow
    let src = std::fs::read_to_string(root(TRADE).join("modules/tradeui.lua")).unwrap();
    assert!(!src.contains("pointer.pressed"), "the offer is the window's own input, not a pointer event");
    // the row's Offer button (left-click), found inside the bags list
    hover_item(&host, &bridge, &mut ctx, "##bags", "Offer##offer_2");
    click(&host, &bridge, &mut ctx, dear_imgui_rs::MouseButton::Left);
    assert_eq!(queued(), vec!["trade_add"], "Offer sends one add");
    // the right-click on the row itself
    hover_item(&host, &bridge, &mut ctx, "##bags", "Rusty Sword x1##bag_2");
    click(&host, &bridge, &mut ctx, dear_imgui_rs::MouseButton::Right);
    assert_eq!(queued(), vec!["trade_add", "trade_add"], "a right-click on a row offers it");
    // control: a right-click on the window's empty space offers nothing
    ctx.io_mut().add_mouse_pos_event([5.0, 5.0]);
    frames(&host, &bridge, &mut ctx, 1);
    click(&host, &bridge, &mut ctx, dear_imgui_rs::MouseButton::Right);
    assert_eq!(queued().len(), 2);
    // a stack's All button offers the whole stack (a moment after the other row: the double-click guard has passed)
    host.execute("require('tradeui').state.last_offer = nil").unwrap();
    hover_item(&host, &bridge, &mut ctx, "##bags", "All##all_1");
    click(&host, &bridge, &mut ctx, dear_imgui_rs::MouseButton::Left);
    assert_eq!(queued().len(), 3);
}

#[test]
fn a9_trade_mod_takes_its_windows_again_when_the_switch_was_turned_off_and_on() {
    let (host, bridge, mut ctx, _vm) = start(TRADE);
    let kinds = [TakeoverKind::TradeAsk, TakeoverKind::Trade, TakeoverKind::DuelInvite, TakeoverKind::Duel];
    // the player's switch releases every claim, with no zone and no event the mod sees
    window_takeover::release_all();
    for k in kinds {
        assert!(!window_takeover::is_taken(k));
    }
    frames(&host, &bridge, &mut ctx, 60);
    assert!(!window_takeover::is_taken(TakeoverKind::Trade), "control: within the first 120 frames nothing is retried");
    frames(&host, &bridge, &mut ctx, 70);
    for k in kinds {
        assert!(window_takeover::is_taken(k), "taken again within 120 frames: {k:?}");
    }
}

#[test]
fn a9_trade_mod_buttons_work_in_the_middle_and_empty_enter_and_double_clicks_offer_nothing_extra() {
    let (host, bridge, mut ctx, vm) = start(TRADE);
    native_a9::publish(View::Trade, Some(open_state(false)), None);
    hold(vm, TakeoverKind::Trade);
    frames(&host, &bridge, &mut ctx, 3);
    // every click point of Offer (row 2) and of All (row 1), across the drawn rectangle: centre and four corners
    for (label, rows_left) in [("Offer##offer_2", 5usize), ("All##all_1", 5usize)] {
        let scan = hovered_points(&host, &bridge, &mut ctx, "##bags", label);
        let points = click_points(&host, &bridge, &mut ctx, &scan, label);
        assert_eq!(points.len(), rows_left);
        for (n, (x, y)) in points.into_iter().enumerate() {
            host.execute("require('tradeui').state.last_offer = nil").unwrap();
            let before = queued().len();
            ctx.io_mut().add_mouse_pos_event([x, y]);
            frames(&host, &bridge, &mut ctx, 1);
            click(&host, &bridge, &mut ctx, dear_imgui_rs::MouseButton::Left);
            assert_eq!(queued().len(), before + 1, "{label}: a click at point {n} ({x:.0}, {y:.0}) of its rectangle offers");
        }
    }
    // negative control: a full-line row drawn first and a button after it (what the window used to do) leaves the
    // button only a thin strip, which the same measurement rejects
    host.execute(
        r#"
        eqoa.events.on("render", function()
            local ui = eqoa.imgui
            ui.SetNextWindowPos(500, 500, ImGuiCond.Always)
            ui.SetNextWindowSize(300, 0, ImGuiCond.Always)
            if ui.Begin("ctl##eqoa_ctl", nil, ImGuiWindowFlags.NoSavedSettings) then
                ui.Selectable("row##ctl_row", false)
                ui.SameLine()
                ui.Button("Btn##ctl_btn")
            end
            ui.End()
        end)
        "#,
    )
    .unwrap();
    frames(&host, &bridge, &mut ctx, 3);
    let covered = hovered_points(&host, &bridge, &mut ctx, "eqoa_ctl", "Btn##ctl_btn").hits;
    let ys: Vec<f32> = covered.iter().map(|p| p.1).collect();
    let height = if ys.is_empty() { 0.0 } else { ys.iter().cloned().fold(f32::MIN, f32::max) - ys.iter().cloned().fold(f32::MAX, f32::min) };
    assert!(height < 10.0, "control: a button under a full-line row is hovered over only a strip ({height:.0}px)");
    ctx.io_mut().add_mouse_pos_event([5.0, 5.0]);
    frames(&host, &bridge, &mut ctx, 2);

    // Enter in an emptied coin box offers nothing; Enter with digits does, and the text is not typed again
    action_broker::reset_all_for_test();
    host.execute("local S = require('tradeui').state; S.typing = true; S.money_text = ''").unwrap();
    CORE_press_enter(&host);
    assert!(queued().is_empty(), "an emptied box and Enter is not an offer of 0");
    host.execute("local S = require('tradeui').state; S.typing = true; S.money_text = '40'").unwrap();
    CORE_press_enter(&host);
    assert_eq!(queued(), vec!["trade_money"]);
    host.execute("local S = require('tradeui').state; assert(S.just_sent == '40' and S.money_text == nil)").unwrap();

    // a second offer of ANOTHER row a moment after the first (the list moved under a double click) is dropped
    action_broker::reset_all_for_test();
    host.execute(
        r#"
        local T = require('tradeui'); local t = eqoa.trade.get()
        T.state.last_offer = nil
        T.offer_row(t.bags[1], false)
        T.offer_row(t.bags[2], false) -- the next row, 0 s later
        "#,
    )
    .unwrap();
    assert_eq!(queued(), vec!["trade_add"], "the second, on another row, is ignored");
    host.execute("local T = require('tradeui'); local t = eqoa.trade.get(); T.offer_row(t.bags[1], false)").unwrap();
    assert_eq!(queued(), vec!["trade_add", "trade_add"], "the same row again (the next of a stack) is allowed");
}

#[test]
fn a9_bag_viewer_closing_the_smith_in_the_middle_of_a_run_still_tells_what_was_repaired() {
    let (host, bridge, mut ctx, _vm) = start(BV);
    native_a9::publish(View::Smith, Some(smith_state(&[(1, 50), (2, 70)], false, 500)), None);
    frames(&host, &bridge, &mut ctx, 2);
    host.execute("local R = require('repair'); local s = eqoa.blacksmith.get(); R.begin_ask(s); R.answer(s, true)").unwrap();
    frames(&host, &bridge, &mut ctx, 3);
    sent_ok(&host);
    native_a9::publish(View::Smith, Some(smith_state(&[(1, 0), (2, 70)], true, 450)), None);
    frames(&host, &bridge, &mut ctx, 2);
    // the player walks away: the smith closes with the second item not yet sent
    native_a9::publish(View::Smith, None, None);
    frames(&host, &bridge, &mut ctx, 2);
    let last: String = host.lua.load("return require('repair').last.text").eval().unwrap();
    assert!(last.starts_with("Repaired 1 item for 50 coins") && last.contains("the smith closed"), "{last}");
    assert!(window_drawn("Repair##eqoa_bag_viewer_repair_done"), "and the panel shows it for a few seconds");
}
