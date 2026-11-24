// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A9's two bundled mods: EQOA Trade (`eqoa_trade`) and the Bag Viewer's repair panel. Headless: the mods run
//! on a real host and a real ImGui context, the game's state is what `native_a9` is given, and what a mod asks for is
//! read from the action broker's queue. Each test says what its control is.
use super::{
    a1_mod_tests::{addon, context, fixture, frames, manifest, root},
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
        json!({"slot": slot, "id": id, "name": name, "quantity": qty, "worn": worn, "no_trade": false, "can_offer": !worn})
    };
    json!({
        "kind": "trade", "phase": "open", "held": true,
        "partner": {"id": 77, "name": "Bobbington the Long-Named Trader"},
        "initiated": false,
        "mine": {"items": [{"slot": 1, "id": 10, "name": "A Very Long Item Name Of Great Rarity", "quantity": 5}], "money": 120, "accepted": accepted_mine},
        "theirs": {"items": [{"slot": 1, "id": 99, "name": "Fine Steel Sword of the Northern Wastes", "quantity": 1}], "money": 5000, "accepted": false},
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
    assert!(host.drain_logs().unwrap().is_empty());
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
    assert!(host.drain_logs().unwrap().is_empty());
    assert!(bridge.drain_diagnostics().is_empty(), "no ImGui recovery diagnostics");
    assert!(queued().is_empty(), "control: a drawn trade sends and accepts nothing");

    // the right-click offer: a row that can be offered, one that is worn (the core refuses it too), and the whole stack
    host.execute("local T = require('tradeui'); local t = eqoa.trade.get(); T.offer_row(t, t.bags[2], false)").unwrap();
    assert_eq!(queued(), vec!["trade_add"]);
    host.execute("local T = require('tradeui'); local t = eqoa.trade.get(); T.offer_row(t, t.bags[3], false)").unwrap();
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
    // count the capture calls the mod makes (the client lets capture lapse after about a second)
    host.execute(
        r#"
        capture_calls, capture_on = 0, 0
        local real = eqoa.input.capture_text
        eqoa.input.capture_text = function(on)
            capture_calls = capture_calls + 1
            if on then capture_on = capture_on + 1 end
            return real(on)
        end
        "#,
    )
    .unwrap();
    // control: the box is not being typed in
    frames(&host, &bridge, &mut ctx, 5);
    let none: i64 = host.lua.load("return capture_on").eval().unwrap();
    assert_eq!(none, 0, "control: no capture while the box is idle");
    assert!(host.lua.load("return require('tradeui').state.enter == nil").eval::<bool>().unwrap(), "control: Enter is not held while idle");
    // typing: capture asked for in each of 6 frames, Enter held under its exact name
    host.execute("require('tradeui').force_typing = true").unwrap();
    frames(&host, &bridge, &mut ctx, 6);
    let on: i64 = host.lua.load("return capture_on").eval().unwrap();
    assert_eq!(on, 6, "the capture is renewed on every frame the box is wanted");
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
            json!({"slot": i + 1, "id": 100 + key, "inventory_key": key, "name": format!("Damaged Item Number {key} of Great Length"),
                "quantity": 1, "condition": 0.5, "cost": cost, "can_repair": *cost > 0, "worn": false})
        })
        .collect();
    let total: i64 = costs.iter().map(|c| c.1).sum();
    json!({"coin": coin, "repair_pending": pending, "held": false, "total": total, "items": items})
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
    assert!(host.drain_logs().unwrap().is_empty());
    assert!(bridge.drain_diagnostics().is_empty());
    // one click repairs one item
    host.execute("local R = require('repair'); R.repair_one(R.damaged(eqoa.blacksmith.get())[1])").unwrap();
    assert_eq!(queued(), vec!["blacksmith_repair"]);
    host.execute("local R = require('repair'); R.on_action_result({request_id = R.run.wait, ok = true, phase = 'completed'})").unwrap();
    action_broker::reset_all_for_test();

    // "Repair all": the question names the total and nothing is sent before the Yes (the control)
    host.execute("local R = require('repair'); R.begin_ask(eqoa.blacksmith.get())").unwrap();
    frames(&host, &bridge, &mut ctx, 3);
    assert_window_fits("Repair##eqoa_bag_viewer_repair", "repair question");
    assert!(queued().is_empty(), "the question sends nothing");
    // No ends it, still nothing
    host.execute("local R = require('repair'); R.answer(eqoa.blacksmith.get(), false)").unwrap();
    frames(&host, &bridge, &mut ctx, 2);
    assert!(queued().is_empty(), "No repairs nothing");
    // Yes: one repair goes, and no second until the server has answered the first
    host.execute("local R = require('repair'); local s = eqoa.blacksmith.get(); R.begin_ask(s); R.answer(s, true)").unwrap();
    frames(&host, &bridge, &mut ctx, 4);
    assert_eq!(queued(), vec!["blacksmith_repair"], "one at a time");
    // the answer comes: the server's read says pending, then the first item is whole
    host.execute("local R = require('repair'); R.on_action_result({request_id = R.run.wait, ok = true, phase = 'completed'})").unwrap();
    native_a9::publish(View::Smith, Some(smith_state(&[(1, 0), (2, 70), (3, 0)], true, 450)), None);
    frames(&host, &bridge, &mut ctx, 3);
    assert_eq!(queued(), vec!["blacksmith_repair"], "no second repair while the smith's last one is pending");
    native_a9::publish(View::Smith, Some(smith_state(&[(1, 0), (2, 70), (3, 0)], false, 450)), None);
    frames(&host, &bridge, &mut ctx, 3);
    assert_eq!(queued(), vec!["blacksmith_repair", "blacksmith_repair"], "the next goes after the answer");
    host.execute("local R = require('repair'); R.on_action_result({request_id = R.run.wait, ok = true, phase = 'completed'})").unwrap();
    native_a9::publish(View::Smith, Some(smith_state(&[(1, 0), (2, 0), (3, 0)], false, 380)), None);
    frames(&host, &bridge, &mut ctx, 3);
    assert_eq!(queued().len(), 2, "nothing is sent twice, and the run is over");
    let line: String = host.lua.load("return require('bags').result and require('bags').result.text or ''").eval().unwrap();
    assert!(line.starts_with("Repaired 2 items for 120 coins"), "the total is told: {line}");
    // a changed price makes the Yes ask again instead of repairing at a price the player did not see
    action_broker::reset_all_for_test();
    native_a9::publish(View::Smith, Some(smith_state(&[(1, 50)], false, 500)), None);
    frames(&host, &bridge, &mut ctx, 2);
    host.execute("local R = require('repair'); R.begin_ask(eqoa.blacksmith.get())").unwrap();
    native_a9::publish(View::Smith, Some(smith_state(&[(1, 90)], false, 500)), None);
    host.execute("local R = require('repair'); R.answer(eqoa.blacksmith.get(), true)").unwrap();
    frames(&host, &bridge, &mut ctx, 3);
    assert!(queued().is_empty(), "a Yes for a different total repairs nothing");
}

#[test]
fn a9_bag_viewer_repair_automatically_is_off_by_default_and_repairs_all_when_the_smith_opens() {
    // control: the option is off, a smith opens, nothing is sent
    let (host, bridge, mut ctx, _vm) = start(BV);
    assert!(host.lua.load("return require('repair').auto == false").eval::<bool>().unwrap(), "off by default");
    native_a9::publish(View::Smith, Some(smith_state(&[(1, 50), (2, 70)], false, 500)), None);
    frames(&host, &bridge, &mut ctx, 4);
    assert!(queued().is_empty(), "with the option off the panel waits for a click");

    // the option on: it repairs as the smith opens, with no question, one at a time, and says the total once
    let (host, bridge, mut ctx, _vm) = start(BV);
    host.execute("require('repair').auto = true").unwrap();
    native_a9::publish(View::Smith, Some(smith_state(&[(1, 50), (2, 70)], false, 500)), None);
    frames(&host, &bridge, &mut ctx, 4);
    assert_eq!(queued(), vec!["blacksmith_repair"], "the first repair goes at once, no question");
    host.execute("local R = require('repair'); R.on_action_result({request_id = R.run.wait, ok = true, phase = 'completed'})").unwrap();
    native_a9::publish(View::Smith, Some(smith_state(&[(1, 0), (2, 70)], false, 450)), None);
    frames(&host, &bridge, &mut ctx, 3);
    host.execute("local R = require('repair'); R.on_action_result({request_id = R.run.wait, ok = true, phase = 'completed'})").unwrap();
    native_a9::publish(View::Smith, Some(smith_state(&[(1, 0), (2, 0)], false, 380)), None);
    frames(&host, &bridge, &mut ctx, 3);
    assert_eq!(queued().len(), 2);
    let line: String = host.lua.load("return require('bags').result and require('bags').result.text or ''").eval().unwrap();
    assert!(line.starts_with("Repaired 2 items for 120 coins"), "{line}");
    // it does not repair again for the same visit, and a refusal stops the run with the reason
    frames(&host, &bridge, &mut ctx, 3);
    assert_eq!(queued().len(), 2);
    native_a9::publish(View::Smith, None, None);
    frames(&host, &bridge, &mut ctx, 2);
    native_a9::publish(View::Smith, Some(smith_state(&[(1, 50)], false, 10)), None);
    frames(&host, &bridge, &mut ctx, 3);
    host.execute("local R = require('repair'); R.on_action_result({request_id = R.run.wait, ok = false, code = 'not_ready', reason = 'you carry 10', phase = 'completed'})").unwrap();
    let line: String = host.lua.load("return require('bags').result.text").eval().unwrap();
    assert!(line.contains("stopped: you carry 10"), "{line}");
}
