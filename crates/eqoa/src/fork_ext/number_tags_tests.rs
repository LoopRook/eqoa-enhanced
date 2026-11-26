// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! `combat.hit`'s attribution (2026-11-26): the number tags are asked for while a mod listens to `combat.hit`, whatever
//! the Number Colors rows say, and a hit the client cannot attribute is nil, never a guess. Included from `play.rs`'s
//! test build (one `#[path]` line) so it reaches `Play`'s own `agree_number_tags` and `damage_indication`.
//!
//! Each test names its negative control.
use super::*;
use crate::fork_ext::addons::{
    host::Host,
    native_core::{self, CORE},
    native_events,
};
use serde_json::json;

type Sent = std::rc::Rc<std::cell::RefCell<Vec<Vec<u8>>>>;

struct Rec(Sent);
impl net::session::Datagrams for Rec {
    fn send_to(&mut self, d: &[u8], _: [u8; 4], _: u16) {
        self.0.borrow_mut().push(d.to_vec());
    }
    fn recv_from(&mut self) -> Option<(Vec<u8>, [u8; 4], u16)> {
        None
    }
}

fn play_with_recorder() -> (Play, Sent) {
    crate::settings::testing::reset(false);
    native_core::fixture_demand(true);
    CORE.with(|c| *c.borrow_mut() = native_core::Core::default());
    let sent: Sent = Default::default();
    let mut s = Session::connect_with(Box::new(Rec(sent.clone())), ([127, 0, 0, 1], 5000), ([127, 0, 0, 1], 10070), "test", [0; 32]).unwrap();
    s.self_id = Some(0x100);
    let p = Play::new(s, Player::default(), Box::default());
    CORE.with(|c| c.borrow_mut().view.player = json!({"id": 0x100, "name": "Me", "pet_id": 0x300}));
    (p, sent)
}

fn answers(p: &mut Play, sent: &Sent) -> Vec<Vec<u8>> {
    p.session.flush();
    let m = super::commands::guaranteed(&sent.borrow()).into_iter().filter(|m| m.starts_with(&[0x39, 0x7E])).collect();
    sent.borrow_mut().clear();
    m
}

/// Every Number Colors row Default: the player asked for no coloured numbers.
fn all_default() {
    let mut c = crate::settings::client();
    c.number_colors = [0; crate::numcolor::ROWS];
    crate::settings::set_client(c);
}

fn host_listening_to(event: &str) -> Host {
    let host = Host::new().unwrap();
    {
        let sess = crate::fork_ext::addons::native::session(&host.lua);
        let mut s = sess.borrow_mut();
        s.addon = "listener".to_string();
        s.native_manifest = true;
        s.capabilities = Some(vec!["eqoa.events.on".to_string()]);
    }
    host.execute(&format!("hits = {{}}; eqoa.events.on('{event}', function(e) hits[#hits + 1] = e.data end)")).unwrap();
    host
}

fn msg(attacker: u32, amount: i64, defender: u32, tag: Option<u8>) -> Vec<u8> {
    let mut w = net::stream::Writer::default();
    w.u32(attacker).signed(amount).u32(defender);
    if let Some(t) = tag {
        w.u8(t);
    }
    w.data
}

/// With every row Default and no mod listening, the server is told to stop (0); while a mod listens to `combat.hit` it
/// is told to send the tags (1), and told to stop again when the mod goes. The control is the first answer: the
/// player's colours alone ask for nothing, so only the listening mod accounts for the 1; and a mod that listens to
/// another event asks for nothing. A server that made no offer is asked nothing either way. The colour of a tagged
/// figure stays the game's with every row Default (`number_colors_paint_the_tagged_numbers`: "Default draws exactly as
/// untagged numbers").
#[test]
fn tags_are_asked_for_while_a_mod_listens_to_combat_hit_whatever_the_colours_say() {
    let (mut p, sent) = play_with_recorder();
    all_default();
    p.agree_number_tags();
    assert!(answers(&mut p, &sent).is_empty(), "no offer: nothing asked, a mod listening or not");
    p.session.number_tags_offer = true;
    p.agree_number_tags();
    assert_eq!(answers(&mut p, &sent), vec![vec![0x39, 0x7E, 0]], "control: every row Default and no mod: stopped");
    let other = host_listening_to("chat.message");
    p.agree_number_tags();
    assert!(answers(&mut p, &sent).is_empty(), "control: a mod listening to another event asks for nothing");
    drop(other);
    let host = host_listening_to("combat.hit");
    p.agree_number_tags();
    assert_eq!(answers(&mut p, &sent), vec![vec![0x39, 0x7E, 1]], "a mod listens to combat.hit: asked, the rows all Default");
    p.agree_number_tags();
    assert!(answers(&mut p, &sent).is_empty(), "asked once");
    drop(host);
    p.agree_number_tags();
    assert_eq!(answers(&mut p, &sent), vec![vec![0x39, 0x7E, 0]], "the mod gone: stopped again");
    crate::settings::testing::reset(false);
}

/// What a mod reads for each state, through `damage_indication` as the server's bytes come (the player 0x100, its pet
/// 0x300, a foe 0x200): the tag read is `attribution "tag"` with true or false, no tag from the player's id is `unknown`
/// with `by_player` and `from_merc` nil, an attacker that is not the player's id is `id` (false, false). Pet hit, merc
/// hit, merc heal, own hit, own heal, a damage shield tick, a proc, a kind not known. The controls are the same bytes
/// with no tag (the old code said `by_player` true for every one that came from the player's id).
#[test]
fn combat_hit_says_who_made_it_or_says_it_cannot() {
    let (mut p, _sent) = play_with_recorder();
    all_default();
    let host = host_listening_to("combat.hit");
    let (me, foe) = (0x100, 0x200);
    p.session.number_tags_offer = true;
    p.damage_indication(&msg(me, -40, foe, Some(0x01)), 1); // its own swing
    p.damage_indication(&msg(me, -9, foe, Some(0x81)), 1); // its pet's swing
    p.damage_indication(&msg(me, -50, foe, Some(0x42)), 1); // its merc's spell
    p.damage_indication(&msg(me, 125, me, Some(0x46)), 1); // its merc's heal on it
    p.damage_indication(&msg(me, 25, me, Some(0x06)), 1); // its own heal on itself
    p.damage_indication(&msg(me, -3, foe, Some(0x04)), 1); // its own damage shield tick
    p.damage_indication(&msg(foe, -12, me, None), 1); // a hit taken: the server tags none
    p.damage_indication(&msg(me, -5, foe, Some(0x05)), 1); // its own proc
    p.damage_indication(&msg(me, -6, foe, Some(0x09)), 1); // a kind a later server sends
    p.damage_indication(&msg(me, -7, foe, Some(0xC1)), 1); // pet and merc bits both set: read as the mercenary's
    // the same player's id with no tag: a server that sends none, or none agreed yet
    p.session.number_tags_offer = false;
    p.damage_indication(&msg(me, -40, foe, None), 1);
    p.damage_indication(&msg(me, 25, me, None), 1);
    native_events::dispatch(&host).unwrap();
    host.execute(
        r#"
        local function chk(i, want, nils)
            local h = hits[i]
            assert(h, 'hit ' .. i)
            for k, v in pairs(want) do
                assert(h[k] == v, string.format('hit %d %s: want %s got %s', i, k, tostring(v), tostring(h[k])))
            end
            for _, k in ipairs(nils or {}) do
                assert(h[k] == nil, string.format('hit %d %s: want nil got %s', i, k, tostring(h[k])))
            end
        end
        assert(#hits == 12, #hits)
        chk(1, {attribution = 'tag', by_player = true,  from_merc = false, hit_kind = 'swing',  kind = 'damage', outgoing = true})
        chk(2, {attribution = 'tag', by_player = false, from_merc = false, hit_kind = 'swing',  outgoing = true})
        chk(3, {attribution = 'tag', by_player = false, from_merc = true,  hit_kind = 'spell',  outgoing = true})
        chk(4, {attribution = 'tag', by_player = false, from_merc = true,  hit_kind = 'heal',   kind = 'heal', incoming = true})
        chk(5, {attribution = 'tag', by_player = true,  from_merc = false, hit_kind = 'heal',   kind = 'heal', outgoing = true, incoming = true})
        chk(6, {attribution = 'tag', by_player = true,  hit_kind = 'shield'})
        chk(7, {attribution = 'id',  by_player = false, from_merc = false, outgoing = false, incoming = true}, {'hit_kind'})
        chk(8, {attribution = 'tag', by_player = true,  hit_kind = 'proc'})
        chk(9, {attribution = 'tag', by_player = true}, {'hit_kind'})
        -- no tag from the player's id: nil, never a guess, and `outgoing` keeps its 0.7.0 meaning
        chk(10, {attribution = 'tag', by_player = false, from_merc = true, hit_kind = 'swing'})
        chk(11, {attribution = 'unknown', outgoing = true, kind = 'damage'}, {'by_player', 'from_merc', 'hit_kind'})
        chk(12, {attribution = 'unknown', outgoing = true, kind = 'heal'}, {'by_player', 'from_merc', 'hit_kind'})
        "#,
    )
    .unwrap();
    crate::settings::testing::reset(false);
}

/// Before the player's own id is known, no tag and an attacker that cannot be compared with it is `unknown`, not `id`
/// (nothing is known about who made it). Negative control: without the `me.is_some()` guard it said `id`, false, false.
#[test]
fn combat_hit_with_the_players_id_not_known_is_unknown_not_id() {
    let (mut p, _sent) = play_with_recorder();
    let host = host_listening_to("combat.hit");
    p.session.number_tags_offer = true;
    p.damage_indication(&msg(0x200, -12, 0x100, None), 1);
    CORE.with(|c| c.borrow_mut().view.player = serde_json::Value::Null);
    native_events::dispatch(&host).unwrap();
    host.execute(
        r#"
        assert(#hits == 1)
        assert(hits[1].attribution == 'unknown' and hits[1].by_player == nil and hits[1].from_merc == nil, tostring(hits[1].attribution))
        "#,
    )
    .unwrap();
    crate::settings::testing::reset(false);
}

/// A server whose client patch carries the colour itself (S2 `damage_rgb`) sends three colour bytes, not a tag: the hit
/// from the player's id is unknown, not the player's (the old code said `by_player` true for a pet's hit there).
#[test]
fn a_patch_servers_colour_bytes_are_no_tag_and_the_hit_is_unknown() {
    let (mut p, _sent) = play_with_recorder();
    let host = host_listening_to("combat.hit");
    p.session.number_tags_offer = true;
    p.damage_indication(&msg(0x100, -40, 0x200, None), 1);
    // three colour bytes (a pet's red): `number_tag` reads none
    let mut body = msg(0x100, -9, 0x200, None);
    body.extend_from_slice(&[0xff, 0, 0]);
    p.damage_indication(&body, 1);
    native_events::dispatch(&host).unwrap();
    host.execute(
        r#"
        assert(#hits == 2)
        for i = 1, 2 do
            assert(hits[i].attribution == 'unknown' and hits[i].by_player == nil and hits[i].from_merc == nil, 'hit ' .. i)
        end
        "#,
    )
    .unwrap();
    crate::settings::testing::reset(false);
}
