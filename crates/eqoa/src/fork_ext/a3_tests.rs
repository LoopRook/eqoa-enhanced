// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A3: frame data and the small actions, from Play-shaped state (what the server sent, what the client
//! holds), through the Lua API a mod sees. Each read and each action has a test with a control: the same call where
//! the data or the permission is absent.
use super::*;
use crate::fork_ext::addons::{
    action_broker,
    host::Host,
    native_a3,
    native_core::{self, CORE, Core},
    native_events,
};
use crate::fork_ext::addons::native_tests::{FRAME_EVENTS, install_field_check};
use net::buffs::Line;
use net::session::{Event, PetEffect, QuestGoal, RaidMember, TrackedQuest};
use net::stream::Writer;

/// The game's side alone: no mod, nothing subscribed.
fn game() -> Play {
    crate::settings::testing::reset(false);
    CORE.with(|c| *c.borrow_mut() = Core::default());
    native_core::fixture_demand(true);
    action_broker::reset_all_for_test();
    native_a3::reset_for_test();
    let mut p = super::super::tests::play();
    let _ = p.loading.wait.take();
    p.hud = Some((
        crate::ui::Vui::new(Default::default(), Default::default(), vec![]),
        Default::default(),
        crate::hud::Hud::new(),
    ));
    // what the client sends is kept in `record`, message by message
    p.session.record = Some(Vec::new());
    p
}

fn world_with(caps: &[&str]) -> (Play, Host) {
    let p = game();
    let host = Host::with_test_capabilities(caps).unwrap();
    host.execute(
        r#"
        results = {}
        eqoa.events.on("action.result", function(e) results[#results + 1] = e.data end)
        "#,
    )
    .unwrap();
    (p, host)
}

fn world() -> (Play, Host) {
    world_with(&["eqoa.actions.*"])
}

/// One drawn frame: the captured state is brought up to date and the mod's events are delivered.
fn frame(p: &Play, h: &Host) {
    h.snapshot.set(p.forge_snapshot(None));
    native_events::dispatch(h).unwrap();
}

/// A game step passes `n` times (the 30 Hz clock `Event.tick` counts).
fn steps(n: u64) {
    CORE.with(|c| c.borrow_mut().tick += n);
}

/// Runs mod code, then lets the broker dispatch it at the next game step and delivers the results.
fn act(p: &mut Play, h: &Host, code: &str) {
    frame(p, h);
    h.execute(code).unwrap();
    super::super::begin_iteration(p, true);
    native_events::dispatch(h).unwrap();
}

fn sent(p: &Play) -> Vec<Vec<u8>> {
    p.session.record.clone().unwrap_or_default().into_iter().map(|(_, d)| d).collect()
}

fn sent_op(p: &Play, op: u16) -> Vec<Vec<u8>> {
    sent(p).into_iter().filter(|d| d.len() >= 2 && d[..2] == op.to_le_bytes()).collect()
}

fn apply(p: &Play, event: Event) {
    let capture = begin_game_event(p, &event);
    end_game_event(p, capture);
}

fn buffs_on(p: &mut Play, lines: Vec<Line>) {
    p.session.buffs_offer = true;
    p.session.agree_buffs(true);
    p.session.buffs = lines;
    p.session.buffs_at = p.session.now().max(1);
}

fn line(whose: u8, removable: bool, id: u32, icon: u32, left_ms: u32, name: &str) -> Line {
    Line { whose, removable, id, icon, left_ms, name: name.into() }
}

fn player_entity(id: u32, name: &str) -> Option<net::entity::Entity> {
    Some(net::entity::Entity {
        id,
        kind: net::entity::TYPE_CREATURE,
        p18: 1,
        p19: 200,
        creature: Some(net::entity::Creature { name: name.into(), p256: 2, p264: 1, ..Default::default() }),
        ..Default::default()
    })
}

fn cast_packet(caster: u32, target: u32, time_ms: u32, effect: u32) -> Event {
    let mut w = Writer::default();
    w.u32(caster);
    w.u32(target);
    w.u8(0);
    w.u32(time_ms);
    w.u32(effect);
    Event::Opcode { op: 0x00B1, data: w.data }
}

// ---- buffs ------------------------------------------------------------------------------------------------------

#[test]
fn a3_buffs_read_time_left_whose_spell_and_removable_and_count_down_on_the_game_step() {
    let (mut p, h) = world();
    // control: no buff window from the server, nothing to read
    frame(&p, &h);
    h.execute("local b, err = eqoa.effects.buffs(); assert(b == nil and err.code == 'unavailable', tostring(err and err.code))")
        .unwrap();

    buffs_on(
        &mut p,
        vec![line(0, true, 9304, 11, 60_000, "Dark Ward"), line(1, false, 77, 12, net::buffs::NO_END, "Plague")],
    );
    frame(&p, &h);
    h.execute(
        r#"
        local b, err = eqoa.effects.buffs()
        assert(err == nil and #b == 2, tostring(err and err.code))
        assert(b[1].index == 1 and b[1].whose == 'player' and b[1].name == 'Dark Ward')
        assert(b[1].spell_id == '9304' and b[1].icon == 11 and b[1].removable == true)
        assert(math.abs(b[1].time_left - 60) < 0.1, b[1].time_left)
        assert(b[2].whose == 'pet' and b[2].removable == false and b[2].time_left == nil, 'no end kept is nil')
        "#,
    )
    .unwrap();
    // ten seconds on the 30 Hz clock: 300 game steps
    steps(300);
    frame(&p, &h);
    h.execute(
        "local b = eqoa.effects.buffs(); assert(math.abs(b[1].time_left - 50) < 0.1, b[1].time_left); assert(b[2].time_left == nil)",
    )
    .unwrap();
    // the player turns Buff Window off: the set is not read any more
    let mut c = crate::settings::client();
    c.buff_window = false;
    crate::settings::set_client(c);
    frame(&p, &h);
    h.execute("local b, err = eqoa.effects.buffs(); assert(b == nil and err.code == 'unavailable')").unwrap();
}

#[test]
fn a3_a_buff_reads_the_description_of_the_players_ability_with_its_name_and_nil_without_one() {
    let (mut p, h) = world();
    p.abilities.insert(
        9304,
        net::dump::Ability { id: 9304, name: "Dark Ward".into(), desc: "Target, Increase armor by 30".into(), ..Default::default() },
    );
    // an ability with no description says nothing, and one with another name is not the effect
    p.abilities.insert(5, net::dump::Ability { id: 5, name: "Plague".into(), desc: String::new(), ..Default::default() });
    p.abilities.insert(6, net::dump::Ability { id: 6, name: "Other".into(), desc: "Not this one".into(), ..Default::default() });
    buffs_on(
        &mut p,
        vec![line(0, true, 9304, 11, 60_000, "Dark Ward"), line(1, false, 77, 12, 30_000, "Plague"), line(0, true, 1, 13, 5_000, "Stranger's Boon")],
    );
    frame(&p, &h);
    h.execute(
        r#"
        local b = eqoa.effects.buffs()
        assert(b[1].description == 'Target, Increase armor by 30', tostring(b[1].description))
        assert(b[2].description == nil, 'an ability with no description gives none')
        assert(b[3].description == nil, 'an effect of a spell the player does not have gives none')
        "#,
    )
    .unwrap();
}

#[test]
fn a3_a_melee_round_is_not_a_cast_but_a_spell_of_the_same_length_is() {
    let (mut p, h) = world();
    p.entities.slots[1] = player_entity(0x2001, "Mira");
    p.target = 0x2001;
    h.execute(
        r#"
        n = 0
        eqoa.events.on('cast.started', function(e) n = n + 1 end)
        "#,
    )
    .unwrap();
    frame(&p, &h);
    // every melee effect the client plays for a swing, at every delay the swings have: no cast, no event
    for k in 0..=11 {
        let effect = vi::swing::swing_effect(k);
        if effect == 0 {
            continue;
        }
        apply(&p, cast_packet(0x2001, 0x100, vi::swing::swing_delay(k) as u32, effect));
    }
    frame(&p, &h);
    h.execute("assert(n == 0 and #eqoa.casts.list() == 0 and eqoa.casts.get(0x2001) == nil, 'a swing is not a cast: ' .. n)").unwrap();
    // control: the same caster, target and time, with the effect of a spell, is a cast with its bar
    apply(&p, cast_packet(0x2001, 0x100, 3500, 0xBEEF));
    frame(&p, &h);
    h.execute("assert(n == 1 and eqoa.casts.get(0x2001).duration == 3.5, 'a real cast of the same length still starts')").unwrap();
    // the player's own swing is no cast either
    apply(&p, cast_packet(0x100, 0x2001, 3500, vi::swing::swing_effect(1)));
    frame(&p, &h);
    h.execute("assert(n == 1 and eqoa.casts.get(0x100) == nil)").unwrap();
}

#[test]
fn a3_the_swing_effects_are_exactly_the_clients_swing_effects() {
    use super::a3::is_melee_swing;
    for name in ["Attack1", "Attack2", "Attack3", "Attack4", "Range1", "Range2", "Range3", "Range4"] {
        assert!(is_melee_swing(vi::hash::hash_resource_id(name)), "{name}");
    }
    assert!(!is_melee_swing(0) && !is_melee_swing(0xAA) && !is_melee_swing(vi::hash::hash_resource_id("Fireball")));
}

#[test]
fn a3_a_cast_of_the_target_is_written_to_the_log_with_its_packet_and_fields() {
    let c = native_a3::Cast {
        caster: 0x2001,
        target: 0x100,
        targets: vec![1, 2],
        effect: 0xBEEF,
        duration_ms: 3500,
        start_tick: 0,
        ends_tick: 105,
        own: false,
    };
    assert_eq!(
        native_a3::cast_log_line("cast.started", &c, None),
        "frames: cast.started caster=0x2001 target=0x100 targets=2 effect=0x0000beef time=3500ms own=false src=0x00B1"
    );
    assert!(native_a3::cast_log_line("cast.ended", &c, Some("elapsed")).ends_with(" reason=elapsed"));
}

#[test]
fn a3_buffs_changed_fires_for_a_new_set_and_for_a_line_running_out_but_not_for_a_resent_set() {
    let (mut p, h) = world();
    h.execute(
        r#"
        changes = {}
        eqoa.events.on('buffs.changed', function(e)
            changes[#changes + 1] = { before = e.data.before and #e.data.before, after = e.data.after and #e.data.after, changed = e.data.changed[1] }
        end)
        "#,
    )
    .unwrap();
    frame(&p, &h);
    buffs_on(&mut p, vec![line(0, true, 1, 11, 2_000, "Short"), line(0, true, 2, 12, 600_000, "Long")]);
    frame(&p, &h);
    h.execute("assert(#changes == 1 and changes[1].before == nil and changes[1].after == 2 and changes[1].changed == 'records', #changes)").unwrap();
    // the same set sent again (new end times, nothing else): no event
    p.session.buffs_at += 1;
    frame(&p, &h);
    h.execute("assert(#changes == 1, 'a re-sent set is not a change: ' .. #changes)").unwrap();
    // the short line's two seconds pass on the game clock: it drops out, with an event
    steps(60);
    frame(&p, &h);
    h.execute("assert(#changes == 2 and changes[2].before == 2 and changes[2].after == 1, #changes); assert(#eqoa.effects.buffs() == 1)").unwrap();
    // the player's own line is not counted in frames drawn: many frames in one game step change nothing
    for _ in 0..20 {
        frame(&p, &h);
    }
    h.execute("assert(#changes == 2, #changes)").unwrap();
}

#[test]
fn a3_remove_effect_sends_the_removal_for_a_removable_buff_and_refuses_the_rest() {
    let (mut p, h) = world();
    buffs_on(
        &mut p,
        vec![line(0, true, 9304, 11, 60_000, "Dark Ward"), line(1, false, 77, 12, net::buffs::NO_END, "Plague")],
    );
    frame(&p, &h);
    act(&mut p, &h, "req, err = eqoa.actions.remove_effect(eqoa.effects.buffs()[1].ref); assert(err == nil, err and err.code)");
    let removal = net::buffs::remove(&line(0, true, 9304, 11, 0, "Dark Ward"));
    assert_eq!(sent_op(&p, net::buffs::OP_BUFF_REMOVE), vec![removal], "0x7E59 names whose, spell id and icon");
    h.execute(
        "local r = results[#results]; assert(r.kind == 'remove_effect' and r.state == 'completed' and r.ok == true, tostring(r.code)); assert(r.detail.sent == true and r.detail.confirmed == false)",
    )
    .unwrap();

    // control: the pet's line is not removable, so nothing is queued and nothing is sent
    let before = sent_op(&p, net::buffs::OP_BUFF_REMOVE).len();
    act(&mut p, &h, "req, err = eqoa.actions.remove_effect(eqoa.effects.buffs()[2].ref); assert(req == nil and err.code == 'not_ready', tostring(err and err.code))");
    assert_eq!(sent_op(&p, net::buffs::OP_BUFF_REMOVE).len(), before);

    // control: a reference from before the list changed is stale
    h.execute("old = eqoa.effects.buffs()[1].ref").unwrap();
    p.session.buffs = vec![line(0, true, 9304, 11, 59_000, "Dark Ward"), line(0, true, 5, 13, 9_000, "New")];
    p.session.buffs_at += 1;
    frame(&p, &h);
    act(&mut p, &h, "req, err = eqoa.actions.remove_effect(old); assert(req == nil and err.code == 'stale_handle', tostring(err and err.code))");
    assert_eq!(sent_op(&p, net::buffs::OP_BUFF_REMOVE).len(), before);

    // control: the line goes between the call and the game step: refused at dispatch, nothing sent
    frame(&p, &h);
    h.execute("req = eqoa.actions.remove_effect(eqoa.effects.buffs()[2].ref); assert(req ~= nil)").unwrap();
    p.session.buffs.retain(|l| l.id != 5);
    super::super::begin_iteration(&mut p, true);
    native_events::dispatch(&h).unwrap();
    h.execute("local r = results[#results]; assert(r.state == 'refused' and r.code == 'stale_handle', tostring(r.code))").unwrap();
    assert_eq!(sent_op(&p, net::buffs::OP_BUFF_REMOVE).len(), before);
}

#[test]
fn a3_a_set_that_came_earlier_is_counted_from_when_it_came_not_from_when_a_mod_first_looks() {
    let (mut p, h) = world();
    buffs_on(&mut p, vec![line(0, true, 1, 11, 60_000, "Ward"), line(0, true, 2, 12, 20_000, "Spent")]);
    // the set came 30 seconds ago, before any mod looked: the 20 s line is over, the 60 s one has 30 s left
    p.session.buffs_at = p.session.now().wrapping_sub(30_000);
    p.session.pet_effects_offer = true;
    p.session.agree_pet_effects(true);
    p.session.pet_effects = vec![PetEffect { icon: 21, left_ms: 30_000, name: "Rage".into() }];
    p.session.pet_effects_at = p.session.now().wrapping_sub(10_000);
    frame(&p, &h);
    h.execute(
        r#"
        local b = eqoa.effects.buffs()
        assert(#b == 1 and b[1].name == 'Ward' and math.abs(b[1].time_left - 30) < 0.3, #b .. ' ' .. tostring(b[1] and b[1].time_left))
        local e = eqoa.effects.pet()
        assert(#e == 1 and math.abs(e[1].time_left - 20) < 0.3, tostring(e[1] and e[1].time_left))
        "#,
    )
    .unwrap();
}

#[test]
fn a3_after_zoning_buffs_and_pet_effects_are_unavailable_until_the_new_zones_set_arrives() {
    let (mut p, h) = world();
    buffs_on(&mut p, vec![line(0, true, 1, 11, 60_000, "Ward")]);
    p.session.pet_effects_offer = true;
    p.session.agree_pet_effects(true);
    p.session.pet_effects = vec![PetEffect { icon: 21, left_ms: 30_000, name: "Rage".into() }];
    p.session.pet_effects_at = p.session.now().max(1);
    frame(&p, &h);
    h.execute("assert(#eqoa.effects.buffs() == 1 and #eqoa.effects.pet() == 1)").unwrap();
    // zoning to another server: the session clears the sets and the offers and keeps `buffs_at` and `pet_effects_at`
    p.session.buffs_offer = false;
    p.session.buffs.clear();
    p.session.pet_effects_offer = false;
    p.session.pet_effects.clear();
    frame(&p, &h);
    h.execute("local b, e = eqoa.effects.buffs(); assert(b == nil and e.code == 'unavailable'); local q, e2 = eqoa.effects.pet(); assert(q == nil and e2.code == 'unavailable')").unwrap();
    // the new server offers the windows again and has not sent a set yet: still unavailable, not an empty list
    p.session.buffs_offer = true;
    p.session.pet_effects_offer = true;
    p.session.agree_pet_effects(false);
    p.session.agree_pet_effects(true);
    frame(&p, &h);
    h.execute("local b, e = eqoa.effects.buffs(); assert(b == nil and e.code == 'unavailable', tostring(b and #b)); local q, e2 = eqoa.effects.pet(); assert(q == nil and e2.code == 'unavailable', tostring(q and #q))").unwrap();
    // its first set arrives: now there is a list
    p.session.buffs = vec![line(0, true, 2, 12, 20_000, "New")];
    p.session.buffs_at = p.session.buffs_at.wrapping_add(7);
    p.session.pet_effects = vec![PetEffect { icon: 22, left_ms: 10_000, name: "Calm".into() }];
    p.session.pet_effects_at = p.session.pet_effects_at.wrapping_add(7);
    frame(&p, &h);
    h.execute("local b = eqoa.effects.buffs(); assert(#b == 1 and b[1].name == 'New'); local q = eqoa.effects.pet(); assert(#q == 1 and q[1].name == 'Calm')").unwrap();
}

#[test]
fn a3_a_frame_where_nothing_changed_builds_and_allocates_nothing() {
    use crate::fork_ext::addons::perf_alloc::measure;
    let (mut p, h) = world();
    p.entities.slots[1] = player_entity(0x2001, "Mira");
    p.target = 0x2001;
    p.target_info = Some(4);
    p.rings.con = Some(1);
    p.target_title = "Warlord".into();
    p.target_guild = "Iron Tide".into();
    buffs_on(&mut p, vec![line(0, true, 1, 11, 600_000, "Ward"), line(1, false, 77, 12, net::buffs::NO_END, "Plague")]);
    p.session.pet_effects_offer = true;
    p.session.agree_pet_effects(true);
    p.session.pet_effects = vec![PetEffect { icon: 21, left_ms: 600_000, name: "Rage".into() }];
    p.session.pet_effects_at = p.session.now().max(1);
    p.session.quest_progress_offer = true;
    p.session.quest_progress =
        vec![TrackedQuest { name: "Bat Eyes".into(), ability: false, goals: vec![QuestGoal { text: "Bat eyes".into(), have: 1, need: 5 }] }];
    p.group.raid = vec![RaidMember { id: 11, name: "Tanky".into(), class: 0, group: 1, ..Default::default() }];
    frame(&p, &h);
    apply(&p, cast_packet(0x2001, 0x100, 9000, 0xCC));
    frame(&p, &h);
    frame(&p, &h);
    // steady frames: the same buffs, goals, target, raid and a cast running
    let (_, _, allocations, _) = measure(|| {
        for _ in 0..20 {
            a3::refresh(&p, None);
        }
    });
    assert_eq!(allocations, 0, "twenty frames of nothing new allocate nothing");
    // control: the meter sees the rebuilds when the source changes (the target's title, a goal, a buff set)
    p.target_title = "Champion".into();
    let (_, _, rebuilt, _) = measure(|| a3::refresh(&p, None));
    assert!(rebuilt > 0, "a changed target is rebuilt");
    p.session.quest_progress[0].goals[0].have = 2;
    let (_, _, rebuilt, _) = measure(|| a3::refresh(&p, None));
    assert!(rebuilt > 0, "a changed goal is rebuilt");
    p.session.buffs_at += 1;
    let (_, _, rebuilt, _) = measure(|| a3::refresh(&p, None));
    assert!(rebuilt > 0, "a re-sent set is read again");
}

// ---- pet effects ------------------------------------------------------------------------------------------------

#[test]
fn a3_pet_effects_read_time_left_and_are_unavailable_without_the_servers_offer() {
    let (mut p, h) = world();
    frame(&p, &h);
    h.execute("local e, err = eqoa.effects.pet(); assert(e == nil and err.code == 'unavailable')").unwrap();
    p.session.pet_effects_offer = true;
    p.session.agree_pet_effects(true);
    p.session.pet_effects = vec![PetEffect { icon: 21, left_ms: 30_000, name: "Rage".into() }];
    p.session.pet_effects_at = p.session.now().max(1);
    frame(&p, &h);
    h.execute(
        "local e, err = eqoa.effects.pet(); assert(err == nil and #e == 1 and e[1].name == 'Rage' and e[1].icon == 21 and math.abs(e[1].time_left - 30) < 0.1)",
    )
    .unwrap();
    steps(31 * 30);
    frame(&p, &h);
    h.execute("local e = eqoa.effects.pet(); assert(#e == 0, 'a line past its time is gone')").unwrap();
}

// ---- casts ------------------------------------------------------------------------------------------------------

#[test]
fn a3_casts_start_count_down_on_the_game_step_and_end_by_time_replacement_or_the_caster_leaving() {
    let (mut p, h) = world();
    p.entities.slots[1] = player_entity(0x2001, "Mira");
    p.entities.slots[2] = player_entity(0x2002, "Bob");
    h.execute(
        r#"
        log = {}
        eqoa.events.on('cast.started', function(e)
            log[#log + 1] = { 'started', e.data.caster_id, e.data.duration, e.data.effect, e.data.own, e.data.target_id }
        end)
        eqoa.events.on('cast.ended', function(e) log[#log + 1] = { 'ended', e.data.caster_id, e.data.reason } end)
        "#,
    )
    .unwrap();
    frame(&p, &h);
    // the player's own cast of two and a half seconds, and another player's
    apply(&p, cast_packet(0x100, 0x2002, 2500, 0xAA));
    apply(&p, cast_packet(0x2001, 0x100, 5000, 0xBB));
    frame(&p, &h);
    h.execute(
        r#"
        assert(#log == 2 and log[1][1] == 'started' and log[1][2] == 0x100 and log[1][3] == 2.5 and log[1][4] == 0xAA and log[1][5] == true and log[1][6] == 0x2002)
        assert(log[2][5] == false)
        local list = eqoa.casts.list()
        assert(#list == 2)
        local mine = eqoa.casts.get(0x100)
        assert(mine.own == true and mine.duration == 2.5 and math.abs(mine.remaining - 2.5) < 0.04 and mine.fraction < 0.05)
        assert(mine.targets and #mine.targets == 0)
        local hers = eqoa.casts.get(eqoa.entities.find(0x2001).ref)
        assert(hers.caster_id == 0x2001 and hers.target_id == 0x100 and hers.own == false)
        assert(eqoa.casts.get(0x9999) == nil, 'a caster with no cast is nil')
        "#,
    )
    .unwrap();
    // a second and a half passes on the game's 30 Hz clock (45 steps), however many frames are drawn
    steps(45);
    for _ in 0..7 {
        frame(&p, &h);
    }
    h.execute(
        "local mine = eqoa.casts.get(0x100); assert(math.abs(mine.fraction - 0.6) < 0.05 and math.abs(mine.remaining - 1.0) < 0.04, mine.fraction .. ' ' .. mine.remaining)",
    )
    .unwrap();
    // her cast is replaced by a new one: the old one ends as replaced
    apply(&p, cast_packet(0x2001, 0x100, 5000, 0xCC));
    frame(&p, &h);
    h.execute("assert(log[3][1] == 'ended' and log[3][2] == 0x2001 and log[3][3] == 'replaced', log[3] and log[3][3]); assert(log[4][1] == 'started' and log[4][4] == 0xCC)").unwrap();
    // the player's own cast runs out: its time is up (the client is not told whether it landed)
    steps(46);
    frame(&p, &h);
    h.execute("assert(log[5][1] == 'ended' and log[5][2] == 0x100 and log[5][3] == 'elapsed', log[5] and log[5][3]); assert(eqoa.casts.get(0x100) == nil)").unwrap();
    // her new cast is cut short by her leaving the scene
    p.entities.slots[1] = None;
    frame(&p, &h);
    h.execute("local last = log[#log]; assert(last[1] == 'ended' and last[2] == 0x2001 and last[3] == 'gone', last[3]); assert(#eqoa.casts.list() == 0)").unwrap();
}

#[test]
fn a3_a_cast_with_no_time_is_announced_and_not_tracked_and_a_death_ends_the_players_own_cast() {
    let (mut p, h) = world();
    h.execute(
        r#"
        n = 0; reasons = {}
        eqoa.events.on('cast.started', function(e) n = n + 1 end)
        eqoa.events.on('cast.ended', function(e) reasons[#reasons + 1] = e.data.reason end)
        "#,
    )
    .unwrap();
    frame(&p, &h);
    apply(&p, cast_packet(0x100, 0, 0, 0xAA));
    frame(&p, &h);
    h.execute("assert(n == 1 and #eqoa.casts.list() == 0, 'announced, not tracked')").unwrap();
    apply(&p, cast_packet(0x100, 0, 9000, 0xAA));
    frame(&p, &h);
    h.execute("assert(#eqoa.casts.list() == 1)").unwrap();
    p.death = Some(crate::death::DeathCam::new([0.; 3], [0., 0., 1.], [0.; 3]));
    frame(&p, &h);
    h.execute("assert(#reasons == 1 and reasons[1] == 'dead' and #eqoa.casts.list() == 0, tostring(reasons[1]))").unwrap();
}

// ---- quest goals ------------------------------------------------------------------------------------------------

#[test]
fn a3_quest_goals_come_only_where_the_server_sends_them_and_match_the_journal() {
    let (mut p, h) = world();
    p.quests.titles = vec![crate::ui::wide("Bat Eyes"), crate::ui::wide("The Militia")];
    p.quests.stamp = 3;
    h.execute(
        r#"
        changes = 0
        eqoa.events.on('quests.goals_changed', function(e) changes = changes + 1 end)
        "#,
    )
    .unwrap();
    // control: a server that sends no goals: unavailable, and the journal still has its titles
    frame(&p, &h);
    h.execute("local g, err = eqoa.quests.goals(); assert(g == nil and err.code == 'unavailable'); assert(#eqoa.quests.list() == 2)").unwrap();

    p.session.quest_progress_offer = true;
    p.session.quest_progress = vec![TrackedQuest {
        name: "Bat Eyes".into(),
        ability: true,
        goals: vec![
            QuestGoal { text: "Bat eyes".into(), have: 3, need: 5 },
            QuestGoal { text: "Return to Ana".into(), have: 0, need: 0 },
        ],
    }];
    frame(&p, &h);
    h.execute(
        r#"
        local g, err = eqoa.quests.goals()
        assert(err == nil and #g == 1, tostring(err and err.code))
        assert(g[1].title == 'Bat Eyes' and g[1].ability == true)
        assert(g[1].quest == eqoa.quests.list()[1].ref, 'tied to the journal entry')
        assert(#g[1].goals == 2 and g[1].goals[1].have == 3 and g[1].goals[1].need == 5 and g[1].goals[1].done == false)
        assert(g[1].goals[2].need == 0 and g[1].goals[2].done == false, 'a thing to do, nothing to count')
        assert(changes == 1, changes)
        "#,
    )
    .unwrap();
    p.session.quest_progress[0].goals[0].have = 5;
    frame(&p, &h);
    h.execute("assert(eqoa.quests.goals()[1].goals[1].done == true and changes == 2, changes)").unwrap();
    // the player turns the Quest Tracker off: not available again
    let mut c = crate::settings::client();
    c.quest_tracker = false;
    crate::settings::set_client(c);
    frame(&p, &h);
    h.execute("local g, err = eqoa.quests.goals(); assert(g == nil and err.code == 'unavailable')").unwrap();
}

// ---- training, breath, target, raid -----------------------------------------------------------------------------

#[test]
fn a3_training_points_breath_and_the_targets_title_guild_and_consider_colour() {
    let (mut p, h) = world();
    p.dump.player.p40 = 4;
    p.dump.player.p44 = 90;
    p.dump.player.p30 = 51;
    p.target = 0x2001;
    p.entities.slots[1] = player_entity(0x2001, "Mira");
    frame(&p, &h);
    h.execute(
        r#"
        local t, err = eqoa.player.training()
        assert(err == nil and t.points == 4 and t.stat_cap == 90)
        local b = eqoa.player.breath()
        assert(math.abs(b.fraction - 0.2) < 0.001, b.fraction)
        -- the walker is not given in this fixture: whether the body is under water is not known
        assert(b.underwater == nil)
        -- the target has not been described by the server yet
        local info = eqoa.target.info()
        assert(info.id == 0x2001 and info.title == nil and info.guild == nil and info.consider == nil)
        "#,
    )
    .unwrap();
    // the server describes the target: a dark-blue consider colour (byte 3), a title and a guild
    p.target_info = Some(4);
    p.rings.con = Some(3);
    p.target_title = "Warlord".into();
    p.target_guild = "Iron Tide".into();
    p.dump.player.p40 = 0;
    frame(&p, &h);
    h.execute(
        r#"
        local info = eqoa.target.info()
        assert(info.title == 'Warlord' and info.guild == 'Iron Tide')
        assert(info.consider.code == 3 and info.consider.name == 'blue')
        assert(info.consider.color.r == 0 and info.consider.color.b == 1 and info.consider.color.a == 1)
        assert(eqoa.player.training().points == 0, 'zero points is a real value')
        "#,
    )
    .unwrap();
    // control: no target is nothing there, not an error
    p.target = u32::MAX;
    frame(&p, &h);
    h.execute("local info, err = eqoa.target.info(); assert(info == nil and err == nil)").unwrap();
}

#[test]
fn a3_breath_knows_the_body_is_under_water_when_the_walk_is_given() {
    let (mut p, h) = world();
    p.dump.player.p30 = 255;
    let w = Play::bundled_ui_walker();
    h.snapshot.set(p.forge_snapshot(Some(&w)));
    native_events::dispatch(&h).unwrap();
    h.execute("local b = eqoa.player.breath(); assert(b.fraction == 1 and b.underwater == false, tostring(b.underwater))").unwrap();
}

#[test]
fn a3_raid_roles_name_the_group_the_role_and_the_marks() {
    let (mut p, h) = world();
    let member = |id: u32, name: &str, class: u8, group: u8, merc: bool| RaidMember {
        id,
        name: name.into(),
        class,
        group,
        merc,
        ..Default::default()
    };
    // a control first: no raid, an empty list
    frame(&p, &h);
    h.execute("local r, err = eqoa.group.raid_roles(); assert(err == nil and #r == 0)").unwrap();
    p.group.raid = vec![
        member(11, "Tanky", 0, 1, false),
        member(12, "Holy", 9, 1, false),
        member(13, "Stabby", 5, 2, false),
        member(14, "Hired", 0, 0, true),
    ];
    p.raid.state.leader = 11;
    p.raid.state.assist = 12;
    frame(&p, &h);
    h.execute(
        r#"
        local r = eqoa.group.raid_roles()
        assert(#r == 4)
        assert(r[1].name == 'Tanky' and r[1].role == 'tank' and r[1].group == 1 and r[1].leader == true and r[1].main_assist == false)
        assert(r[2].role == 'healer' and r[2].main_assist == true and r[2].leader == false and r[2].helper == false)
        assert(r[3].role == 'damage' and r[3].group == 2 and r[3].class_id == 5)
        assert(r[4].role == 'merc' and r[4].merc == true and r[4].group == nil, 'no group where the server sends none')
        "#,
    )
    .unwrap();
}

#[test]
fn a3_reads_say_not_in_world_outside_the_world_and_nothing_of_an_earlier_world_survives_zoning() {
    let (mut p, h) = world();
    buffs_on(&mut p, vec![line(0, true, 1, 11, 60_000, "Ward")]);
    frame(&p, &h);
    h.execute("assert(#eqoa.effects.buffs() == 1)").unwrap();
    // zoning: the generation moves on and the state of the old world is gone
    CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.generation += 1;
        c.view.in_world = false;
    });
    p.session.buffs.clear();
    h.execute(
        r#"
        for _, name in ipairs({ 'effects.buffs', 'effects.pet', 'casts.list', 'quests.goals', 'player.training', 'player.breath', 'target.info', 'group.raid_roles' }) do
            local module, fn = name:match('(.-)%.(.*)')
            local value, err = eqoa[module][fn]()
            assert(value == nil and err and err.code == 'not_in_world', name .. ': ' .. tostring(err and err.code))
        end
        local value, err = eqoa.casts.get(0x100)
        assert(value == nil and err.code == 'not_in_world')
        "#,
    )
    .unwrap();
}

#[test]
fn a3_nothing_is_captured_or_sent_while_no_mod_is_enabled() {
    // control: with a mod wanting the state, the buffs and a cast are taken
    {
        let (mut p, h) = world();
        buffs_on(&mut p, vec![line(0, true, 1, 11, 60_000, "Ward")]);
        frame(&p, &h);
        apply(&p, cast_packet(0x100, 0, 2500, 0xAA));
        frame(&p, &h);
        assert!(native_a3::STATE.with(|s| s.borrow().buffs.is_some() && s.borrow().casts.len() == 1));
    }

    // no mod enabled: the same game state and the same packet leave nothing in the mods' state and send nothing
    let mut p = game();
    native_core::fixture_demand(false);
    buffs_on(&mut p, vec![line(0, true, 1, 11, 60_000, "Ward")]);
    let before = sent(&p).len();
    p.forge_snapshot(None);
    apply(&p, cast_packet(0x100, 0, 2500, 0xAA));
    native_a3::STATE.with(|s| assert!(s.borrow().casts.is_empty(), "a cast is not kept for nobody"));
    p.forge_snapshot(None);
    native_a3::STATE.with(|s| {
        let s = s.borrow();
        assert!(s.buffs.is_none() && s.pet.is_none() && s.goals.is_none() && s.casts.is_empty() && s.raid.is_empty());
    });
    assert_eq!(sent(&p).len(), before, "the frames code sends nothing of its own");
    assert!(!a3::jump_pending());
    native_core::fixture_demand(true);
}

#[test]
fn a3_every_frame_event_reaches_lua_with_its_declared_fields_and_every_record_matches_its_declaration() {
    let (mut p, h) = world();
    install_field_check(&h).unwrap();
    p.entities.slots[1] = player_entity(0x2001, "Mira");
    p.quests.titles = vec![crate::ui::wide("Bat Eyes")];
    p.quests.stamp = 1;
    p.session.quest_progress_offer = true;
    h.lua.globals().set("frame_events", h.lua.create_sequence_from(FRAME_EVENTS.iter().copied()).unwrap()).unwrap();
    h.execute(
        r#"
        seen = {}
        for _, name in ipairs(frame_events) do
            assert(eqoa.capabilities.has('eqoa.events.' .. name), name)
            assert(eqoa.events.on(name, function(e) seen[name] = e; round7c_note(name, e) end))
        end
        "#,
    )
    .unwrap();
    frame(&p, &h);
    // the buff list comes, a goal list comes, and two casts start (one with a caster in view)
    // an ability the effect names, so that its description is a field the record must hold
    p.abilities.insert(9304, net::dump::Ability { id: 9304, name: "Dark Ward".into(), desc: "Target, Increase armor by 30".into(), ..Default::default() });
    buffs_on(&mut p, vec![line(0, true, 9304, 11, 60_000, "Dark Ward"), line(1, false, 77, 12, net::buffs::NO_END, "Plague")]);
    p.session.quest_progress =
        vec![TrackedQuest { name: "Bat Eyes".into(), ability: false, goals: vec![QuestGoal { text: "Bat eyes".into(), have: 1, need: 5 }] }];
    p.session.pet_effects_offer = true;
    p.session.agree_pet_effects(true);
    p.session.pet_effects = vec![PetEffect { icon: 21, left_ms: 30_000, name: "Rage".into() }];
    p.session.pet_effects_at = p.session.now().max(1);
    frame(&p, &h);
    // the lists change again, so that an event has a before as well as an after
    p.session.buffs.pop();
    p.session.buffs_at += 1;
    p.session.quest_progress[0].goals[0].have = 2;
    frame(&p, &h);
    apply(&p, cast_packet(0x2001, 0x100, 1000, 0xBB));
    frame(&p, &h);
    // her cast runs out
    steps(31);
    frame(&p, &h);
    // every record the new reads give is what its declaration says it is
    p.target = 0x2001;
    p.target_info = Some(4);
    p.rings.con = Some(1);
    p.target_title = "Warlord".into();
    p.target_guild = "Iron Tide".into();
    p.group.raid = vec![RaidMember { id: 11, name: "Tanky".into(), class: 0, group: 1, ..Default::default() }];
    frame(&p, &h);
    apply(&p, cast_packet(0x2001, 0x100, 9000, 0xCC));
    frame(&p, &h);
    h.execute(
        r#"
        local missing = {}
        for _, name in ipairs(frame_events) do if not seen[name] then missing[#missing + 1] = name end end
        assert(#missing == 0, 'not emitted: ' .. table.concat(missing, ', '))
        event_fields(seen, frame_events)

        local b = eqoa.effects.buffs()
        fields('effects.Buff', b[1])
        fields('effects.PetEffect', eqoa.effects.pet()[1])
        fields('casts.Cast', eqoa.casts.list()[1], 'target_id')
        local g = eqoa.quests.goals()
        fields('quests.QuestGoals', g[1])
        fields('quests.QuestGoal', g[1].goals[1])
        fields('player.Training', eqoa.player.training())
        fields('player.Breath', eqoa.player.breath(), 'underwater')
        local info = eqoa.target.info()
        fields('target.TargetInfo', info)
        fields('target.Consider', info.consider)
        fields('Color', info.consider.color)
        fields('group.RaidRole', eqoa.group.raid_roles()[1])
        "#,
    )
    .unwrap();
}

// ---- the actions ------------------------------------------------------------------------------------------------

fn invitable(p: &mut Play) {
    p.entities.slots[1] = player_entity(0x2001, "Mira");
    p.target = 0x2001;
}

#[test]
fn a3_group_invite_sends_the_invitation_for_a_player_target_and_refuses_the_rest() {
    let (mut p, h) = world();
    invitable(&mut p);
    act(&mut p, &h, "req, err = eqoa.actions.group_invite(); assert(err == nil, err and err.code)");
    let invites = sent_op(&p, 0x0625);
    assert_eq!(invites.len(), 1, "0x0625 went out");
    assert!(invites[0].windows(4).any(|w| w == 0x2001u32.to_le_bytes()), "it names the target");
    h.execute(
        "local r = results[#results]; assert(r.kind == 'group_invite' and r.state == 'completed' and r.ok == true); assert(r.detail.sent == true and r.detail.confirmed == false)",
    )
    .unwrap();
    assert!(p.group.in_group && p.group.leader, "the game's own function formed the group");

    // controls: nothing more is sent for no target, for an NPC, for the player, for one already in the group
    let before = sent_op(&p, 0x0625).len();
    p.target = u32::MAX;
    act(&mut p, &h, "eqoa.actions.group_invite()");
    h.execute("local r = results[#results]; assert(r.state == 'refused' and r.code == 'not_ready', tostring(r.code))").unwrap();
    p.target = 0x2001;
    p.entities.slots[1].as_mut().unwrap().creature.as_mut().unwrap().p256 = 0;
    act(&mut p, &h, "eqoa.actions.group_invite()");
    h.execute("local r = results[#results]; assert(r.state == 'refused' and r.code == 'not_ready', tostring(r.code))").unwrap();
    p.entities.slots[1].as_mut().unwrap().creature.as_mut().unwrap().p256 = 2;
    p.target = 0x100;
    p.entities.slots[2] = player_entity(0x100, "Me");
    act(&mut p, &h, "eqoa.actions.group_invite()");
    h.execute("local r = results[#results]; assert(r.state == 'refused' and r.code == 'invalid_argument', tostring(r.code))").unwrap();
    assert_eq!(sent_op(&p, 0x0625).len(), before, "no refused call sent anything");
}

#[test]
fn a3_pet_attack_and_back_off_send_the_pet_command_and_refuse_without_a_pet_or_a_target() {
    let (mut p, h) = world();
    invitable(&mut p);
    // control: no pet
    act(&mut p, &h, "eqoa.actions.pet_attack(); eqoa.actions.pet_back_off()");
    assert!(sent_op(&p, 0x0041).is_empty(), "no pet, no command");
    h.execute(
        "assert(#results == 2 and results[1].state == 'refused' and results[1].code == 'not_ready' and results[2].code == 'not_ready')",
    )
    .unwrap();

    p.pet_id = 0x3001;
    act(&mut p, &h, "eqoa.actions.pet_attack()");
    let attack = sent_op(&p, 0x0041);
    assert_eq!(attack.len(), 1);
    let mut expected = vec![0x41, 0x00, 8];
    expected.extend(0x2001u32.to_le_bytes());
    assert_eq!(attack[0], expected, "the pet is sent in on the target (command 8 with its id)");
    h.execute("local r = results[#results]; assert(r.kind == 'pet_attack' and r.state == 'completed' and r.detail.confirmed == false)").unwrap();

    act(&mut p, &h, "eqoa.actions.pet_back_off()");
    let all = sent_op(&p, 0x0041);
    assert_eq!(all.len(), 2);
    assert_eq!(all[1], vec![0x41, 0x00, 7], "back off is command 7, with no target");
    h.execute("local r = results[#results]; assert(r.kind == 'pet_back_off' and r.state == 'completed')").unwrap();

    // control: attack needs a target; back off does not
    p.target = u32::MAX;
    act(&mut p, &h, "eqoa.actions.pet_attack()");
    h.execute("local r = results[#results]; assert(r.state == 'refused' and r.code == 'not_ready')").unwrap();
    assert_eq!(sent_op(&p, 0x0041).len(), 2);
}

#[test]
fn a3_pet_attack_refuses_target_zero_and_remove_effect_re_checks_the_buff_window_when_it_runs() {
    let (mut p, h) = world();
    p.pet_id = 0x3001;
    p.target = 0;
    act(&mut p, &h, "eqoa.actions.pet_attack()");
    h.execute("local r = results[#results]; assert(r.state == 'refused' and r.code == 'not_ready', tostring(r.code))").unwrap();
    assert!(sent_op(&p, 0x0041).is_empty());

    buffs_on(&mut p, vec![line(0, true, 9304, 11, 60_000, "Dark Ward")]);
    frame(&p, &h);
    h.execute("req = eqoa.actions.remove_effect(eqoa.effects.buffs()[1].ref); assert(req ~= nil)").unwrap();
    // Buff Window is turned off before the game step that would send it
    let mut c = crate::settings::client();
    c.buff_window = false;
    crate::settings::set_client(c);
    super::super::begin_iteration(&mut p, true);
    native_events::dispatch(&h).unwrap();
    h.execute("local r = results[#results]; assert(r.state == 'refused' and r.code == 'unavailable', tostring(r.code))").unwrap();
    assert!(sent_op(&p, net::buffs::OP_BUFF_REMOVE).is_empty(), "nothing was sent");
}

#[test]
fn a3_emote_sends_the_games_emote_and_refuses_an_unknown_name() {
    let (mut p, h) = world();
    act(&mut p, &h, "req, err = eqoa.actions.emote('Wave'); assert(err == nil, err and err.code)");
    assert_eq!(sent_op(&p, 30).len(), 1, "the wave's opcode (30) went out");
    assert_eq!(p.emote, Some(56), "the player's own action follows, as for the menu's entry");
    h.execute("local r = results[#results]; assert(r.kind == 'emote' and r.state == 'completed' and r.detail.emote == 'wave')").unwrap();
    act(&mut p, &h, "eqoa.actions.emote('bow')");
    assert_eq!(sent_op(&p, 66).len(), 1, "the second emote has its own opcode");
    // controls: a name the game has no emote for, and a wrong type, are refused when asked
    h.execute(
        r#"
        local r, err = eqoa.actions.emote('flip'); assert(r == nil and err.code == 'invalid_argument')
        r, err = eqoa.actions.emote(3); assert(r == nil and err.code == 'invalid_argument')
        r, err = eqoa.actions.emote(); assert(r == nil and err.code == 'invalid_argument')
        "#,
    )
    .unwrap();
    // dead: nothing sent
    p.death = Some(crate::death::DeathCam::new([0.; 3], [0., 0., 1.], [0.; 3]));
    let before = sent(&p).len();
    act(&mut p, &h, "eqoa.actions.emote('wave')");
    h.execute("local r = results[#results]; assert(r.state == 'refused' and r.code == 'not_ready')").unwrap();
    assert_eq!(sent(&p).len(), before);
}

#[test]
fn a3_jump_asks_the_walk_where_the_server_offers_it_and_is_unsupported_elsewhere() {
    let (mut p, h) = world();
    // control: a server that never offered the jump
    act(&mut p, &h, "eqoa.actions.jump()");
    h.execute("local r = results[#results]; assert(r.state == 'refused' and r.code == 'unsupported', tostring(r.code))").unwrap();
    assert!(!a3::jump_pending());

    p.session.jumps = true;
    act(&mut p, &h, "eqoa.actions.jump()");
    h.execute("local r = results[#results]; assert(r.kind == 'jump' and r.state == 'completed' and r.detail.confirmed == false)").unwrap();
    // P3: `requested`, never `asked` (a mod that waits for `confirm.closed` on `asked` would wait forever on a jump)
    h.execute("local r = results[#results]; assert(r.detail.requested == true and r.detail.asked == nil and r.detail.confirm == nil)").unwrap();
    assert!(a3::jump_pending(), "the walk is asked at the next frame that has it");
    let mut w = Play::bundled_ui_walker();
    a3::apply_jump(&mut p, &mut w);
    assert!(!a3::jump_pending(), "the request was taken once");
    a3::apply_jump(&mut p, &mut w);
    assert!(!a3::jump_pending());
}

#[test]
fn a3_every_action_needs_its_own_capability_and_the_label_names_it() {
    let (mut p, h) = world_with(&["eqoa.actions.emote"]);
    invitable(&mut p);
    p.pet_id = 0x3001;
    frame(&p, &h);
    h.execute(
        r#"
        for _, name in ipairs({ 'group_invite', 'pet_attack', 'pet_back_off', 'jump' }) do
            local r, err = eqoa.actions[name]()
            assert(r == nil and err.code == 'permission_denied', name .. ': ' .. tostring(err and err.code))
        end
        local r, err = eqoa.actions.emote('wave'); assert(err == nil, err and err.code)
        r, err = eqoa.actions.remove_effect(nil); assert(r == nil and err.code == 'permission_denied', tostring(err and err.code))
        "#,
    )
    .unwrap();
    // a mod with no capability list at all is refused with not_declared
    let bare = Host::new().unwrap();
    bare.execute("local r, err = eqoa.actions.jump(); assert(r == nil and err.code == 'not_declared', tostring(err and err.code))")
        .unwrap();
    // each new name is part of the supported surface
    h.execute(
        r#"
        for _, path in ipairs({
            'eqoa.actions.group_invite', 'eqoa.actions.pet_attack', 'eqoa.actions.pet_back_off', 'eqoa.actions.emote',
            'eqoa.actions.jump', 'eqoa.actions.remove_effect', 'eqoa.effects.buffs', 'eqoa.effects.pet', 'eqoa.casts.list',
            'eqoa.casts.get', 'eqoa.quests.goals', 'eqoa.player.training', 'eqoa.player.breath', 'eqoa.target.info',
            'eqoa.group.raid_roles', 'eqoa.events.buffs.changed', 'eqoa.events.cast.started', 'eqoa.events.cast.ended',
            'eqoa.events.quests.goals_changed', 'eqoa.effects.Buff.time_left', 'eqoa.casts.Cast.fraction',
        }) do
            assert(eqoa.capabilities.has(path), path)
        end
        assert(not eqoa.capabilities.has('eqoa.actions.pet_dismiss'), 'a name that is not part of the API')
        "#,
    )
    .unwrap();
}
