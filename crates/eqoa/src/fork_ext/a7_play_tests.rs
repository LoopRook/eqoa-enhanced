// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A7 from Play-shaped state: friends, ignore, guild, who, group search, optional-arg `group_invite`,
//! held `friend_request`/`guild_invite` sessions with 45-second auto-refusal, A1 client-drawn confirmations
//! for `guild_kick`/`friend_accept`/`guild_accept`, and exact wire-byte parity against the game's own builders.
//!
//! Each test states its negative control.
use super::*;
use crate::fork_ext::addons::{
    action_broker, confirm,
    discovery::{DiscoveredAddon, EntryMode, Extensions},
    host::Host,
    native_a7,
    native_core::{self, CORE, Core},
    native_events, window_sessions as ws, window_takeover,
};
use crate::groupsearch::{Lfg, Query};
use crate::guild::Cmds;
use crate::social::Ignore;
use net::session::{Datagrams, Event, Session};
use net::stream::Writer;
use std::time::Duration;
use winit::keyboard::{Key, NamedKey};

struct NullNet;
impl Datagrams for NullNet {
    fn send_to(&mut self, _: &[u8], _: [u8; 4], _: u16) {}
    fn recv_from(&mut self) -> Option<(Vec<u8>, [u8; 4], u16)> {
        None
    }
}

fn live_session() -> Session {
    let mut s = Session::connect_with(
        Box::new(NullNet),
        ([127, 0, 0, 1], 40000),
        ([127, 0, 0, 1], 10070),
        "test",
        [0; 32],
    )
    .unwrap();
    // connect_with opens a login conn; keep it as the active game conn in world and record every sent payload
    s.state = net::session::State::World;
    s.self_id = Some(100);
    s.record = Some(Vec::new());
    s
}

fn recorded(p: &Play) -> Vec<Vec<u8>> {
    p.session
        .record
        .as_ref()
        .map(|r| r.iter().map(|(_, data)| data.clone()).collect())
        .unwrap_or_default()
}

fn clear_recorded(p: &mut Play) {
    if let Some(r) = p.session.record.as_mut() {
        r.clear();
    }
}

/// Puts every thread-wide piece of state a test here touched back to its baseline when the test ends (also on a
/// panic). With `--test-threads=1` all tests share one thread, so what this round's tests leave behind (a held
/// request, an open question, a mod's takeover, the live-features word, the demand flag, the process-wide key table) reaches the next
/// test, which may belong to another round and does not reset it.
struct Baseline;

impl Drop for Baseline {
    fn drop(&mut self) {
        confirm::cancel_all("test");
        native_a7::forget();
        action_broker::reset_all_for_test();
        ws::reset_for_test();
        window_takeover::reset_all_for_test();
        window_takeover::forget_switch_for_test();
        native_core::fixture_demand(false);
        // the key table is one process-wide list: a mod's "9" key left in it blocks the next round's test mod
        crate::fork_ext::bindpage::reset_in_memory();
        CORE.with(|c| *c.borrow_mut() = Core::default());
        crate::live::set_features(crate::live::Features(None));
    }
}

fn ready() -> Play {
    CORE.with(|c| *c.borrow_mut() = Core::default());
    native_core::fixture_demand(true);
    action_broker::reset_all_for_test();
    ws::reset_for_test();
    window_takeover::reset_all_for_test();
    window_takeover::forget_switch_for_test();
    confirm::cancel_all("test");
    native_a7::forget();
    crate::fork_ext::bindpage::reset_in_memory();
    // Reset global live-features flags to retail (non-live) baseline before each test.
    crate::live::set_features(crate::live::Features(None));
    let mut p = super::tests::play();
    let _ = p.loading.wait.take();
    p.hud = Some((
        crate::ui::Vui::new(Default::default(), Default::default(), vec![]),
        Default::default(),
        crate::hud::Hud::new(),
    ));
    p.session = live_session();
    p.dump.player.id = 100;
    p.dump.player.level = 30;
    p.player.name = "Hero".into();
    CORE.with(|c| c.borrow_mut().view.in_world = true);
    p
}

fn host_all() -> Host {
    let root = std::env::temp_dir();
    let addon = DiscoveredAddon {
        id: "a7_test_mod".into(),
        name: "a7_test_mod".into(),
        entry: root.join("main.lua"),
        script_root: root.clone(),
        package_root: root.clone(),
        module_dirs: vec![],
        resource_dirs: vec![],
        api: Some("eqoa/1".into()),
        entry_mode: EntryMode::Once,
        capabilities_declared: true,
        requested_capabilities: [
            "eqoa.actions.*",
            "eqoa.who.request",
            "eqoa.lfg.search",
            "eqoa.lfg.register",
            "eqoa.lfg.unregister",
            "eqoa.windows.session",
            "eqoa.windows.focus",
            "eqoa.windows.takeover",
            "eqoa.events.on",
        ]
        .iter()
        .map(|s| (*s).to_string())
        .collect(),
        extensions: Extensions {
            replaces: vec!["friend_request".into(), "guild_invite".into()],
            ..Default::default()
        },
        initially_enabled: true,
        mods_root: None,
    };
    let host = Host::for_addon(&addon).unwrap();
    host.execute(
        r#"
        results, closed, ev = {}, {}, {}
        friends_ev, ignored_ev, guild_ev, who_ev, lfg_ev, lfg_ch = {}, {}, {}, {}, {}, {}
        eqoa.events.on("action.result", function(e) results[#results + 1] = e.data end)
        eqoa.events.on("confirm.closed", function(e) closed[#closed + 1] = e.data end)
        eqoa.events.on("windows.session", function(e) ev[#ev + 1] = e.data.phase .. ":" .. e.data.kind .. ":" .. tostring(e.data.reason) end)
        eqoa.events.on("friends.changed", function(e) friends_ev[#friends_ev + 1] = e.data end)
        eqoa.events.on("ignored.changed", function(e) ignored_ev[#ignored_ev + 1] = e.data end)
        eqoa.events.on("guild.changed", function(e) guild_ev[#guild_ev + 1] = e.data end)
        eqoa.events.on("who.result", function(e) who_ev[#who_ev + 1] = e.data end)
        eqoa.events.on("lfg.result", function(e) lfg_ev[#lfg_ev + 1] = e.data end)
        eqoa.events.on("lfg.changed", function(e) lfg_ch[#lfg_ch + 1] = e.data end)
        eqoa.input.bind({ id = "go", label = "Go", default_key = "9" }, function() last_req, last_err = next_fn() end)
        "#,
    )
    .unwrap();
    host
}

fn press_9(host: &Host) {
    CORE.with(|c| {
        c.borrow_mut().emit(
            "input.key",
            serde_json::json!({"key":"9","down":true,"repeat":false,"synthetic":false,"text":""}),
        )
    });
    native_events::dispatch(host).unwrap();
}

fn ask(p: &mut Play, host: &Host, code: &str) {
    host.execute(&format!(
        "results = {{}}; closed = {{}}; last_err = nil; last_req = nil; next_fn = function() {code} end"
    ))
    .unwrap();
    press_9(host);
    super::begin_iteration(p, true);
    native_events::dispatch(host).unwrap();
}

fn step(p: &mut Play, host: &Host) {
    super::begin_iteration(p, true);
    native_events::dispatch(host).unwrap();
}

fn key(k: NamedKey) {
    assert!(confirm::key(&Key::Named(k)));
}

/// Delivers a server opcode through the exact same `begin_game_event` / `Play` handler / `end_game_event`
/// sequence that `Play::frame` runs.
fn deliver_op(p: &mut Play, op: u16, payload: Vec<u8>) {
    let ev = Event::Opcode { op, data: payload.clone() };
    let note = super::begin_game_event(p, &ev);
    match op {
        0x03D5 | 0x03D6 => {
            p.buddies.recv(op, &payload);
        }
        0x076F => p.buddy_request(&payload),
        0x1003..=0x1006 => {
            p.ignore.recv(op, &payload);
        }
        0x0468 => {
            let _ = p.guild.recv_0468(&payload);
        }
        0x057A => {
            let idle = p.mode == 0;
            let (inv, _) = p.guild_cmds.recv_057a(&payload, &p.guild, idle);
            match inv {
                Some(crate::guild::Invited::Busy(m)) => p.session.send(&m),
                Some(crate::guild::Invited::InGuild(m)) => p.session.send(&m),
                Some(crate::guild::Invited::Show) => p.open_guild(20),
                None => {}
            }
        }
        0x0E02 => p.who.recv(&payload),
        0x0840 => {
            p.live.part.recv(&payload);
        }
        0x07F5 => crate::live::set_features(crate::live::Features(crate::live::Features::parse(&payload))),
        other => panic!("unexpected test opcode {other:#06x}"),
    }
    super::end_game_event(p, note);
}

fn pkt_03d5(id: i32, name: &str, class: i32, race: i32, level: i32, online: bool, world: &str) -> Vec<u8> {
    let mut w = Writer::default();
    w.signed(id as i64)
        .string(name.as_bytes())
        .signed(class as i64)
        .signed(race as i64)
        .signed(level as i64)
        .u8(online as u8)
        .wide(world);
    w.data
}

fn pkt_03d6(id: i32) -> Vec<u8> {
    let mut w = Writer::default();
    w.signed(id as i64);
    w.data
}

fn pkt_076f(id: u32, name: &str) -> Vec<u8> {
    let mut w = Writer::default();
    w.u32(id).string(name.as_bytes());
    w.data
}

fn pkt_1003(name: &str, id: u32) -> Vec<u8> {
    let mut w = Writer::default();
    w.string(name.as_bytes()).u32(id);
    w.data
}

fn pkt_1004(id: u32) -> Vec<u8> {
    let mut w = Writer::default();
    w.u32(id);
    w.data
}

fn pkt_0468_info(name: &str, motd: &str) -> Vec<u8> {
    let mut w = Writer::default();
    w.u8(1).wide(name).wide(motd);
    w.data
}

fn pkt_0468_member(id: i32, name: &str, class: i32, race: i32, level: i32, rank: i32, online: bool) -> Vec<u8> {
    let mut w = Writer::default();
    w.u8(3)
        .signed(id as i64)
        .string(name.as_bytes())
        .signed(class as i64)
        .signed(race as i64)
        .signed(level as i64)
        .signed(rank as i64)
        .u8(online as u8);
    w.data
}

fn pkt_057a(inviter_id: u32, inviter: &str, guild: &str) -> Vec<u8> {
    let mut w = Writer::default();
    w.u8(1).signed(0).u32(inviter_id).string(inviter.as_bytes()).wide(guild);
    w.data
}

fn pkt_0e02(serial: i32, rows: &[(u8, u8, u8, &str, u32)]) -> Vec<u8> {
    let mut w = Writer::default();
    w.u8(1).signed(serial as i64).signed(rows.len() as i64);
    for &(race, class, level, name, id) in rows {
        w.u8(race).u8(class).u8(level).string(name.as_bytes()).u32(id);
    }
    w.data
}

fn pkt_0840(rows: &[(&str, u8, i32, i32, u32)]) -> Vec<u8> {
    let mut w = Writer::default();
    w.u8(0x40).signed(rows.len() as i64);
    for &(name, level, class, race, id) in rows {
        w.string(name.as_bytes())
            .u8(level)
            .signed(class as i64)
            .signed(race as i64)
            .u32(1.0f32.to_bits())
            .u32(2.0f32.to_bits())
            .u32(3.0f32.to_bits())
            .u8(1)
            .u32(id);
    }
    w.data
}

#[test]
fn a7_friends_read_events_and_wire_byte_parity_with_negative_controls() {
    let _baseline = Baseline;
    let mut p = ready();
    let host = host_all();
    step(&mut p, &host);

    // Server sends 0x03D5: friend list populates and fires friends.changed
    deliver_op(&mut p, 0x03D5, pkt_03d5(201, "Alyra", 1, 2, 42, true, "Castle Lightwolf"));
    step(&mut p, &host);
    host.execute(
        r#"
        local list = eqoa.social.friends()
        assert(#list == 1 and list[1].id == 201 and list[1].name == "Alyra" and list[1].online == true and list[1].level == 42)
        assert(#friends_ev >= 1 and #friends_ev[#friends_ev].after == 1)
        "#,
    )
    .unwrap();

    // Wire-byte parity: friend_add sends exact Buddies::add bytes (0x0366)
    let expected_add = p.buddies.add(&p.player.name, "Bram").unwrap();
    clear_recorded(&mut p);
    ask(&mut p, &host, "return eqoa.actions.friend_add('Bram')");
    host.execute("assert(results[1] and results[1].ok == true)").unwrap();
    assert_eq!(recorded(&p), vec![expected_add], "friend_add must match Buddies::add bytes");

    // Negative controls: self-add and duplicate add send nothing
    clear_recorded(&mut p);
    ask(&mut p, &host, "return eqoa.actions.friend_add('Hero')");
    host.execute("assert(results[1] and results[1].ok == false and results[1].code == 'invalid_argument')").unwrap();
    assert!(recorded(&p).is_empty(), "negative control: self friend_add sends nothing");

    ask(&mut p, &host, "return eqoa.actions.friend_add('alyra')");
    host.execute("assert(results[1] and results[1].ok == false and results[1].code == 'not_ready')").unwrap();
    assert!(recorded(&p).is_empty(), "negative control: duplicate friend_add sends nothing");

    // Wire-byte parity: friend_remove sends exact Buddies::remove(idx) bytes (0x0369)
    let expected_remove = p.buddies.remove(0).unwrap();
    clear_recorded(&mut p);
    ask(&mut p, &host, "return eqoa.actions.friend_remove(201)");
    host.execute("assert(results[1] and results[1].ok == true)").unwrap();
    assert_eq!(recorded(&p), vec![expected_remove], "friend_remove must match Buddies::remove bytes");

    // Negative control: removing unknown friend returns not_found and sends nothing
    clear_recorded(&mut p);
    ask(&mut p, &host, "return eqoa.actions.friend_remove('Nobody')");
    host.execute("assert(results[1] and results[1].ok == false and results[1].code == 'not_found')").unwrap();
    assert!(recorded(&p).is_empty());

    // Server removes via 0x03D6: list clears
    deliver_op(&mut p, 0x03D6, pkt_03d6(201));
    step(&mut p, &host);
    host.execute("assert(#eqoa.social.friends() == 0)").unwrap();
}

#[test]
fn a7_ignore_read_events_and_wire_byte_parity_with_negative_controls() {
    let _baseline = Baseline;
    let mut p = ready();
    let host = host_all();
    step(&mut p, &host);

    // Server adds ignore via 0x1003
    deliver_op(&mut p, 0x1003, pkt_1003("Spammer", 55));
    step(&mut p, &host);
    host.execute(
        r#"
        local ig = eqoa.social.ignored()
        assert(#ig == 1 and ig[1].id == 55 and ig[1].name == "Spammer")
        assert(#ignored_ev >= 1)
        "#,
    )
    .unwrap();

    // Wire-byte parity: ignore_add sends exact Ignore::add bytes (0x1000)
    clear_recorded(&mut p);
    ask(&mut p, &host, "return eqoa.actions.ignore_add('Troll')");
    host.execute("assert(results[1] and results[1].ok == true)").unwrap();
    assert_eq!(recorded(&p), vec![Ignore::add("Troll")], "ignore_add must match Ignore::add bytes");

    // Negative controls: self and duplicate ignore send nothing
    clear_recorded(&mut p);
    ask(&mut p, &host, "return eqoa.actions.ignore_add('Hero')");
    host.execute("assert(results[1] and results[1].ok == false and results[1].code == 'invalid_argument')").unwrap();
    ask(&mut p, &host, "return eqoa.actions.ignore_add('spammer')");
    host.execute("assert(results[1] and results[1].ok == false and results[1].code == 'not_ready')").unwrap();
    assert!(recorded(&p).is_empty());

    // Wire-byte parity: ignore_remove sends exact Ignore::remove_id bytes (0x1002)
    clear_recorded(&mut p);
    ask(&mut p, &host, "return eqoa.actions.ignore_remove(55)");
    host.execute("assert(results[1] and results[1].ok == true)").unwrap();
    assert_eq!(recorded(&p), vec![Ignore::remove_id(55)], "ignore_remove must match Ignore::remove_id bytes");

    // Negative control: unknown player sends nothing
    clear_recorded(&mut p);
    ask(&mut p, &host, "return eqoa.actions.ignore_remove('NoSuchPlayer')");
    host.execute("assert(results[1] and results[1].ok == false and results[1].code == 'not_found')").unwrap();
    assert!(recorded(&p).is_empty());

    // Server removes via 0x1004
    deliver_op(&mut p, 0x1004, pkt_1004(55));
    step(&mut p, &host);
    host.execute("assert(#eqoa.social.ignored() == 0)").unwrap();
}

#[test]
fn a7_guild_roster_motd_rank_actions_wire_parity_and_no_disband_or_quit() {
    let _baseline = Baseline;
    let mut p = ready();
    let host = host_all();
    step(&mut p, &host);

    // Owner requirement: disband and quit keep the game's own prompt and are NOT offered on eqoa.actions.
    host.execute(
        r#"
        for _, bad in ipairs({"guild_disband", "guild_quit", "guild_leave", "disband_guild", "quit_guild"}) do
            assert(eqoa.actions[bad] == nil, bad .. " must not exist")
        end
        "#,
    )
    .unwrap();

    // Populate guild via 0x0468: Hero (id 100) is Leader (rank 3), Cora (id 101) is Senior (rank 1), Dan (id 102) is Member (rank 0)
    deliver_op(&mut p, 0x0468, pkt_0468_info("Iron Dawn", "Raid at 8"));
    deliver_op(&mut p, 0x0468, pkt_0468_member(100, "Hero", 1, 2, 30, 3, true));
    deliver_op(&mut p, 0x0468, pkt_0468_member(101, "Cora", 2, 1, 28, 1, true));
    deliver_op(&mut p, 0x0468, pkt_0468_member(102, "Dan", 3, 0, 25, 0, false));
    step(&mut p, &host);

    host.execute(
        r#"
        local g = eqoa.guild.get()
        assert(g.in_guild == true and g.name == "Iron Dawn" and g.motd == "Raid at 8")
        assert(g.self_rank == 3 and g.can_invite == true and g.can_set_motd == true)
        assert(#g.members == 3)
        assert(#guild_ev >= 1)
        "#,
    )
    .unwrap();

    // Wire-byte parity: guild_invite, guild_promote, guild_demote, guild_set_motd
    clear_recorded(&mut p);
    let expected_inv = Cmds::invite(&p.player.name, "Elena", &p.guild).unwrap();
    ask(&mut p, &host, "return eqoa.actions.guild_invite('Elena')");
    host.execute("assert(results[1] and results[1].ok == true)").unwrap();
    assert_eq!(recorded(&p), vec![expected_inv], "guild_invite wire parity");

    clear_recorded(&mut p);
    ask(&mut p, &host, "return eqoa.actions.guild_promote(101)");
    host.execute("assert(results[1] and results[1].ok == true)").unwrap();
    assert_eq!(recorded(&p), vec![Cmds::member_op(4, 101)], "guild_promote wire parity");

    clear_recorded(&mut p);
    ask(&mut p, &host, "return eqoa.actions.guild_demote('Cora')");
    host.execute("assert(results[1] and results[1].ok == true)").unwrap();
    assert_eq!(recorded(&p), vec![Cmds::member_op(5, 101)], "guild_demote wire parity");

    clear_recorded(&mut p);
    ask(&mut p, &host, "return eqoa.actions.guild_set_motd('Meet at Tunaria')");
    host.execute("assert(results[1] and results[1].ok == true)").unwrap();
    assert_eq!(recorded(&p), vec![Cmds::text(11, "Meet at Tunaria")], "guild_set_motd wire parity");

    // Negative control: demote lowest-rank member (Dan, rank 0) refused, nothing sent
    clear_recorded(&mut p);
    ask(&mut p, &host, "return eqoa.actions.guild_demote(102)");
    host.execute("assert(results[1] and results[1].ok == false and results[1].code == 'not_ready')").unwrap();
    assert!(recorded(&p).is_empty());

    // Negative control: plain member (rank 0) cannot invite, set MOTD, promote, or kick
    deliver_op(&mut p, 0x0468, pkt_0468_member(100, "Hero", 1, 2, 30, 0, true));
    step(&mut p, &host);
    clear_recorded(&mut p);
    ask(&mut p, &host, "return eqoa.actions.guild_invite('Elena')");
    host.execute("assert(results[1] and results[1].ok == false and results[1].code == 'permission_denied')").unwrap();
    ask(&mut p, &host, "return eqoa.actions.guild_set_motd('Nope')");
    host.execute("assert(results[1] and results[1].ok == false and results[1].code == 'permission_denied')").unwrap();
    ask(&mut p, &host, "return eqoa.actions.guild_kick(102)");
    host.execute("assert(results[1] and results[1].ok == false and results[1].code == 'permission_denied')").unwrap();
    assert!(recorded(&p).is_empty());
    assert!(!confirm::is_open());
}

#[test]
fn a7_guild_kick_asks_a1_confirm_and_only_players_yes_sends_exact_bytes() {
    let _baseline = Baseline;
    let mut p = ready();
    let host = host_all();
    deliver_op(&mut p, 0x0468, pkt_0468_info("Iron Dawn", ""));
    deliver_op(&mut p, 0x0468, pkt_0468_member(100, "Hero", 1, 2, 30, 3, true));
    deliver_op(&mut p, 0x0468, pkt_0468_member(102, "Dan", 3, 0, 25, 0, true));
    step(&mut p, &host);
    clear_recorded(&mut p);

    // Asking opens A1's client-drawn confirm; nothing sent yet (negative control)
    ask(&mut p, &host, "return eqoa.actions.guild_kick(102)");
    host.execute("assert(results[1] and results[1].ok == true and results[1].detail.asked == true)").unwrap();
    assert!(recorded(&p).is_empty(), "guild_kick must send nothing before player Yes");
    let q = confirm::current_for_test().expect("confirm open");
    assert!(!q.yes && q.text.contains("Dan"), "starts on No and names member: {}", q.text);

    // Enter on default No sends nothing
    key(NamedKey::Enter);
    step(&mut p, &host);
    assert!(recorded(&p).is_empty(), "No must send nothing");
    host.execute("assert(closed[1].kind == 'guild_kick' and closed[1].answer == 'no')").unwrap();

    // Re-ask, press Right then Enter -> Yes sends exact Cmds::member_op(6, 102)
    ask(&mut p, &host, "return eqoa.actions.guild_kick(102)");
    key(NamedKey::ArrowRight);
    key(NamedKey::Enter);
    assert!(recorded(&p).is_empty(), "not sent until next Play step");
    step(&mut p, &host);
    assert_eq!(recorded(&p), vec![Cmds::member_op(6, 102)], "Yes sends exact kick bytes");
    host.execute("assert(closed[1].answer == 'yes' and closed[1].detail.sent == true)").unwrap();

    // Negative control: member leaves roster before Yes -> nothing sent on Yes
    clear_recorded(&mut p);
    ask(&mut p, &host, "return eqoa.actions.guild_kick(102)");
    p.guild.members.remove(&102);
    key(NamedKey::ArrowRight);
    key(NamedKey::Enter);
    step(&mut p, &host);
    assert!(recorded(&p).is_empty(), "changed roster before Yes sends nothing");
    host.execute("assert(closed[1].answer == 'yes' and closed[1].detail.sent == false)").unwrap();
}

#[test]
fn a7_who_request_and_reply_with_stale_serial_negative_control() {
    let _baseline = Baseline;
    let mut p = ready();
    let host = host_all();
    step(&mut p, &host);

    // Compute exact expected bytes for the next Who::open() call (serial goes 0 -> 1)
    let mut expected_who = crate::wholist::Who::default();
    let expected_pkt = expected_who.open();

    clear_recorded(&mut p);
    ask(&mut p, &host, "return eqoa.who.request()");
    host.execute("assert(results[1] and results[1].ok == true)").unwrap();
    assert_eq!(recorded(&p), vec![expected_pkt], "who.request must match Who::open() bytes");
    host.execute("assert(eqoa.who.get().pending == true)").unwrap();

    // Negative control: stale serial (0 instead of 1) is dropped and does not fire who.result
    deliver_op(&mut p, 0x0E02, pkt_0e02(0, &[(1, 2, 15, "Stale", 999)]));
    step(&mut p, &host);
    host.execute("assert(#who_ev == 0 and #eqoa.who.get().entries == 0)").unwrap();

    // Matching serial (1) populates who.get() and fires who.result
    deliver_op(&mut p, 0x0E02, pkt_0e02(1, &[(2, 3, 45, "Talia", 777)]));
    step(&mut p, &host);
    host.execute(
        r#"
        local w = eqoa.who.get()
        assert(w.pending == false and w.received == 1 and #w.entries == 1)
        assert(w.entries[1].name == "Talia" and w.entries[1].level == 45 and w.entries[1].id == 777)
        assert(#who_ev == 1 and who_ev[1].count == 1)
        "#,
    )
    .unwrap();
}

#[test]
fn a7_lfg_gated_on_non_live_and_wire_parity_on_live_server() {
    let _baseline = Baseline;
    let mut p = ready();
    let host = host_all();
    step(&mut p, &host);

    // Negative control: non-live server -> available=false, all lfg calls refuse with unsupported (the server does not offer the feature: section 1's table) and send nothing
    clear_recorded(&mut p);
    host.execute("assert(eqoa.lfg.get().available == false)").unwrap();
    ask(&mut p, &host, "return eqoa.lfg.search()");
    host.execute("assert(results[1] and results[1].ok == false and results[1].code == 'unsupported')").unwrap();
    ask(&mut p, &host, "return eqoa.lfg.register()");
    host.execute("assert(results[1] and results[1].ok == false and results[1].code == 'unsupported')").unwrap();
    ask(&mut p, &host, "return eqoa.lfg.unregister()");
    host.execute("assert(results[1] and results[1].ok == false and results[1].code == 'unsupported')").unwrap();
    assert!(recorded(&p).is_empty(), "non-live server must send nothing for LFG");

    // Server enables live features via 0x07F5 (word 0 bit 0 = 1)
    deliver_op(&mut p, 0x07F5, vec![1, 0, 0, 0]);
    step(&mut p, &host);
    host.execute("assert(eqoa.lfg.get().available == true)").unwrap();

    // Register / Unregister wire-byte parity
    clear_recorded(&mut p);
    ask(&mut p, &host, "return eqoa.lfg.register()");
    host.execute("assert(results[1] and results[1].ok == true)").unwrap();
    assert_eq!(recorded(&p), vec![Lfg::register()], "lfg.register wire parity");

    clear_recorded(&mut p);
    ask(&mut p, &host, "return eqoa.lfg.unregister()");
    host.execute("assert(results[1] and results[1].ok == true)").unwrap();
    assert_eq!(recorded(&p), vec![Lfg::unregister()], "lfg.unregister wire parity");

    // Search wire-byte parity
    let mut ref_lfg = Lfg::default();
    let expected_search = ref_lfg
        .search(&Query { kind: 1, races: -1, classes: -1, min: 25, max: 35 }, 0)
        .unwrap();
    clear_recorded(&mut p);
    ask(&mut p, &host, "return eqoa.lfg.search({min_level = 25, max_level = 35})");
    host.execute("assert(results[1] and results[1].ok == true)").unwrap();
    assert_eq!(recorded(&p), vec![expected_search], "lfg.search wire parity");
    host.execute("assert(eqoa.lfg.get().busy == true)").unwrap();

    // Negative control: second search while busy refused
    clear_recorded(&mut p);
    ask(&mut p, &host, "return eqoa.lfg.search()");
    host.execute("assert(results[1] and results[1].ok == false and results[1].code == 'busy')").unwrap();
    assert!(recorded(&p).is_empty());

    // 0x0840 sub 0x40 reply populates entries and fires lfg.result
    deliver_op(&mut p, 0x0840, pkt_0840(&[("Kael", 31, 4, 1, 888)]));
    step(&mut p, &host);
    host.execute(
        r#"
        local l = eqoa.lfg.get()
        assert(l.busy == false and #l.entries == 1 and l.entries[1].name == "Kael" and l.entries[1].id == 888)
        assert(#lfg_ev == 1 and lfg_ev[1].count == 1)
        "#,
    )
    .unwrap();
}

#[test]
fn a7_group_invite_optional_args_match_native_invite_to_group_bytes() {
    let _baseline = Baseline;
    let mut p = ready();
    let host = host_all();
    step(&mut p, &host);

    // Native bytes from Play::invite_to_group(777, "Talia")
    clear_recorded(&mut p);
    p.invite_to_group(777, "Talia");
    let native_with_id = recorded(&p);
    assert_eq!(native_with_id.len(), 1);

    // Mod call group_invite(777, "Talia") sends identical bytes
    clear_recorded(&mut p);
    ask(&mut p, &host, "return eqoa.actions.group_invite(777, 'Talia')");
    host.execute("assert(results[1] and results[1].ok == true)").unwrap();
    assert_eq!(recorded(&p), native_with_id, "group_invite(id, name) wire parity");

    // Mod call group_invite("Talia") resolves id from who/friends/guild/lfg/entities and matches invite_to_group(777, "Talia")
    p.who.list.push(crate::wholist::WhoRow { race: 1, class: 2, level: 30, name: "Talia".into(), x1c: 777 });
    clear_recorded(&mut p);
    ask(&mut p, &host, "return eqoa.actions.group_invite('Talia')");
    host.execute("assert(results[1] and results[1].ok == true)").unwrap();
    assert_eq!(recorded(&p), native_with_id, "group_invite(name) resolves id and matches wire bytes");

    // Negative control: group_invite with unknown name refuses with not_found and sends nothing
    clear_recorded(&mut p);
    ask(&mut p, &host, "return eqoa.actions.group_invite('UnknownPlayer')");
    host.execute("assert(results[1] and results[1].ok == false and results[1].code == 'not_found')").unwrap();
    assert!(recorded(&p).is_empty());

    // Negative control: 0-arg group_invite() with no target still refuses with not_ready
    clear_recorded(&mut p);
    ask(&mut p, &host, "return eqoa.actions.group_invite()");
    host.execute("assert(results[1] and results[1].ok == false and results[1].code == 'not_ready')").unwrap();
    assert!(recorded(&p).is_empty());
}

#[test]
fn a7_friend_request_mod_off_control_takeover_45s_timeout_and_confirmed_accept() {
    let _baseline = Baseline;
    // 1. Control: mod off (no takeover) -> native mode 17 box opens unchanged
    let mut p = ready();
    let host = host_all();
    deliver_op(&mut p, 0x076F, pkt_076f(401, "Vex"));
    super::health_redraw(&mut p, false, false);
    assert_eq!(p.mode, 17, "control: with mod off, native friend request opens in mode 17");
    assert!(p.buddy_wnd.mbox_visible, "control: native buddy request box is visible");

    // 2. scope="own" is rejected for server-initiated friend_request
    host.execute(
        "local ok, err = eqoa.windows.takeover('friend_request', {scope='own'}); assert(ok == nil and err.code == 'invalid_argument')",
    )
    .unwrap();

    // 3. Mod takes over friend_request with scope="all": native box never draws, session opens
    let mut p = ready();
    let host = host_all();
    host.execute("assert(eqoa.windows.takeover('friend_request', {scope='all'}) == true)").unwrap();
    host.execute("assert(eqoa.windows.taken().friend_request == true)").unwrap();
    step(&mut p, &host);

    deliver_op(&mut p, 0x076F, pkt_076f(401, "Vex"));
    // health_redraw runs before draw_hud; it must hide the native box immediately so 0 frames draw
    super::health_redraw(&mut p, false, false);
    assert_eq!(p.mode, 0, "taken friend_request hides native mode 17 before draw_hud");
    assert!(!p.buddy_wnd.mbox_visible, "native box stays hidden");
    step(&mut p, &host);
    host.execute(
        r#"
        local r = eqoa.social.friend_request()
        assert(r ~= nil and r.id == 401 and r.name == "Vex" and r.seconds_remaining > 40)
        assert(eqoa.windows.session("friend_request") ~= nil)
        "#,
    )
    .unwrap();

    // Second simultaneous friend request is refused as busy (0x0368 reason 1) without clobbering first
    clear_recorded(&mut p);
    let mut ref_buddies = crate::social::Buddies::default();
    ref_buddies.request = 402;
    let expected_busy = ref_buddies.refuse(1);
    deliver_op(&mut p, 0x076F, pkt_076f(402, "Second"));
    super::health_redraw(&mut p, false, false);
    assert_eq!(recorded(&p), vec![expected_busy], "second request refused as busy");
    assert_eq!(p.mode, 0);
    host.execute("assert(eqoa.social.friend_request().id == 401)").unwrap();

    // 4. 45-second timeout test: aged 44 s still open; aged 45 s auto-refused with exact native bytes (reason 2)
    clear_recorded(&mut p);
    native_a7::age_friend_request_for_test(Duration::from_secs(44));
    step(&mut p, &host);
    assert!(recorded(&p).is_empty(), "at 44 s, request must NOT time out yet");
    host.execute("assert(eqoa.social.friend_request() ~= nil and eqoa.windows.session('friend_request') ~= nil)").unwrap();

    let mut ref_buddies = crate::social::Buddies::default();
    ref_buddies.request = 401;
    let expected_timeout = ref_buddies.refuse(2);
    native_a7::age_friend_request_for_test(Duration::from_secs(2));
    step(&mut p, &host);
    assert_eq!(recorded(&p), vec![expected_timeout], "at 45 s, exact native timeout refusal bytes sent");
    host.execute(
        "assert(eqoa.social.friend_request() == nil and eqoa.windows.session('friend_request') == nil and ev[#ev] == 'closed:friend_request:timeout')",
    )
    .unwrap();

    // 5. Accepting requires player's own Yes on A1 confirm; sends exact Buddies::accept() bytes (0x0367)
    clear_recorded(&mut p);
    deliver_op(&mut p, 0x076F, pkt_076f(405, "Nira"));
    super::health_redraw(&mut p, false, false);
    step(&mut p, &host);
    ask(&mut p, &host, "return eqoa.actions.friend_accept()");
    host.execute("assert(results[1] and results[1].ok == true and results[1].detail.asked == true)").unwrap();
    assert!(recorded(&p).is_empty(), "friend_accept sends nothing until player Yes");
    let q = confirm::current_for_test().expect("confirm open");
    assert!(!q.yes && q.text.contains("Nira"));

    // Pressing Enter on default No sends nothing and keeps the request open
    key(NamedKey::Enter);
    step(&mut p, &host);
    assert!(recorded(&p).is_empty());
    host.execute("assert(eqoa.social.friend_request() ~= nil)").unwrap();

    // Ask again, Right+Enter -> Yes sends exact accept bytes and clears request so it can't be accepted twice
    let mut ref_buddies = crate::social::Buddies::default();
    ref_buddies.request = 405;
    let expected_accept = ref_buddies.accept();
    ask(&mut p, &host, "return eqoa.actions.friend_accept()");
    key(NamedKey::ArrowRight);
    key(NamedKey::Enter);
    step(&mut p, &host);
    assert_eq!(recorded(&p), vec![expected_accept], "Yes sends exact Buddies::accept bytes");
    assert_eq!(p.buddies.request, u32::MAX, "request id cleared after accept");
    host.execute("assert(eqoa.social.friend_request() == nil and eqoa.windows.session('friend_request') == nil)").unwrap();

    // 6. Explicit refuse sends exact Buddies::refuse(2) bytes immediately
    clear_recorded(&mut p);
    deliver_op(&mut p, 0x076F, pkt_076f(406, "Orin"));
    super::health_redraw(&mut p, false, false);
    step(&mut p, &host);
    let mut ref_buddies = crate::social::Buddies::default();
    ref_buddies.request = 406;
    let expected_refuse = ref_buddies.refuse(2);
    ask(&mut p, &host, "return eqoa.actions.friend_refuse()");
    assert_eq!(recorded(&p), vec![expected_refuse]);
    host.execute("assert(eqoa.social.friend_request() == nil)").unwrap();

    // 7. pass() hands back to the native mode 17 box
    clear_recorded(&mut p);
    deliver_op(&mut p, 0x076F, pkt_076f(407, "Pia"));
    super::health_redraw(&mut p, false, false);
    step(&mut p, &host);
    host.execute("local s = assert(eqoa.windows.session('friend_request')); assert(eqoa.windows.pass(s.ref) == true)").unwrap();
    step(&mut p, &host);
    assert_eq!(p.mode, 17, "pass opens native mode 17 friend request");
    assert!(p.buddy_wnd.mbox_visible);
    assert!(recorded(&p).is_empty(), "passing back does not refuse the request");
}

#[test]
fn a7_guild_invite_mod_off_control_takeover_45s_timeout_confirm_and_drop_cleanup() {
    let _baseline = Baseline;
    // 1. Control: mod off -> native mode 20 opens unchanged
    let mut p = ready();
    let _host = host_all();
    deliver_op(&mut p, 0x057A, pkt_057a(501, "Garr", "Silver Hand"));
    super::health_redraw(&mut p, false, false);
    assert_eq!(p.mode, 20, "control: with mod off, native guild invite opens in mode 20");
    assert_eq!(p.guild_open, Some(20), "control: native guild invite box queued to open");

    // 2. Mod takes over guild_invite with scope="all"
    let mut p = ready();
    let host = host_all();
    host.execute("assert(eqoa.windows.takeover('guild_invite', {scope='all'}) == true)").unwrap();
    step(&mut p, &host);

    deliver_op(&mut p, 0x057A, pkt_057a(501, "Garr", "Silver Hand"));
    super::health_redraw(&mut p, false, false);
    assert_eq!(p.mode, 0, "taken guild_invite hides native mode 20 before draw_hud");
    assert_eq!(p.guild_open, None, "native guild invite open cleared");
    assert!(!p.guild_wnd.mbox_visible, "native guild invite box not visible");
    step(&mut p, &host);
    host.execute(
        r#"
        local inv = eqoa.guild.invite()
        assert(inv ~= nil and inv.inviter_id == 501 and inv.inviter == "Garr" and inv.guild == "Silver Hand")
        assert(eqoa.windows.session("guild_invite") ~= nil)
        "#,
    )
    .unwrap();

    // Second simultaneous guild invite is refused as busy (sub 10 reason 2) without clobbering first
    clear_recorded(&mut p);
    let mut ref_cmds = Cmds { inviter: 502, ..Default::default() };
    let expected_busy = ref_cmds.refuse(2);
    deliver_op(&mut p, 0x057A, pkt_057a(502, "Other", "Other Guild"));
    super::health_redraw(&mut p, false, false);
    assert_eq!(recorded(&p), vec![expected_busy]);
    host.execute("assert(eqoa.guild.invite().inviter_id == 501)").unwrap();

    // 3. 45-second timeout cancels an open accept confirm AND sends exact native refusal bytes (sub 10 reason 1)
    clear_recorded(&mut p);
    ask(&mut p, &host, "return eqoa.actions.guild_accept()");
    assert!(confirm::is_open(), "confirm is open before timeout");
    native_a7::age_guild_invite_for_test(Duration::from_secs(44));
    step(&mut p, &host);
    assert!(recorded(&p).is_empty(), "at 44 s, not expired yet");
    assert!(confirm::is_open());

    let mut ref_cmds = Cmds { inviter: 501, ..Default::default() };
    let expected_timeout = ref_cmds.refuse(1);
    native_a7::age_guild_invite_for_test(Duration::from_secs(2));
    step(&mut p, &host);
    assert_eq!(recorded(&p), vec![expected_timeout], "at 45 s, exact native guild timeout refusal sent");
    assert!(!confirm::is_open(), "open accept confirm cancelled on 45 s timeout");
    host.execute("assert(eqoa.guild.invite() == nil and ev[#ev] == 'closed:guild_invite:timeout')").unwrap();

    // 4. Accepting via player Yes sends exact Cmds::join() bytes (sub 9)
    clear_recorded(&mut p);
    deliver_op(&mut p, 0x057A, pkt_057a(505, "Mira", "Dawn Watch"));
    super::health_redraw(&mut p, false, false);
    step(&mut p, &host);
    ask(&mut p, &host, "return eqoa.actions.guild_accept()");
    assert!(recorded(&p).is_empty(), "nothing sent until player Yes");
    let q = confirm::current_for_test().unwrap();
    assert!(!q.yes && q.text.contains("Mira") && q.text.contains("Dawn Watch"));

    let mut ref_cmds = Cmds { inviter: 505, ..Default::default() };
    let expected_join = ref_cmds.join();
    key(NamedKey::ArrowRight);
    key(NamedKey::Enter);
    step(&mut p, &host);
    assert_eq!(recorded(&p), vec![expected_join], "Yes sends exact Cmds::join bytes");
    host.execute("assert(eqoa.guild.invite() == nil)").unwrap();

    // 5. Leaving the world (Play dropped with quit=true) refuses any held invite so server never waits
    clear_recorded(&mut p);
    deliver_op(&mut p, 0x057A, pkt_057a(509, "Soren", "Night Guard"));
    super::health_redraw(&mut p, false, false);
    step(&mut p, &host);
    let mut ref_cmds = Cmds { inviter: 509, ..Default::default() };
    let expected_drop_refuse = ref_cmds.refuse(1);
    p.quit = true;
    super::dropped(&mut p);
    assert_eq!(recorded(&p), vec![expected_drop_refuse], "leaving world refuses held guild invite");
    assert!(native_a7::guild_invite().is_none());
}

#[test]
fn a7_every_new_event_reaches_lua_with_its_declared_fields_and_the_records_match() {
    let _baseline = Baseline;
    let mut p = ready();
    let host = host_all();
    crate::fork_ext::addons::native_tests::install_field_check(&host).unwrap();
    host.execute("assert(eqoa.windows.takeover('friend_request', {scope='all'}) == true)").unwrap();
    host.execute("assert(eqoa.windows.takeover('guild_invite', {scope='all'}) == true)").unwrap();
    step(&mut p, &host);

    // Populate every A7 state and trigger every event in A7_EVENTS
    deliver_op(&mut p, 0x03D5, pkt_03d5(201, "Alyra", 1, 2, 42, true, "Tunaria"));
    deliver_op(&mut p, 0x1003, pkt_1003("Spammer", 55));
    deliver_op(&mut p, 0x0468, pkt_0468_info("Iron Dawn", "Raid at 8"));
    deliver_op(&mut p, 0x0468, pkt_0468_member(100, "Hero", 1, 2, 30, 3, true));
    deliver_op(&mut p, 0x0468, pkt_0468_member(101, "Cora", 2, 1, 28, 1, true));
    ask(&mut p, &host, "return eqoa.who.request()");
    deliver_op(&mut p, 0x0E02, pkt_0e02(1, &[(2, 3, 45, "Talia", 777)]));
    deliver_op(&mut p, 0x07F5, vec![1, 0, 0, 0]);
    step(&mut p, &host);
    deliver_op(&mut p, 0x0840, pkt_0840(&[("Kael", 31, 4, 1, 888)]));
    deliver_op(&mut p, 0x076F, pkt_076f(401, "Vex"));
    super::health_redraw(&mut p, false, false);
    // Leave guild briefly to test incoming guild_invite record shape
    p.guild.reset();
    deliver_op(&mut p, 0x057A, pkt_057a(501, "Garr", "Silver Hand"));
    super::health_redraw(&mut p, false, false);
    // Restore guild for guild record check
    deliver_op(&mut p, 0x0468, pkt_0468_info("Iron Dawn", "Raid at 8"));
    deliver_op(&mut p, 0x0468, pkt_0468_member(100, "Hero", 1, 2, 30, 3, true));
    step(&mut p, &host);

    // Check every record against its declared fields
    host.execute(
        r#"
        fields('social.Friend', eqoa.social.friends()[1])
        fields('social.Ignored', eqoa.social.ignored()[1])
        fields('social.FriendRequest', eqoa.social.friend_request())
        local g = eqoa.guild.get()
        fields('guild.Guild', g)
        fields('guild.GuildMember', g.members[1])
        local w = eqoa.who.get()
        fields('who.WhoState', w)
        fields('who.WhoEntry', w.entries[1])
        local l = eqoa.lfg.get()
        fields('lfg.LfgState', l, 'registered invites_open members_invite')
        fields('lfg.LfgEntry', l.entries[1])
        fields('Vec3', l.entries[1].position)
        -- Every A7 event arrived with its declared fields
        assert(#friends_ev >= 1 and #ignored_ev >= 1 and #guild_ev >= 1 and #who_ev >= 1 and #lfg_ev >= 1 and #lfg_ch >= 1,
            string.format("events: f=%d i=%d g=%d w=%d l=%d lc=%d", #friends_ev, #ignored_ev, #guild_ev, #who_ev, #lfg_ev, #lfg_ch))
        fields('events.friends.changed.data', friends_ev[#friends_ev], nil, nil, nil, true)
        fields('events.ignored.changed.data', ignored_ev[#ignored_ev], nil, nil, nil, true)
        fields('events.guild.changed.data', guild_ev[#guild_ev], nil, nil, nil, true)
        fields('events.who.result.data', who_ev[#who_ev], nil, nil, nil, true)
        fields('events.lfg.result.data', lfg_ev[#lfg_ev], nil, nil, nil, true)
        fields('events.lfg.changed.data', lfg_ch[#lfg_ch], nil, nil, nil, true)
        "#,
    )
    .unwrap();

    // Also verify guild.GuildInvite record when out of guild
    p.guild.reset();
    deliver_op(&mut p, 0x057A, pkt_057a(502, "Mira", "Dawn Watch"));
    super::health_redraw(&mut p, false, false);
    step(&mut p, &host);
    host.execute("fields('guild.GuildInvite', eqoa.guild.invite())").unwrap();
}

/// Takes both request kinds for the test mod, delivers one friend request and one guild invitation, and steps until
/// the mod holds both (the native boxes never drew).
fn hold_both(p: &mut Play, host: &Host) {
    host.execute("assert(eqoa.windows.takeover('friend_request', {scope='all'}) == true)").unwrap();
    host.execute("assert(eqoa.windows.takeover('guild_invite', {scope='all'}) == true)").unwrap();
    step(p, host);
    deliver_op(p, 0x076F, pkt_076f(401, "Vex"));
    super::health_redraw(p, false, false);
    deliver_op(p, 0x057A, pkt_057a(501, "Garr", "Silver Hand"));
    super::health_redraw(p, false, false);
    step(p, host);
    assert_eq!(p.mode, 0, "both prompts are the mod's: the game's boxes never opened");
    host.execute("assert(eqoa.social.friend_request() ~= nil and eqoa.guild.invite() ~= nil)").unwrap();
}

#[test]
fn a7_zoning_teleport_and_disconnect_refuse_held_requests_with_the_games_bytes() {
    let _baseline = Baseline;
    let mut ref_buddies = crate::social::Buddies::default();
    ref_buddies.request = 401;
    let friend_refusal = ref_buddies.refuse(2);
    let mut ref_cmds = Cmds { inviter: 501, ..Default::default() };
    let guild_refusal = ref_cmds.refuse(1);
    let events = [
        ("teleport", Event::Teleport { world: 1, pos: [0.0; 3], hpr: [0.0; 3], token: 7 }),
        ("world entry", Event::WorldEntry { self_id: 100, player: Default::default(), dump: Default::default() }),
        ("disconnect", Event::Disconnected(3)),
    ];
    for (what, event) in events {
        let mut p = ready();
        let host = host_all();
        hold_both(&mut p, &host);
        clear_recorded(&mut p);
        let note = super::begin_game_event(&p, &event);
        super::end_game_event(&p, note);
        step(&mut p, &host);
        let sent = recorded(&p);
        assert!(sent.contains(&friend_refusal), "{what}: the friend request is refused to the server, got {sent:?}");
        assert!(sent.contains(&guild_refusal), "{what}: the guild invitation is refused to the server, got {sent:?}");
        assert_eq!(sent.len(), 2, "{what}: nothing else is sent");
        host.execute(
            "assert(eqoa.social.friend_request() == nil and eqoa.guild.invite() == nil              and eqoa.windows.session('friend_request') == nil and eqoa.windows.session('guild_invite') == nil)",
        )
        .unwrap();
        // a second step sends nothing more: the refusal is given once
        clear_recorded(&mut p);
        step(&mut p, &host);
        assert!(recorded(&p).is_empty(), "{what}: never refused twice");
    }
    // control: an event that is not the end of the world leaves the held requests held and sends nothing
    let mut p = ready();
    let host = host_all();
    hold_both(&mut p, &host);
    clear_recorded(&mut p);
    let event = Event::State { channel: 1, data: vec![] };
    let note = super::begin_game_event(&p, &event);
    super::end_game_event(&p, note);
    step(&mut p, &host);
    assert!(recorded(&p).is_empty(), "control: nothing is refused while the world stands");
    host.execute("assert(eqoa.social.friend_request() ~= nil and eqoa.guild.invite() ~= nil)").unwrap();
}

#[test]
fn a7_a_mod_that_fails_hands_back_its_held_requests_and_the_next_ones_are_the_games() {
    let _baseline = Baseline;
    // control: a healthy mod keeps both claims and the prompts stay hidden
    let mut p = ready();
    let host = host_all();
    hold_both(&mut p, &host);
    step(&mut p, &host);
    assert!(window_takeover::is_taken(window_takeover::TakeoverKind::FriendRequest), "control: still held");
    assert_eq!(p.mode, 0, "control: hidden");
    drop((p, host));
    // a mod whose handler fails: it loses both claims, the held request is the game's box again with its time left,
    // and the next request is the game's at once (negative control: without the release it stayed hidden and was
    // refused after 45 s)
    let mut p = ready();
    let host = host_all();
    host.execute("eqoa.events.on('windows.session', function(e) if e.data.phase == 'opened' then error('boom') end end)")
        .unwrap();
    host.execute("assert(eqoa.windows.takeover('friend_request', {scope='all'}) == true)").unwrap();
    host.execute("assert(eqoa.windows.takeover('guild_invite', {scope='all'}) == true)").unwrap();
    step(&mut p, &host);
    deliver_op(&mut p, 0x076F, pkt_076f(401, "Vex"));
    super::health_redraw(&mut p, false, false);
    clear_recorded(&mut p);
    step(&mut p, &host); // the session opens, the handler fails, the claims are released
    step(&mut p, &host); // the held request goes back to the game
    assert!(!window_takeover::is_taken(window_takeover::TakeoverKind::FriendRequest), "the failed mod holds nothing");
    assert!(!window_takeover::is_taken(window_takeover::TakeoverKind::GuildInvite), "it holds no guild claim either");
    assert_eq!(p.mode, 17, "the request it could not show is the game's own box");
    assert!(p.buddy_wnd.mbox_visible);
    assert!(recorded(&p).is_empty(), "handing back refuses nothing");
    // the player answers the game's box; the next request is the game's at once, not hidden for a dead mod
    p.mode = 0;
    p.buddies.request = u32::MAX;
    p.buddy_wnd.hide();
    deliver_op(&mut p, 0x076F, pkt_076f(402, "Second"));
    super::health_redraw(&mut p, false, false);
    assert_eq!(p.mode, 17, "the next friend request opens the game's box");
    p.mode = 0;
    deliver_op(&mut p, 0x057A, pkt_057a(502, "Other", "Other Guild"));
    super::health_redraw(&mut p, false, false);
    assert_eq!(p.mode, 20, "the next guild invitation opens the game's box");
}
