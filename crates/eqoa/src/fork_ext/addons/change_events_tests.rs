// Copyright LoopRook. SPDX-License-Identifier: AGPL-3.0-or-later
//! The change events of the systems that were poll-only (`craft.changed`, `mail.changed`, `auction.changed`,
//! `raid.changed`, `factions.changed`) and the raid ready-check and loot-roll sessions (`raid_ready`, `raid_roll`).
//! None of these needs a graphics adapter or the disc: every test here runs on every machine (no SKIP path).
//!
//! Each test names its negative control (what it would see without the change). The controls were run red by
//! temporarily removing the change, as the report says.
use super::{
    action_broker, change_events,
    host::Host,
    native_a10, native_a5, native_a8,
    native_core::{self, CORE, Core},
    native_events,
    raid_strips::{self, LootRollEntry, RaidMemberSnap, RaidSnapshot, ReadyCheckSnap},
    window_sessions as ws,
    window_takeover::{self, TakeoverKind, TakeoverScope},
};
use serde_json::{Value, json};

fn fresh() {
    CORE.with(|c| *c.borrow_mut() = Core::default());
    native_core::fixture_demand(true);
    action_broker::reset_all_for_test();
    ws::reset_for_test();
    window_takeover::reset_all_for_test();
    window_takeover::forget_switch_for_test();
    change_events::reset_for_test();
    native_a8::clear();
    native_a10::clear_mail();
    native_a10::clear_auction();
    native_a5::reset_for_test();
    raid_strips::reset();
    CORE.with(|c| c.borrow_mut().view.in_world = true);
}

const CAPS: &[&str] = &[
    "eqoa.events.on",
    "eqoa.windows.session",
    "eqoa.craft.get",
    "eqoa.mail.get",
    "eqoa.auction.get",
    "eqoa.raid.get",
    "eqoa.factions.list",
];

/// A mod that listens to the five events (and to the raid sessions) and writes down what it hears.
fn listener() -> Host {
    let h = Host::with_test_capabilities(CAPS).unwrap();
    h.execute(
        r#"
        evs, sess = {}, {}
        for _, n in ipairs({"craft.changed", "mail.changed", "auction.changed", "raid.changed", "factions.changed"}) do
            eqoa.events.on(n, function(e) evs[#evs + 1] = n .. ":" .. tostring(e.data.open) .. ":" .. tostring(e.data.revision) end)
        end
        eqoa.events.on("windows.session", function(e)
            local d = e.data
            sess[#sess + 1] = d.kind .. ":" .. d.phase .. ":" .. tostring(d.reason) .. ":" .. tostring(d.opened_from)
        end)
        "#,
    )
    .unwrap();
    h
}

fn vm_of(h: &Host) -> u64 {
    native_events::bus(&h.lua).borrow().vm
}

/// One game step: the writes of the step are over, the client flushes, the mod's handlers run.
fn step(h: &Host) {
    change_events::flush();
    native_events::dispatch(h).unwrap();
}

fn heard(h: &Host) -> String {
    h.lua.load("return table.concat(evs, ' ')").eval::<String>().unwrap()
}

fn clear_heard(h: &Host) {
    h.execute("evs = {}; sess = {}").unwrap();
}

fn sessions(h: &Host) -> String {
    h.lua.load("return table.concat(sess, ' ')").eval::<String>().unwrap()
}

// ---- crafting, mail, the auction: written by the client's reading of the game's state --------------------------------

fn craft(n: u32) -> Value {
    json!({"revision": 0, "pending": "none", "recipes": [{"id": n}], "notice": null})
}

/// `craft`, `mail` and `auction` share one shape and one way of being told: each system's publish and clear.
type Publish = fn(Value);
type Clear = fn();

fn each_system() -> Vec<(&'static str, Publish, Clear, fn(u32) -> Value)> {
    fn body(n: u32) -> Value {
        json!({"revision": 0, "entries": [{"id": n}]})
    }
    vec![
        ("craft", native_a8::publish as Publish, native_a8::clear as Clear, craft as fn(u32) -> Value),
        ("mail", native_a10::publish_mail, native_a10::clear_mail, body),
        ("auction", native_a10::publish_auction, native_a10::clear_auction, body),
    ]
}

/// A change fires exactly one event in its step, however many writes the step had; the same state again fires none;
/// the state going fires one with `open = false`. Negative control: with the `note` call left out of the publish
/// (or the clear), the first assertion of that system finds an empty list.
#[test]
fn craft_mail_auction_one_event_per_changed_step_and_none_for_no_change() {
    for (name, publish, clear, make) in each_system() {
        fresh();
        let h = listener();
        publish(make(1));
        step(&h);
        let first = heard(&h);
        assert!(first.starts_with(&format!("{name}.changed:true:")), "{name}: {first}");
        assert_eq!(first.matches(".changed").count(), 1, "{name}: one event: {first}");
        // no change: the same state again
        clear_heard(&h);
        publish(make(1));
        step(&h);
        assert_eq!(heard(&h), "", "{name}: an unchanged state fires nothing");
        // two writes in one step: one event, with the later revision
        publish(make(2));
        publish(make(3));
        step(&h);
        let both = heard(&h);
        assert_eq!(both.matches(".changed").count(), 1, "{name}: two writes, one event: {both}");
        // an empty step after it fires nothing, either
        clear_heard(&h);
        step(&h);
        assert_eq!(heard(&h), "", "{name}: nothing more");
        // the state going: one event, open = false, with the last revision
        clear();
        clear();
        step(&h);
        let gone = heard(&h);
        assert!(gone.starts_with(&format!("{name}.changed:false:")), "{name}: {gone}");
        assert_eq!(gone.matches(".changed").count(), 1, "{name}: {gone}");
    }
}

/// The event carries the revision the read returns. Negative control: a revision of 0 (the placeholder the state is
/// published with) would be what a mod hears if the event were built from the published value, not the assigned one.
#[test]
fn craft_mail_auction_event_revision_is_the_reads_revision() {
    fresh();
    let h = listener();
    native_a8::publish(craft(1));
    native_a10::publish_mail(json!({"revision": 0, "entries": []}));
    native_a10::publish_auction(json!({"revision": 0, "results": []}));
    step(&h);
    h.execute(
        r#"
        assert(eqoa.craft.get().revision >= 1)
        local want = {}
        want['craft.changed'] = eqoa.craft.get().revision
        want['mail.changed'] = eqoa.mail.get().revision
        want['auction.changed'] = eqoa.auction.get().revision
        local seen = 0
        for _, line in ipairs(evs) do
            local name, rev = line:match('^(%S-):%a+:(%d+)$')
            assert(want[name] == tonumber(rev), line .. ' vs ' .. tostring(want[name]))
            seen = seen + 1
        end
        assert(seen == 3, 'three events: ' .. table.concat(evs, ' '))
        "#,
    )
    .unwrap();
}

/// A mod that does not listen costs nothing: nothing is kept for the step, no event is made, and the read still
/// works. Negative control: with `note`'s `wanted` test removed, `pending_for_test` is true after the write.
#[test]
fn craft_mail_auction_a_mod_that_does_not_listen_costs_nothing() {
    for (name, publish, clear, make) in each_system() {
        fresh();
        let h = Host::with_test_capabilities(CAPS).unwrap();
        let before = CORE.with(|c| c.borrow().seq);
        publish(make(1));
        assert!(!change_events::pending_for_test(), "{name}: nothing is kept for a mod that listens to nothing");
        step(&h);
        clear();
        assert!(!change_events::pending_for_test(), "{name}: nor for the close");
        step(&h);
        assert_eq!(CORE.with(|c| c.borrow().seq), before, "{name}: no event was made");
    }
}

/// Told after a zone change: a world change drops what was pending, and the next change of the new world is told.
/// Negative control: with `change_events::world_changed` left out of `release_all`, the pending event of the old
/// world is told in the new one (the second assertion).
#[test]
fn craft_mail_auction_events_arrive_after_a_zone_change() {
    for (name, publish, _clear, make) in each_system() {
        fresh();
        let h = listener();
        publish(make(1));
        // the world changes in the same step, before the flush
        window_takeover::release_all();
        step(&h);
        assert_eq!(heard(&h), "", "{name}: the old world's change is not told in the new one");
        publish(make(2));
        step(&h);
        let after = heard(&h);
        assert!(after.starts_with(&format!("{name}.changed:true:")), "{name}: {after}");
    }
}

// ---- the faction page --------------------------------------------------------------------------------------------------

fn page(standing: i32) -> Value {
    json!({"page": 1, "pages": 1, "entries": [{"name": "Guards", "standing": standing}]})
}

/// A page that arrives fires one event; the same page again (a new key, same content) and an unchanged key fire none;
/// a changed standing fires one. Negative control: with the comparison removed, the same-content page fires.
#[test]
fn factions_one_event_for_a_changed_page_none_for_the_same_one() {
    fresh();
    let h = listener();
    native_a5::set_factions(1, || None);
    step(&h);
    assert_eq!(heard(&h), "", "no page, nothing changed");
    native_a5::set_factions(2, || Some(page(10)));
    native_a5::set_factions(3, || Some(page(11)));
    step(&h);
    let arrived = heard(&h);
    assert_eq!(arrived.matches("factions.changed:true:").count(), 1, "two writes, one event: {arrived}");
    clear_heard(&h);
    native_a5::set_factions(3, || Some(page(99)));
    native_a5::set_factions(4, || Some(page(11)));
    step(&h);
    assert_eq!(heard(&h), "", "the same page under a new key, and the same key again: nothing changed");
    native_a5::set_factions(5, || Some(page(12)));
    step(&h);
    assert_eq!(heard(&h).matches("factions.changed:true:").count(), 1);
    // the zone changes: the new world has no page, and its first page is told
    window_takeover::release_all();
    clear_heard(&h);
    CORE.with(|c| c.borrow_mut().generation += 1);
    native_a5::enter_generation();
    native_a5::set_factions(6, || Some(page(12)));
    step(&h);
    assert_eq!(heard(&h).matches("factions.changed:true:").count(), 1, "told after a zone change: {}", heard(&h));
}

/// A mod that does not listen: no revision is spent and nothing is kept. Negative control: with the `wanted` test
/// removed from `set_factions`, the counter moves.
#[test]
fn factions_a_mod_that_does_not_listen_costs_nothing() {
    fresh();
    let h = Host::with_test_capabilities(CAPS).unwrap();
    native_a5::set_factions(1, || Some(page(10)));
    assert!(!change_events::pending_for_test());
    assert_eq!(change_events::next_revision(change_events::Sys::Factions), 1, "no revision was spent");
    step(&h);
    h.execute("assert(eqoa.factions.list ~= nil)").unwrap();
}

// ---- the raid ----------------------------------------------------------------------------------------------------------

fn member(id: u32, hp: f32) -> RaidMemberSnap {
    RaidMemberSnap {
        id,
        name: format!("Member{id}"),
        hp_fraction: hp,
        power_fraction: 1.0,
        group: 1,
        merc: false,
        leader: id == 1,
        helper: false,
        assist: false,
    }
}

fn raid(members: Vec<RaidMemberSnap>) -> RaidSnapshot {
    RaidSnapshot { in_raid: true, loot_rule: "free_for_all", members, now_ms: 1000, ..Default::default() }
}

fn ready(id: u32, deadline_ms: i64) -> ReadyCheckSnap {
    ReadyCheckSnap { id, asker: "Leader".into(), seconds_remaining: 10.0, duration: 30.0, deadline_ms }
}

fn roll(id: u32, deadline_ms: i64) -> LootRollEntry {
    LootRollEntry {
        id,
        item_name: format!("Item{id}"),
        item_id: id.to_string(),
        icon: Some(5),
        may_need: true,
        seconds_remaining: 10.0,
        duration: 30.0,
        deadline_ms,
    }
}

/// The first look is a base line; a joined member fires one event in its step; the same roster fires none; a health bar
/// moving fires none (the roster is what `raid.get` is; the bars are `group.changed`'s); a ready check coming and going
/// fires. Negative control: with the key's comparison left out, the hp-only step fires.
#[test]
fn raid_one_event_for_a_change_of_the_roster_and_none_for_a_bar() {
    fresh();
    let h = listener();
    raid_strips::update(raid(vec![member(1, 1.0)]));
    step(&h);
    assert_eq!(heard(&h), "", "the first look is a base line, not a change");
    raid_strips::update(raid(vec![member(1, 1.0), member(2, 1.0)]));
    raid_strips::update(raid(vec![member(1, 1.0), member(2, 1.0), member(3, 1.0)]));
    step(&h);
    let joined = heard(&h);
    assert_eq!(joined.matches("raid.changed:true:").count(), 1, "two writes of one step, one event: {joined}");
    clear_heard(&h);
    raid_strips::update(raid(vec![member(1, 1.0), member(2, 1.0), member(3, 1.0)]));
    step(&h);
    assert_eq!(heard(&h), "", "the same roster: nothing");
    raid_strips::update(raid(vec![member(1, 0.4), member(2, 0.7), member(3, 0.1)]));
    step(&h);
    assert_eq!(heard(&h), "", "health bars alone are not a change of the raid");
    let mut with_check = raid(vec![member(1, 0.4), member(2, 0.7), member(3, 0.1)]);
    with_check.ready = Some(ready(7, 9000));
    raid_strips::update(with_check);
    step(&h);
    assert_eq!(heard(&h).matches("raid.changed:true:").count(), 1, "a ready check came: {}", heard(&h));
    // the seconds left moving is not a change either
    let mut later = raid(vec![member(1, 0.4), member(2, 0.7), member(3, 0.1)]);
    later.ready = Some(ReadyCheckSnap { seconds_remaining: 3.0, ..ready(7, 9000) });
    clear_heard(&h);
    raid_strips::update(later);
    step(&h);
    assert_eq!(heard(&h), "", "seconds left is not a change");
    // the raid ends: one event, `open` false (not in a raid, nothing up)
    clear_heard(&h);
    raid_strips::update(RaidSnapshot { now_ms: 2000, ..Default::default() });
    step(&h);
    assert_eq!(heard(&h).matches("raid.changed:false:").count(), 1, "{}", heard(&h));
}

/// A mod that does not listen: no key is hashed and no base line is kept. Negative control: with `wanted` removed from
/// `update`, `change_base_for_test` is set.
#[test]
fn raid_a_mod_that_does_not_listen_costs_nothing() {
    fresh();
    let h = Host::with_test_capabilities(CAPS).unwrap();
    raid_strips::update(raid(vec![member(1, 1.0)]));
    raid_strips::update(raid(vec![member(1, 1.0), member(2, 1.0)]));
    assert!(!raid_strips::change_base_for_test(), "no roster was hashed for a mod that does not listen");
    assert!(!change_events::pending_for_test());
    step(&h);
    // the read still works
    h.execute("assert(eqoa.raid.get ~= nil)").unwrap();
}

/// Told after a zone change and a restart of the server's world: the raid state resets with the world (the base line
/// too), and the first change of the new world is told. Negative control: with `change_base` not reset, the first
/// look of the new world would be compared with the old one and fire at once (the first assertion).
#[test]
fn raid_events_arrive_after_a_zone_change() {
    fresh();
    let h = listener();
    raid_strips::update(raid(vec![member(1, 1.0), member(2, 1.0)]));
    step(&h);
    // the world changes: the old raid is gone
    window_takeover::release_all();
    raid_strips::reset();
    raid_strips::update(RaidSnapshot { now_ms: 5, ..Default::default() });
    step(&h);
    assert_eq!(heard(&h), "", "the new world's first look is a base line");
    raid_strips::update(raid(vec![member(9, 1.0)]));
    step(&h);
    assert_eq!(heard(&h).matches("raid.changed:true:").count(), 1, "told after a zone change: {}", heard(&h));
}

// ---- the raid strips as sessions ---------------------------------------------------------------------------------------

fn hold(h: &Host, kind: TakeoverKind) {
    window_takeover::takeover(vm_of(h), "raidtest", kind, TakeoverScope::All).unwrap();
}

fn with_ready(id: u32, deadline: i64, now: i64) -> RaidSnapshot {
    RaidSnapshot { ready: Some(ready(id, deadline)), now_ms: now, ..Default::default() }
}

/// The ready check as a session: opened with its data, readable by `eqoa.windows.session`, changed when the check
/// does, ended `answered` when the mod answered, `timeout` when its timer ran out, `server` when it went early, and
/// `released` when the claim went. Negative control: with `sync_sessions` not called from `update`, no session opens.
#[test]
fn raid_ready_check_is_a_session_with_its_data_and_its_end_reasons() {
    fresh();
    let h = listener();
    hold(&h, TakeoverKind::RaidReady);
    raid_strips::update(with_ready(7, 9000, 1000));
    step(&h);
    assert_eq!(sessions(&h), "raid_ready:opened:nil:request");
    h.execute(
        r#"
        local s = assert(eqoa.windows.session('raid_ready'))
        assert(s.kind == 'raid_ready' and s.origin == 'server' and s.opened_from == 'request')
        assert(s.data.id == 7 and s.data.asker == 'Leader' and s.data.duration == 30 and s.data.deadline_ms == 9000)
        assert(s.data.choices[1] == 'ready' and s.data.choices[2] == 'not_ready')
        assert(s.data.seconds_remaining == nil, 'the clock is not in the data')
        assert(s.data.rolls == nil and s.data.text == nil, 'fields of other kinds read nil, with no diagnostic')
        "#,
    )
    .unwrap();
    // the seconds moving changes nothing; a second check is the same session with new data
    clear_heard(&h);
    raid_strips::update(RaidSnapshot { ready: Some(ReadyCheckSnap { seconds_remaining: 2.0, ..ready(7, 9000) }), now_ms: 8000, ..Default::default() });
    step(&h);
    assert_eq!(sessions(&h), "", "the seconds left are not data");
    raid_strips::update(with_ready(8, 20000, 8100));
    step(&h);
    assert_eq!(sessions(&h), "raid_ready:changed:nil:request");
    // the mod answers: the session ends `answered`
    clear_heard(&h);
    raid_strips::note_answered(TakeoverKind::RaidReady);
    raid_strips::update(RaidSnapshot { now_ms: 8200, ..Default::default() });
    step(&h);
    assert_eq!(sessions(&h), "raid_ready:closed:answered:request");
    h.execute("assert(eqoa.windows.session('raid_ready') == nil)").unwrap();

    // its own timer ran out
    clear_heard(&h);
    raid_strips::update(with_ready(9, 9000, 1000));
    step(&h);
    raid_strips::update(RaidSnapshot { now_ms: 9001, ..Default::default() });
    step(&h);
    assert_eq!(sessions(&h), "raid_ready:opened:nil:request raid_ready:closed:timeout:request");
    // the server took it away early
    clear_heard(&h);
    raid_strips::update(with_ready(10, 90000, 1000));
    step(&h);
    raid_strips::update(RaidSnapshot { now_ms: 2000, ..Default::default() });
    step(&h);
    assert_eq!(sessions(&h), "raid_ready:opened:nil:request raid_ready:closed:server:request");
    // the claim is released while a check is up
    clear_heard(&h);
    raid_strips::update(with_ready(11, 90000, 1000));
    step(&h);
    assert!(window_takeover::release(vm_of(&h), TakeoverKind::RaidReady));
    raid_strips::update(with_ready(11, 90000, 1100));
    step(&h);
    assert_eq!(sessions(&h), "raid_ready:opened:nil:request raid_ready:closed:released:request");
    // `mod_window_up` is not touched by a strip: the game does not hold its other windows for it
    assert!(!crate::fork_ext::a11_play::mod_window_up());
}

/// With no claim there is no session; a claim taken after the check came opens one for the check that is up.
/// Negative control: with the holder test removed, a session would open for a mod that holds nothing.
#[test]
fn raid_ready_check_without_a_claim_is_no_session() {
    fresh();
    let h = listener();
    raid_strips::update(with_ready(7, 9000, 1000));
    step(&h);
    assert_eq!(sessions(&h), "");
    h.execute("assert(eqoa.windows.session('raid_ready') == nil)").unwrap();
    hold(&h, TakeoverKind::RaidReady);
    raid_strips::update(with_ready(7, 9000, 1100));
    step(&h);
    assert_eq!(sessions(&h), "raid_ready:opened:nil:request");
}

/// The loot rolls as one session: every roll up is in its `rolls`; one added or answered is `changed`; the last one
/// answered ends it `answered`. The data of a session roll has no `seconds_remaining`.
#[test]
fn raid_roll_is_a_session_of_every_roll_up() {
    fresh();
    let h = listener();
    hold(&h, TakeoverKind::RaidRoll);
    raid_strips::update(RaidSnapshot { rolls: vec![roll(1, 5000)], now_ms: 1000, ..Default::default() });
    step(&h);
    assert_eq!(sessions(&h), "raid_roll:opened:nil:request");
    h.execute(
        r#"
        local s = assert(eqoa.windows.session('raid_roll'))
        assert(#s.data.rolls == 1)
        local r = s.data.rolls[1]
        assert(r.id == 1 and r.item_name == 'Item1' and r.may_need == true and r.deadline_ms == 5000 and r.icon == 5)
        assert(r.choices[1] == 'need' and r.choices[3] == 'pass')
        assert(r.seconds_remaining == nil)
        assert(s.data.id == nil and s.data.asker == nil)
        "#,
    )
    .unwrap();
    clear_heard(&h);
    raid_strips::update(RaidSnapshot { rolls: vec![roll(1, 5000), roll(2, 6000)], now_ms: 1100, ..Default::default() });
    step(&h);
    assert_eq!(sessions(&h), "raid_roll:changed:nil:request");
    // one answered: the session stays, changed
    clear_heard(&h);
    raid_strips::note_answered(TakeoverKind::RaidRoll);
    raid_strips::update(RaidSnapshot { rolls: vec![roll(2, 6000)], now_ms: 1200, ..Default::default() });
    step(&h);
    assert_eq!(sessions(&h), "raid_roll:changed:nil:request");
    // the last one answered: it ends answered
    clear_heard(&h);
    raid_strips::note_answered(TakeoverKind::RaidRoll);
    raid_strips::update(RaidSnapshot { now_ms: 1300, ..Default::default() });
    step(&h);
    assert_eq!(sessions(&h), "raid_roll:closed:answered:request");
}

/// A world change ends a strip session `zoning` and does not open it again for the check that is still up; the next
/// check does. Negative control: without the `blocked` flag, the same check would open a new session the next step.
#[test]
fn raid_strip_session_ends_with_the_world_and_is_not_reopened_for_the_same_check() {
    fresh();
    let h = listener();
    hold(&h, TakeoverKind::RaidReady);
    raid_strips::update(with_ready(7, 90000, 1000));
    step(&h);
    // the player's switch ends every mod window at once
    ws::end_all("game_windows");
    window_takeover::reset_all_for_test();
    hold(&h, TakeoverKind::RaidReady);
    raid_strips::update(with_ready(7, 90000, 1100));
    raid_strips::update(with_ready(7, 90000, 1200));
    step(&h);
    assert_eq!(sessions(&h), "raid_ready:opened:nil:request raid_ready:closed:game_windows:request");
    // a different check is a new opening
    raid_strips::update(RaidSnapshot { now_ms: 1300, ..Default::default() });
    raid_strips::update(with_ready(8, 90000, 1400));
    step(&h);
    assert!(sessions(&h).ends_with("raid_ready:opened:nil:request"), "{}", sessions(&h));
    // a zone change ends it `zoning`
    window_takeover::release_all();
    step(&h);
    assert!(sessions(&h).ends_with("raid_ready:closed:zoning:request"), "{}", sessions(&h));
}

/// The strips cannot be passed or closed by a call; they are not a view that hides nothing.
#[test]
fn raid_strip_sessions_cannot_be_passed() {
    fresh();
    let h = listener();
    hold(&h, TakeoverKind::RaidReady);
    raid_strips::update(with_ready(7, 90000, 1000));
    step(&h);
    let id = ws::sessions_for_test()[0].id.clone();
    let err = ws::pass(vm_of(&h), &id).unwrap_err();
    assert_eq!(err.0, "unsupported");
    assert!(err.1.contains("release"), "{}", err.1);
    let err = ws::close(vm_of(&h), &id, "closed").unwrap_err();
    assert_eq!(err.0, "unsupported");
}

/// The window flags and conditions the spec lists are the ones the bridge installs, with ImGui's own values (spec
/// section 5, "Window flags and conditions"). Negative control: a name or value in the spec table that the tables do
/// not have is the failure.
#[test]
fn imgui_window_flags_and_conditions_are_the_ones_the_spec_lists() {
    let h = Host::with_test_capabilities(&[]).unwrap();
    let _bridge = super::bridge::Bridge::install(&h).unwrap();
    let spec = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/modding/EQOA-API-SPEC.md"))
        .unwrap()
        .replace("\r\n", "\n");
    let from = spec.find("**Window flags and conditions.**").expect("the spec has the section");
    let to = spec[from..].find("A draw list from").expect("and its end") + from;
    let section = &spec[from..to];
    let mut in_spec = std::collections::BTreeMap::new();
    for line in section.lines().filter(|l| l.starts_with("| `")) {
        let cells: Vec<&str> = line.trim().trim_matches('|').split('|').map(str::trim).collect();
        let pairs: &[(usize, &str)] = if cells.len() == 4 { &[(0, "flags"), (2, "flags")] } else { &[(0, "cond")] };
        for (i, kind) in pairs {
            let (name, value) = (cells[*i].trim_matches('`'), cells[i + 1]);
            if let Ok(value) = value.parse::<i64>() {
                in_spec.insert(format!("{}.{name}", if *kind == "flags" { "ImGuiWindowFlags" } else { "ImGuiCond" }), value);
            }
        }
    }
    let have: std::collections::BTreeMap<String, i64> = h
        .lua
        .load(
            r#"
            local out = {}
            for _, g in ipairs({'ImGuiWindowFlags', 'ImGuiCond'}) do
                for k, v in pairs(_G[g]) do out[#out + 1] = {g .. '.' .. k, v} end
            end
            return out
            "#,
        )
        .eval::<Vec<(String, i64)>>()
        .unwrap()
        .into_iter()
        .collect();
    assert_eq!(in_spec, have, "the spec's tables and the installed ones differ");
    assert!(have.len() >= 26, "{}", have.len());
    // each is a supported capability, so the support table and the frozen record name it
    h.execute(
        r#"
        for _, name in ipairs({'ImGuiWindowFlags.NoTitleBar', 'ImGuiWindowFlags.NoInputs', 'ImGuiCond.Always', 'ImGuiCond.FirstUseEver'}) do
            assert(eqoa.capabilities.get(name).status == 'supported', name)
        end
        "#,
    )
    .unwrap();
}
