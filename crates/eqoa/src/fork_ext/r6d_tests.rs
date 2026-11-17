// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round 6 window and quest actions, driven from Play-shaped state through the broker.
use super::*;
use crate::fork_ext::addons::{
    action_broker,
    host::Host,
    native_core::{self, CORE, Core},
    native_events,
};

fn ready() -> Play {
    CORE.with(|c| *c.borrow_mut() = Core::default());
    native_core::fixture_demand(true);
    action_broker::reset_all_for_test();
    r6a::forget();
    let mut p = super::super::tests::play();
    let _ = p.loading.wait.take();
    p.hud = Some((
        crate::ui::Vui::new(Default::default(), Default::default(), vec![]),
        Default::default(),
        crate::hud::Hud::new(),
    ));
    p.session.conn = Some(usize::MAX);
    CORE.with(|c| c.borrow_mut().view.in_world = true);
    p
}

fn rig() -> (Play, Host) {
    let p = ready();
    let host = Host::new().unwrap();
    host.execute(
        r#"
        results = {}
        eqoa.events.on("action.result", function(e) results[#results + 1] = e.data end)
        eqoa.input.bind({ id = "go", label = "Go", default_key = "k" }, function()
            called = true
            last_req, last_err = next_fn()
        end)
        "#,
    )
    .unwrap();
    (p, host)
}

fn press_k(host: &Host) {
    CORE.with(|c| {
        c.borrow_mut().emit(
            "input.key",
            serde_json::json!({"key":"k","down":true,"repeat":false,"synthetic":false,"text":""}),
        )
    });
    native_events::dispatch(host).unwrap();
}

/// Refresh the captured state, queue the action in a player gesture and dispatch it.
fn gesture(p: &mut Play, host: &Host, code: &str) {
    r6a::refresh(p, None);
    host.execute(&format!("results = {{}}; last_err = nil; last_req = nil; called = false; next_fn = function() {code} end")).unwrap();
    press_k(host);
    super::super::begin_iteration(p, true);
    native_events::dispatch(host).unwrap();
}

/// Queue in a gesture but dispatch only after `mutate` changed the game.
fn gesture_then(p: &mut Play, host: &Host, code: &str, mutate: impl FnOnce(&mut Play)) {
    r6a::refresh(p, None);
    host.execute(&format!("results = {{}}; last_err = nil; last_req = nil; called = false; next_fn = function() {code} end")).unwrap();
    press_k(host);
    mutate(p);
    super::super::begin_iteration(p, true);
    native_events::dispatch(host).unwrap();
}

fn last_ok(host: &Host) -> bool {
    host.execute("assert(results[1] ~= nil and results[1].ok == true, tostring(last_err and last_err.code or results[1] and results[1].code))").unwrap();
    true
}

/// The refusal code of the last gesture: at queue time (`last_err`) or at dispatch (the result).
fn expect_code(host: &Host, code: &str) {
    host.execute(&format!(
        "local c = (last_err and last_err.code) or (results[1] and results[1].code); assert(c == '{code}', tostring(c) .. ' req=' .. tostring(last_req) .. ' n=' .. #results .. ' msg=' .. tostring(last_err and last_err.message) .. ' called=' .. tostring(called))"
    ))
    .unwrap();
}

fn quests(p: &mut Play, titles: &[&str]) {
    p.quests.titles = titles.iter().map(|t| crate::ui::wide(t)).collect();
    p.quests.stamp = 7;
}

#[test]
fn round6d_drop_quest_sends_native_remove_and_refuses_stale_or_ungestured() {
    let (mut p, host) = rig();
    quests(&mut p, &["Bat Eyes", "The Militia"]);
    p.mode = 22;
    let sent = p.session.sends_lost;
    gesture(&mut p, &host, "return eqoa.actions.drop_quest(eqoa.quests.list()[2].ref)");
    assert!(last_ok(&host));
    assert!(p.session.sends_lost > sent, "0x007D went out through the session");

    // a ref from before the journal changed is stale at queue time
    host.execute("old = eqoa.quests.list()[1].ref").unwrap();
    quests(&mut p, &["The Militia"]);
    let sent = p.session.sends_lost;
    gesture(&mut p, &host, "return eqoa.actions.drop_quest(old)");
    expect_code(&host, "stale_handle");
    assert_eq!(p.session.sends_lost, sent);

    // a change between queue and dispatch is caught by the dispatch revalidation
    quests(&mut p, &["Bat Eyes", "The Militia"]);
    gesture_then(&mut p, &host, "return eqoa.actions.drop_quest(eqoa.quests.list()[2].ref)", |p| {
        p.quests.titles[1] = crate::ui::wide("Something Else");
    });
        expect_code(&host, "stale_handle");
    assert_eq!(p.session.sends_lost, sent);

    // outside a gesture the call is refused
    r6a::refresh(&p, None);
    host.execute("e1, e2 = eqoa.actions.drop_quest(eqoa.quests.list()[1].ref)").unwrap();
    host.execute("assert(e1 == nil and e2.code == 'needs_player_action')").unwrap();
}

#[test]
fn round6d_mastery_actions_use_native_requests_and_gates() {
    let (mut p, host) = rig();
    p.cm.m.x08 = 50;
    p.cm.skills = vec![crate::classmastery::Skill { id: 100, x00: 1, known: 1, ..Default::default() }];
    r6a::refresh(&p, None);

    // the share: exact steps only; one request at a time (the part's pending lock)
    gesture(&mut p, &host, "return eqoa.actions.mastery_share(0.555)");
    expect_code(&host, "invalid_argument");
    gesture(&mut p, &host, "return eqoa.actions.mastery_share(0.25)");
    assert!(last_ok(&host));
    assert_eq!(p.cm.pending, 1, "Part::select(25) is the native request");
    gesture(&mut p, &host, "return eqoa.actions.mastery_share(0.5)");
    expect_code(&host, "busy");

    // browse, buy and refund need the open window; buy and refund need the inspected node
    p.cm.pending = 0;
    gesture(&mut p, &host, "return eqoa.actions.mastery_browse(100)");
    expect_code(&host, "not_ready");
    p.mode = 40;
    gesture(&mut p, &host, "return eqoa.actions.mastery_buy(100)");
    expect_code(&host, "not_ready");
    gesture(&mut p, &host, "return eqoa.actions.mastery_browse(999)");
    expect_code(&host, "not_found");
    gesture(&mut p, &host, "return eqoa.actions.mastery_browse(100)");
    assert!(last_ok(&host));
    assert_eq!(p.cm.pending, 5, "Part::browse is sub-op 5");

    p.cm.pending = 0;
    p.cm.node = Some(crate::classmastery::Node { x08: 100, x05: 1, ..Default::default() });
    gesture(&mut p, &host, "return eqoa.actions.mastery_refund(100)");
    expect_code(&host, "not_ready");
    gesture(&mut p, &host, "return eqoa.actions.mastery_buy(100)");
    assert!(last_ok(&host));
    assert_eq!(p.cm.pending, 2, "Part::send02 is the purchase");
    p.cm.pending = 0;
    p.cm.skills[0].on = true;
    p.cm.node = Some(crate::classmastery::Node { x08: 100, x05: 1, x04: 1, ..Default::default() });
    gesture(&mut p, &host, "return eqoa.actions.mastery_buy(100)");
    expect_code(&host, "not_ready");
    gesture(&mut p, &host, "return eqoa.actions.mastery_refund(100)");
    assert!(last_ok(&host));
    assert_eq!(p.cm.pending, 3, "Part::send03 is the refund");
}

fn response(p: &mut Play) {
    let vui = crate::ui::Vui::new(Default::default(), Default::default(), vec![]);
    p.mode = 7;
    p.response.set_dialog(
        &vui,
        crate::dialog::DialogBox {
            id: 42,
            text: crate::ui::wide("Choose:"),
            options: (1..=4).map(|i| crate::ui::wide(&format!("Option {i}"))).collect(),
        },
    );
}

const RESP: &str = "local w = eqoa.windows.find('response_dialog'); ";

#[test]
fn round6d_window_rows_walk_native_keys_and_refuse_stale_or_bad_rows() {
    let (mut p, host) = rig();
    response(&mut p);
    gesture(&mut p, &host, &format!("{RESP} return eqoa.actions.select_row(w.ref, 3)"));
    assert!(last_ok(&host));
    assert_eq!(p.held, Some(139), "the first of two Down keys is held");
    assert_eq!(p.key_queue.iter().copied().collect::<Vec<_>>(), vec![139]);

    // a second sequence waits for the first to be delivered
    gesture(&mut p, &host, &format!("{RESP} return eqoa.actions.select_row(w.ref, 2)"));
    expect_code(&host, "busy");
    p.key_queue.clear();
    p.held = None;

    gesture(&mut p, &host, &format!("{RESP} return eqoa.actions.activate_row(w.ref, 1)"));
    assert!(last_ok(&host));
    // the first held key already moved the dialog's cursor (selection is native, immediate), so
    // the walk back is one Up, then Cross
    let mut got = vec![p.held.unwrap()];
    got.extend(p.key_queue.iter().copied());
    assert_eq!(got, vec![138, 134], "the selected row is confirmed with Cross");
    p.held = None;
    p.key_queue.clear();

    gesture(&mut p, &host, &format!("{RESP} return eqoa.actions.close_window(w.ref)"));
    assert!(last_ok(&host));
    assert_eq!(p.held, Some(132));
    p.held = None;
    response(&mut p); // the Triangle really closed it

    // 1-based visible positions; an unknown command is unsupported
    gesture(&mut p, &host, &format!("{RESP} return eqoa.actions.select_row(w.ref, 99)"));
    expect_code(&host, "out_of_range");
    gesture(&mut p, &host, &format!("{RESP} return eqoa.actions.menu_command(w.ref, 'pet.attack')"));
    expect_code(&host, "unsupported");

    // a page change between queue and dispatch makes the handle stale
    gesture_then(&mut p, &host, &format!("{RESP} return eqoa.actions.select_row(w.ref, 2)"), |p| {
        let vui = crate::ui::Vui::new(Default::default(), Default::default(), vec![]);
        p.response.set_dialog(
            &vui,
            crate::dialog::DialogBox { id: 43, text: crate::ui::wide("Other:"), options: vec![crate::ui::wide("Only")] },
        );
    });
    expect_code(&host, "stale_handle");
    assert_eq!(p.held, None);

    // a window that closed is stale too
    host.execute(&format!("{RESP} w_old = w")).unwrap();
    p.mode = 0;
    gesture(&mut p, &host, "return eqoa.actions.close_window(w_old.ref)");
    expect_code(&host, "stale_handle");

    // outside a gesture
    host.execute("e1, e2 = eqoa.actions.close_window(w_old.ref)").unwrap();
    host.execute("assert(e1 == nil and e2.code == 'needs_player_action')").unwrap();
}

#[test]
fn round6d_review_d_windows_with_unreadable_sub_state_or_pending_locks_refuse() {
    // F1/F6: the journal's boxed prompt (and the bank page, merchant picker, spellbook popup,
    // mastery focus) are private state: row, dialog and close actions are refused, never guessed
    let (mut p, host) = rig();
    quests(&mut p, &["One", "Two", "Three"]);
    p.mode = 22;
    for call in [
        "eqoa.actions.select_row(w.ref, 2)",
        "eqoa.actions.activate_row(w.ref, 1)",
        "eqoa.actions.close_window(w.ref)",
    ] {
        gesture(&mut p, &host, &format!("local w = eqoa.windows.find('quest_log'); return {call}"));
        expect_code(&host, "unsupported");
        assert_eq!(p.held, None);
        assert!(p.key_queue.is_empty());
    }
    // F6: a pending native transaction refuses close_window instead of pressing Triangle
    p.mode = 0;
    response(&mut p);
    p.waiting = true;
    gesture(&mut p, &host, &format!("{RESP} return eqoa.actions.close_window(w.ref)"));
    expect_code(&host, "busy");
    assert_eq!(p.held, None);
    p.waiting = false;
    gesture(&mut p, &host, &format!("{RESP} return eqoa.actions.close_window(w.ref)"));
    assert!(last_ok(&host));
}

#[test]
fn round6d_review_d_trainer_select_is_exact_or_refused() {
    let (mut p, host) = rig();
    let vui = crate::ui::Vui::new(Default::default(), Default::default(), vec![]);
    let mut null_r = crate::pointer::testing::NullRaster;
    p.mode = 12;
    p.trainer_open = true;
    p.run_trainer(&vui, &mut null_r);
    let tr = "local w = eqoa.windows.find('trainer'); ";
    // on the stats: Strength to Wisdom is four Down keys
    gesture(&mut p, &host, &format!("{tr} return eqoa.actions.select_row(w.ref, 5)"));
    assert!(last_ok(&host));
    let mut got = vec![p.held.unwrap()];
    got.extend(p.key_queue.iter().copied());
    assert_eq!(got, vec![139, 139, 139, 139]);
    p.held = None;
    p.key_queue.clear();
    // the buttons rows are not a target, and activation is not offered
    gesture(&mut p, &host, &format!("{tr} return eqoa.actions.select_row(w.ref, 8)"));
    expect_code(&host, "unsupported");
    gesture(&mut p, &host, &format!("{tr} return eqoa.actions.activate_row(w.ref, 1)"));
    expect_code(&host, "unsupported");
    // with the cursor on Train an Up restores the old stat, so no walk starts there
    p.trainer_keys.clear();
    for _ in 0..7 {
        p.trainer_keys.push((139, true));
        p.run_trainer(&vui, &mut null_r);
    }
    gesture(&mut p, &host, &format!("{tr} return eqoa.actions.select_row(w.ref, 5)"));
    expect_code(&host, "not_ready");
    assert_eq!(p.held, None);
}

#[test]
fn round6d_review_d_mastery_buy_refund_use_the_live_skill_list() {
    let (mut p, host) = rig();
    p.cm.m.x08 = 50;
    p.cm.skills = vec![crate::classmastery::Skill { id: 100, x00: 1, known: 1, on: true, ..Default::default() }];
    p.mode = 40;
    // the inspected node still says "not owned" after the purchase: the live list says owned
    p.cm.node = Some(crate::classmastery::Node { x08: 100, x05: 1, x04: 0, ..Default::default() });
    r6a::refresh(&p, None);
    gesture(&mut p, &host, "return eqoa.actions.mastery_buy(100)");
    expect_code(&host, "not_ready");
    assert_eq!(p.cm.pending, 0, "no duplicate purchase request");
    gesture(&mut p, &host, "return eqoa.actions.mastery_refund(100)");
    assert!(last_ok(&host));
    assert_eq!(p.cm.pending, 3);
}

#[test]
fn round6d_review_d_edit_box_replaced_and_quest_stamp_changed_are_stale() {
    let (mut p, host) = rig();
    p.open_edit(0x0B0F_0000, 128, -1, false);
    gesture_then(
        &mut p,
        &host,
        "local w = eqoa.windows.find('input'); return eqoa.actions.input_text(w.ref, 'Bob', true)",
        |p| p.edit.purpose = 0x0B04_0000,
    );
    expect_code(&host, "stale_handle");
    assert!(p.edit.text.is_empty());
    assert_eq!(p.mode, 2);

    // the journal's stamp moved with the same titles before dispatch
    quests(&mut p, &["Bat Eyes", "The Militia"]);
    let sent = p.session.sends_lost;
    gesture_then(&mut p, &host, "return eqoa.actions.drop_quest(eqoa.quests.list()[2].ref)", |p| p.quests.stamp = 8);
    expect_code(&host, "stale_handle");
    assert_eq!(p.session.sends_lost, sent);
}

#[test]
fn round6d_review_d_text_dialog_done_and_report_confirm_refused_at_queue_time() {
    let (mut p, host) = rig();
    p.mode = 9;
    p.priv_chat.set_text(&crate::ui::wide("Hail!"));
    gesture(&mut p, &host, "local w = eqoa.windows.find('text_dialog'); return eqoa.actions.dialog_answer(w.ref, 1)");
    assert!(last_ok(&host));
    assert_eq!(p.held, Some(134), "Done is Cross");
    p.held = None;
    p.mode = 9; // Cross really closed it; open it again
    p.priv_chat.set_text(&crate::ui::wide("Hail!"));
    gesture(&mut p, &host, "local w = eqoa.windows.find('text_dialog'); return eqoa.actions.dialog_answer(w.ref, 2)");
    expect_code(&host, "out_of_range");

    p.mode = 14;
    p.mbox.prompt(&crate::ui::wide("Send report?"), 1, 0, 0x0B0B_1002, 0);
    gesture(&mut p, &host, "local w = eqoa.windows.find('confirm'); return eqoa.actions.dialog_answer(w.ref, 1)");
    host.execute("assert(last_err and last_err.code == 'permission_denied' and results[1] == nil, 'refused before queueing')").unwrap();
}

#[test]
fn round6d_dialog_answer_walks_the_confirm_and_the_report_guard_holds() {
    let (mut p, host) = rig();
    p.mode = 14;
    p.mbox.prompt(&crate::ui::wide("Sure?"), 1, 0, 0, 0);
    gesture(&mut p, &host, "local w = eqoa.windows.find('confirm'); sel = w.dialog.selection; return eqoa.actions.dialog_answer(w.ref, 1)");
    assert!(last_ok(&host));
    host.execute("assert(sel == 2, 'a yes/no box starts on No')").unwrap();
    // from No the answer Yes is Left then Enter
    let want: Vec<i32> = vec![136, 134];
    let mut got = vec![p.held.unwrap()];
    got.extend(p.key_queue.iter().copied());
    assert_eq!(got, want);
    p.held = None;
    p.key_queue.clear();

    gesture(&mut p, &host, "local w = eqoa.windows.find('confirm'); return eqoa.actions.dialog_answer(w.ref, 3)");
    expect_code(&host, "out_of_range");

    // the report confirmation never takes synthetic input
    p.mbox.p2 = 0x0B0B_1002;
    gesture(&mut p, &host, "local w = eqoa.windows.find('confirm'); return eqoa.actions.dialog_answer(w.ref, 1)");
    expect_code(&host, "permission_denied");
    assert_eq!(p.held, None);
    gesture(&mut p, &host, "local w = eqoa.windows.find('confirm'); return eqoa.actions.close_window(w.ref)");
    expect_code(&host, "permission_denied");
    assert_eq!(p.held, None);

    // the support window is refused already at queue time
    p.mbox.p2 = 0;
    p.mode = 31;
    gesture(&mut p, &host, "local w = eqoa.windows.find('support_request'); return eqoa.actions.close_window(w.ref)");
    expect_code(&host, "permission_denied");
}

#[test]
fn round6d_input_text_types_through_the_editor_and_guards_reports_and_passwords() {
    let (mut p, host) = rig();
    p.open_edit(0x0B0F_0000, 128, -1, false);
    gesture(&mut p, &host, "local w = eqoa.windows.find('input'); return eqoa.actions.input_text(w.ref, 'hello there')");
    assert!(last_ok(&host));
    assert_eq!(String::from_utf16_lossy(&p.edit.text), "hello there");
    assert_eq!(p.mode, 2, "not submitted");

    // characters the game's keyboard cannot type are refused, the old text stays
    gesture(&mut p, &host, "local w = eqoa.windows.find('input'); return eqoa.actions.input_text(w.ref, 'caf\u{e9}')");
    expect_code(&host, "invalid_argument");
    assert_eq!(String::from_utf16_lossy(&p.edit.text), "hello there");

    // the report routes, also wrapped in a tell, never reach the editor
    for text in ["/bug my note", "#BUG", "/tell Bob /bug x"] {
        gesture(&mut p, &host, &format!("local w = eqoa.windows.find('input'); return eqoa.actions.input_text(w.ref, '{text}', true)"));
        expect_code(&host, "permission_denied");
        assert_eq!(String::from_utf16_lossy(&p.edit.text), "hello there");
    }

    // submit is the player's edit_done: the chat line goes out and the editor closes
    let sent = p.session.sends_lost;
    gesture(&mut p, &host, "local w = eqoa.windows.find('input'); return eqoa.actions.input_text(w.ref, 'hi all', true)");
    assert!(last_ok(&host));
    assert_eq!(p.mode, 0);
    assert!(p.session.sends_lost > sent);

    // password fields and the report editor are refused
    p.open_edit(0x0B0F_0000, 128, -1, false);
    p.edit.password = true;
    gesture(&mut p, &host, "local w = eqoa.windows.find('input'); return eqoa.actions.input_text(w.ref, 'secret')");
    expect_code(&host, "permission_denied");
    assert!(p.edit.text.is_empty());
    p.edit.password = false;
    p.edit.purpose = 0x0B0B_1002;
    gesture(&mut p, &host, "local w = eqoa.windows.find('input'); return eqoa.actions.input_text(w.ref, 'note')");
    expect_code(&host, "permission_denied");
    assert!(p.edit.text.is_empty());
}

#[test]
fn round6d_capabilities_are_supported_for_what_is_wired() {
    let (_p, host) = rig();
    host.execute(
        r#"
        for _, n in ipairs({"drop_quest","mastery_share","mastery_browse","mastery_buy","mastery_refund","close_window","select_row","activate_row","dialog_answer","input_text","menu_command"}) do
            local c = eqoa.capabilities.get("eqoa.actions." .. n)
            assert(c and c.status == "supported", n)
        end
        local b = eqoa.capabilities.get("eqoa.actions.buttons")
        assert(b and b.status ~= "supported")
        "#,
    )
    .unwrap();
}
