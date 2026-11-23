// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A7 on `Play`: captures friends, ignore, guild, who, group search and incoming social requests into
//! `addons::native_a7`, runs the 45-second timeout on held `friend_request`/`guild_invite` sessions, and dispatches
//! community actions through the game's own public builders (`social.rs`, `guild.rs`, `wholist.rs`, `groupsearch.rs`).
//! A child of the Play adapter, so it adds zero lines to any upstream file.
use super::Play;
use crate::fork_ext::addons::{
    action_broker::DispatchOutcome,
    confirm::{self, Act},
    native_a7::{
        self as a7, A7Action, FriendEntry, GuildMemberSnap, GuildSnap, IdOrName, IgnoredEntry, LfgEntrySnap, LfgSnap,
        WhoEntrySnap, WhoSnap,
    },
    player::{CLASSES, RACES},
    window_sessions,
    window_takeover::{self, TakeoverKind},
};
use crate::groupsearch::{Lfg, Query};
use crate::guild::Cmds;
use crate::social::Ignore;
use serde_json::json;
use std::hash::{Hash, Hasher};

fn hashed(f: impl FnOnce(&mut std::collections::hash_map::DefaultHasher)) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    f(&mut h);
    h.finish()
}

fn class_name(id: i32) -> Option<String> {
    usize::try_from(id).ok().and_then(|i| CLASSES.get(i)).map(|s| (*s).to_string())
}

fn race_name(id: i32) -> Option<String> {
    usize::try_from(id).ok().and_then(|i| RACES.get(i)).map(|s| (*s).to_string())
}

const RANK_WORDS: [i32; 4] = [541, 540, 539, 538];
const RANK_FALLBACK: [&str; 4] = ["Member", "Senior", "Officer", "Leader"];

fn rank_name(play: &Play, rank: i32) -> String {
    let idx = usize::try_from(rank).ok().filter(|&i| i < 4);
    if let Some(i) = idx {
        if let Some(w) = play.words.as_ref() {
            let s = w.get(RANK_WORDS[i], 0);
            if !s.is_empty() && s != "(null)" {
                return s.to_string();
            }
        }
        return RANK_FALLBACK[i].to_string();
    }
    format!("Rank {rank}")
}

fn lfg_range(play: &Play) -> (i32, i32) {
    let own = (play.dump.player.level as i32).max(1);
    if play.group.in_group && play.group.count() > 0 {
        // a group is small (eight slots are plenty): no allocation, so the per-step refresh costs nothing when no mod listens
        let mut levels = [own; 8];
        let n = play.group.count().min(8);
        for (slot, m) in levels.iter_mut().zip(&play.group.members[..n]) {
            *slot = play
                .entities
                .slots
                .iter()
                .flatten()
                .find(|e| e.id == m.id)
                .and_then(|e| e.creature.as_ref())
                .map_or(own, |c| c.level as i32);
        }
        crate::groupsearch::level_range(&levels[..n])
    } else {
        crate::groupsearch::level_range(&[own])
    }
}

fn build_friends(play: &Play) -> Vec<FriendEntry> {
    play.buddies
        .map
        .values()
        .map(|b| FriendEntry {
            id: b.id,
            name: b.name.clone(),
            online: b.online,
            class_id: b.class,
            class_name: class_name(b.class),
            race_id: b.race,
            race_name: race_name(b.race),
            level: b.level,
            world: b.world.clone(),
        })
        .collect()
}

fn build_ignored(play: &Play) -> Vec<IgnoredEntry> {
    play.ignore
        .list
        .iter()
        .map(|e| IgnoredEntry { id: e.id, name: e.name.clone() })
        .collect()
}

fn build_guild(play: &Play) -> GuildSnap {
    let g = &play.guild;
    let me = play.dump.player.id as i32;
    let self_rank = g.in_guild.then(|| g.rank(me));
    let members = g
        .members
        .values()
        .map(|m| GuildMemberSnap {
            id: m.id,
            name: m.name.clone(),
            class_id: m.class,
            class_name: class_name(m.class),
            race_id: m.race,
            race_name: race_name(m.race),
            level: m.level,
            rank: m.rank,
            rank_name: rank_name(play, m.rank),
            online: m.online,
            can_promote: Cmds::can_promote(g, me, m.id) == 0,
            can_demote: Cmds::can_demote(g, me, m.id) == 0,
            can_kick: Cmds::can_kick(g, me, m.id) == 0,
        })
        .collect();
    GuildSnap {
        in_guild: g.in_guild,
        name: g.name.clone(),
        motd: g.motd.clone(),
        show_chat: g.show_chat,
        self_rank,
        self_rank_name: self_rank.map(|r| rank_name(play, r)),
        can_invite: Cmds::can_invite(g, me) == 0,
        can_set_motd: Cmds::can_lead(g, me) == 0,
        members,
    }
}

fn build_who(play: &Play) -> WhoSnap {
    WhoSnap {
        pending: play.who.pending,
        received: play.who.received,
        entries: play
            .who
            .list
            .iter()
            .map(|r| WhoEntrySnap {
                id: r.x1c,
                name: r.name.clone(),
                race_id: r.race as i32,
                race_name: race_name(r.race as i32),
                class_id: r.class as i32,
                class_name: class_name(r.class as i32),
                level: r.level as i32,
            })
            .collect(),
    }
}

fn build_lfg(play: &Play) -> LfgSnap {
    let available = crate::live::features().live();
    let (min, max) = lfg_range(play);
    let reg = play.session.group_reg;
    LfgSnap {
        available,
        busy: play.live.part.busy,
        registered: reg.map(|g| g.registered),
        invites_open: reg.map(|g| g.invites_open),
        members_invite: reg.map(|g| g.members_invite),
        default_min_level: min,
        default_max_level: max,
        entries: play
            .live
            .part
            .found
            .iter()
            .map(|f| LfgEntrySnap {
                id: f.id,
                name: f.name.clone(),
                race_id: f.race,
                race_name: race_name(f.race),
                class_id: f.class,
                class_name: class_name(f.class),
                level: f.level as i32,
                world_id: f.world,
                position: f.pos,
            })
            .collect(),
    }
}

fn refresh_friends(play: &Play) {
    let hash = hashed(|h| {
        play.buddies.map.len().hash(h);
        for b in play.buddies.map.values() {
            (b.id, &b.name, b.class, b.race, b.level, b.online, &b.world).hash(h);
        }
    });
    a7::sync_friends(hash, || build_friends(play));
}

fn refresh_ignored(play: &Play) {
    let hash = hashed(|h| {
        play.ignore.list.len().hash(h);
        for e in &play.ignore.list {
            (e.id, &e.name).hash(h);
        }
    });
    a7::sync_ignored(hash, || build_ignored(play));
}

fn refresh_guild(play: &Play) {
    let me = play.dump.player.id as i32;
    let g = &play.guild;
    let hash = hashed(|h| {
        (me, g.in_guild, &g.name, &g.motd, g.show_chat, g.members.len()).hash(h);
        for m in g.members.values() {
            (m.id, &m.name, m.class, m.race, m.level, m.rank, m.online).hash(h);
        }
    });
    a7::sync_guild(hash, || build_guild(play));
}

fn refresh_lfg(play: &Play, fire_result: bool) {
    let available = crate::live::features().live();
    let (min, max) = lfg_range(play);
    let reg = play.session.group_reg;
    let meta_hash = hashed(|h| {
        (
            available,
            play.live.part.busy,
            reg.map(|g| (g.registered, g.invites_open, g.members_invite)),
            min,
            max,
            play.live.part.found.len(),
        )
            .hash(h);
        for f in &play.live.part.found {
            (f.id, &f.name, f.level, f.race, f.class, f.world).hash(h);
        }
    });
    a7::sync_lfg(meta_hash, || build_lfg(play), fire_result);
}

pub(crate) struct EventNote {
    op: Option<u16>,
    mode_before: i32,
    who_received_before: u64,
    lfg_sub_40: bool,
    reset: bool,
}

pub(crate) fn begin_event(play: &Play, event: &net::session::Event) -> EventNote {
    let (op, lfg_sub_40, reset) = match event {
        net::session::Event::Opcode { op, data } => {
            (Some(*op), *op == 0x0840 && data.first() == Some(&0x40), false)
        }
        net::session::Event::WorldEntry { .. }
        | net::session::Event::Teleport { .. }
        | net::session::Event::Disconnected(_) => (None, false, true),
        _ => (None, false, false),
    };
    EventNote {
        op,
        mode_before: play.mode,
        who_received_before: play.who.received,
        lfg_sub_40,
        reset,
    }
}

pub(crate) fn end_event(play: &Play, note: EventNote) {
    if note.reset {
        // zoning, a teleport, a new world entry or a lost connection: the server must still hear a refusal for a
        // request a mod holds (the next step sends it, `refuse_after_reset`), before the held state is wiped
        a7::note_reset_refusals();
        a7::forget();
        return;
    }
    // no mod listens: nothing is hashed or built (the zero-addon and disabled-VM allocation gates); a mod enabled
    // later is brought up to date by the next step's refresh in `tick`
    if !crate::fork_ext::addons::native_core::capture_requested() {
        return;
    }
    match note.op {
        Some(0x03D5 | 0x03D6) => refresh_friends(play),
        Some(0x1003..=0x1006) => refresh_ignored(play),
        Some(0x0468) => refresh_guild(play),
        Some(0x0E02) => {
            let received = play.who.received != note.who_received_before;
            a7::set_who(build_who(play), received);
        }
        Some(0x0840) if note.lfg_sub_40 => {
            refresh_lfg(play, true);
        }
        Some(0x07F5) => {
            refresh_lfg(play, false);
        }
        Some(0x076F) if note.mode_before == 0 && play.mode == 17 && play.buddies.request != u32::MAX => {
            a7::on_incoming_friend_request(play.buddies.request, play.buddies.request_name.clone());
            window_sessions::sync_held_request(TakeoverKind::FriendRequest, a7::held_friend_holder(), "ended");
        }
        Some(0x057A) if note.mode_before == 0 && play.mode == 20 && play.guild_cmds.inviter != u32::MAX => {
            a7::on_incoming_guild_invite(
                play.guild_cmds.inviter,
                play.guild_cmds.name.clone(),
                play.guild_cmds.guild.clone(),
            );
            window_sessions::sync_held_request(TakeoverKind::GuildInvite, a7::held_guild_holder(), "ended");
        }
        _ => {}
    }
}

/// Called right before `draw_hud` (from `health_redraw`) and at the start of `tick` so a taken incoming
/// friend request (mode 17) or guild invitation (mode 20) never draws a single frame natively.
pub(crate) fn hide_taken_requests(play: &mut Play) {
    let (hide_friend, hide_guild, second_friend, second_guild) = a7::take_pending_hide();
    if let Some(new_id) = second_friend {
        // Busy refusal for the second request while the first stays held.
        let mut w = net::stream::Writer::op(0x368);
        w.u32(new_id).u8(1);
        play.session.send(&w.data);
        if let Some(first) = a7::friend_request() {
            play.buddies.request = first.id;
            play.buddies.request_name = first.name;
        }
        play.buddy_wnd.hide();
        if play.mode == 17 {
            play.mode = 0;
        }
    } else if hide_friend {
        play.buddy_wnd.hide();
        if play.mode == 17 {
            play.mode = 0;
        }
    }
    if let Some(new_inviter) = second_guild {
        let mut w = net::stream::Writer::op(crate::guild::OP);
        w.u8(10).u32(new_inviter).u8(2);
        play.session.send(&w.data);
        if let Some(first) = a7::guild_invite() {
            play.guild_cmds.inviter = first.inviter_id;
            play.guild_cmds.name = first.inviter;
            play.guild_cmds.guild = first.guild;
        }
        play.guild_wnd.reset();
        play.guild_open = None;
        play.guild_keys.clear();
        if play.mode == 20 {
            play.mode = 0;
        }
    } else if hide_guild {
        play.guild_wnd.reset();
        play.guild_open = None;
        play.guild_keys.clear();
        if play.mode == 20 {
            play.mode = 0;
        }
    }
}

fn show_native_friend_request(play: &mut Play, id: u32, name: &str, remaining_ms: i64) {
    play.buddies.request = id;
    play.buddies.request_name = name.to_string();
    let now = play.hud_last;
    let words = play.words.clone();
    let w = |wid: i32| words.as_ref().map_or_else(String::new, |w| w.get(wid, 0).to_string());
    play.buddy_wnd.show_request(&w, name, now);
    play.buddy_wnd.mbox.timeout = now + remaining_ms.max(1000);
    play.mode = 17;
}

fn show_native_guild_invite(play: &mut Play, inviter_id: u32, inviter: &str, guild: &str, remaining_ms: i64) {
    play.guild_cmds.inviter = inviter_id;
    play.guild_cmds.name = inviter.to_string();
    play.guild_cmds.guild = guild.to_string();
    let now = play.hud_last;
    let words = play.words.clone();
    let w = |wid: i32| words.as_ref().map_or_else(String::new, |w| w.get(wid, 0).to_string());
    play.guild_wnd.show_invite(&w, &play.guild_cmds, now);
    play.guild_wnd.mbox.timeout = now + remaining_ms.max(1000);
    play.guild_open = None;
    play.guild_keys.clear();
    play.mode = 20;
}

/// Called from `a1_play::tick` when a mod calls `eqoa.windows.pass` on a held `friend_request` or `guild_invite`.
pub(crate) fn pass_request(play: &mut Play, vm: u64, kind: TakeoverKind) -> bool {
    match kind {
        TakeoverKind::FriendRequest => {
            let Some(req) = a7::friend_request() else { return false };
            if req.held_by.as_ref().is_none_or(|(h, _)| *h != vm) {
                return false;
            }
            let rem_ms = (req.seconds_remaining() * 1000.0) as i64;
            confirm::cancel_kind("friend_accept", "passed");
            a7::hand_back_friend_request();
            show_native_friend_request(play, req.id, &req.name, rem_ms);
            true
        }
        TakeoverKind::GuildInvite => {
            let Some(inv) = a7::guild_invite() else { return false };
            if inv.held_by.as_ref().is_none_or(|(h, _)| *h != vm) {
                return false;
            }
            let rem_ms = (inv.seconds_remaining() * 1000.0) as i64;
            confirm::cancel_kind("guild_accept", "passed");
            a7::hand_back_guild_invite();
            show_native_guild_invite(play, inv.inviter_id, &inv.inviter, &inv.guild, rem_ms);
            true
        }
        _ => false,
    }
}

fn refuse_held_friend(play: &mut Play, id: u32, reason: &'static str) {
    play.buddies.request = id;
    let req = play.buddies.refuse(2);
    play.session.send(&req);
    play.buddy_wnd.hide();
    if play.mode == 17 {
        play.mode = 0;
    }
    confirm::cancel_kind("friend_accept", reason);
    a7::clear_friend_request();
    window_sessions::sync_held_request(TakeoverKind::FriendRequest, None, reason);
}

fn refuse_held_guild(play: &mut Play, inviter_id: u32, reason: &'static str) {
    play.guild_cmds.inviter = inviter_id;
    let m = play.guild_cmds.refuse(1);
    play.session.send(&m);
    play.guild_wnd.reset();
    play.guild_open = None;
    if play.mode == 20 {
        play.mode = 0;
    }
    confirm::cancel_kind("guild_accept", reason);
    a7::clear_guild_invite();
    window_sessions::sync_held_request(TakeoverKind::GuildInvite, None, reason);
}

/// Runs each step from `begin_iteration`: hides taken requests, runs the 45-second timeout and holder checks so
/// held requests are never left hanging, and refreshes community snapshots.
/// Refuses the requests that were held when the world ended (`end_event`), with the game's own bytes.
fn refuse_after_reset(play: &mut Play) {
    let (friends, guilds) = a7::take_reset_refusals();
    for id in friends {
        refuse_held_friend(play, id, "left_world");
    }
    for id in guilds {
        refuse_held_guild(play, id, "left_world");
    }
}

pub(crate) fn tick(play: &mut Play) {
    refuse_after_reset(play);
    hide_taken_requests(play);
    a7::enter_generation();
    let now = play.session.now() as i64;
    if play.live.part.give_up(now) {
        refresh_lfg(play, false);
    }

    let away = !play.in_world_now() || play.world_change.is_some();
    let dead = play.is_dead();

    // Held friend_request lifecycle (45 s timeout, holder gone, game windows switch, death/away)
    if let Some(req) = a7::friend_request()
        && let Some((vm, addon)) = req.held_by.clone()
    {
        if req.expired() {
            refuse_held_friend(play, req.id, "timeout");
        } else if away {
            refuse_held_friend(play, req.id, "left_world");
        } else if dead {
            refuse_held_friend(play, req.id, "death");
        } else if window_takeover::use_game_windows()
            || window_takeover::holder_vm(TakeoverKind::FriendRequest) != Some(vm)
        {
            let reason = if window_takeover::use_game_windows() { "game_windows" } else { "released" };
            if play.mode == 0 {
                let rem_ms = (req.seconds_remaining() * 1000.0) as i64;
                confirm::cancel_kind("friend_accept", reason);
                a7::hand_back_friend_request();
                show_native_friend_request(play, req.id, &req.name, rem_ms);
                window_sessions::sync_held_request(TakeoverKind::FriendRequest, None, reason);
            } else {
                refuse_held_friend(play, req.id, reason);
            }
        } else {
            window_sessions::sync_held_request(TakeoverKind::FriendRequest, Some((vm, addon)), "ended");
        }
    } else {
        window_sessions::sync_held_request(TakeoverKind::FriendRequest, None, "ended");
    }

    // Held guild_invite lifecycle (45 s timeout, holder gone, game windows switch, death/away, already in guild)
    if let Some(inv) = a7::guild_invite()
        && let Some((vm, addon)) = inv.held_by.clone()
    {
        if inv.expired() {
            refuse_held_guild(play, inv.inviter_id, "timeout");
        } else if away {
            refuse_held_guild(play, inv.inviter_id, "left_world");
        } else if dead {
            refuse_held_guild(play, inv.inviter_id, "death");
        } else if play.guild.in_guild {
            refuse_held_guild(play, inv.inviter_id, "in_guild");
        } else if window_takeover::use_game_windows()
            || window_takeover::holder_vm(TakeoverKind::GuildInvite) != Some(vm)
        {
            let reason = if window_takeover::use_game_windows() { "game_windows" } else { "released" };
            if play.mode == 0 {
                let rem_ms = (inv.seconds_remaining() * 1000.0) as i64;
                confirm::cancel_kind("guild_accept", reason);
                a7::hand_back_guild_invite();
                show_native_guild_invite(play, inv.inviter_id, &inv.inviter, &inv.guild, rem_ms);
                window_sessions::sync_held_request(TakeoverKind::GuildInvite, None, reason);
            } else {
                refuse_held_guild(play, inv.inviter_id, reason);
            }
        } else {
            window_sessions::sync_held_request(TakeoverKind::GuildInvite, Some((vm, addon)), "ended");
        }
    } else {
        window_sessions::sync_held_request(TakeoverKind::GuildInvite, None, "ended");
    }

    // nothing is captured, hashed, copied or built while no mod listens (the zero-addon allocation gate): not the
    // lists, and not the names of the game's own request box either
    if !crate::fork_ext::addons::native_core::capture_requested() {
        return;
    }

    // Mirror native unheld prompts for read-only inspection
    let native_friend = (play.mode == 17 && play.buddies.request != u32::MAX)
        .then(|| (play.buddies.request, play.buddies.request_name.clone()));
    let native_guild = (play.mode == 20 && play.guild_cmds.inviter != u32::MAX)
        .then(|| (play.guild_cmds.inviter, play.guild_cmds.name.clone(), play.guild_cmds.guild.clone()));
    a7::sync_native_requests(native_friend, native_guild);

    if !away {
        refresh_friends(play);
        refresh_ignored(play);
        refresh_guild(play);
        a7::set_who(build_who(play), false);
        refresh_lfg(play, false);
    }
}

/// Refuses any held request before `Play` drops on logout/disconnect so the server never waits.
pub(crate) fn play_dropped(play: &mut Play, leaving: bool) {
    if !leaving {
        return;
    }
    refuse_after_reset(play);
    if let Some(req) = a7::friend_request()
        && req.held_by.is_some()
    {
        refuse_held_friend(play, req.id, "left_world");
    }
    if let Some(inv) = a7::guild_invite()
        && inv.held_by.is_some()
    {
        refuse_held_guild(play, inv.inviter_id, "left_world");
    }
    a7::forget();
}

// ---- Actions & A1 confirms ----

fn refuse(code: &'static str, reason: impl Into<String>) -> DispatchOutcome {
    DispatchOutcome::Refused { code, reason: reason.into() }
}

fn sent() -> DispatchOutcome {
    DispatchOutcome::Completed { detail: Some(json!({"sent": true, "confirmed": false})) }
}

fn ask_confirm(act: Act, text: String, session_kinds: &[&str]) -> DispatchOutcome {
    let Some(vm) = crate::fork_ext::addons::action_broker::current_dispatch_vm() else {
        return refuse("not_ready", "no mod is asking");
    };
    let addon = crate::fork_ext::addons::action_broker::dispatch_addon().unwrap_or_default();
    let session = session_kinds
        .iter()
        .find_map(|k| window_sessions::get(vm, k).map(|s| s.id));
    match confirm::open(vm, &addon, session, act, text, 0) {
        Ok(id) => DispatchOutcome::Completed { detail: Some(json!({"asked": true, "confirm": id})) },
        Err((code, msg)) => refuse(code, msg),
    }
}

fn resolve_guild_member(play: &Play, target: &IdOrName) -> Option<(i32, String)> {
    match target {
        IdOrName::Id(id) => i32::try_from(*id)
            .ok()
            .and_then(|id| play.guild.members.get(&id))
            .map(|m| (m.id, m.name.clone())),
        IdOrName::Name(name) => play
            .guild
            .members
            .values()
            .find(|m| m.name.eq_ignore_ascii_case(name))
            .map(|m| (m.id, m.name.clone())),
    }
}

pub(crate) fn dispatch(play: &mut Play, action: &A7Action) -> DispatchOutcome {
    if play.is_dead() {
        return refuse("not_ready", "cannot do this while dead");
    }
    let plain = matches!(play.mode, 0 | 38);
    let request_ok = matches!(
        action,
        A7Action::FriendAccept | A7Action::FriendRefuse | A7Action::GuildAccept | A7Action::GuildRefuse
    );
    if !request_ok && !plain {
        return refuse("not_ready", format!("cannot do this while a window is up (mode {})", play.mode));
    }

    match action {
        A7Action::FriendAdd { name } => {
            if play.buddies.count() >= 32 {
                return refuse("not_ready", "your friend list is full (32)");
            }
            match play.buddies.add(&play.player.name, name) {
                Ok(pkt) => {
                    play.session.send(&pkt);
                    sent()
                }
                Err((679, _)) => refuse("invalid_argument", "cannot add yourself as a friend"),
                Err((592, existing)) => refuse("not_ready", format!("{existing} is already on your friend list")),
                Err(_) => refuse("not_ready", "could not add friend"),
            }
        }
        A7Action::FriendRemove { target } => {
            let idx = match target {
                IdOrName::Id(id) => i32::try_from(*id)
                    .ok()
                    .and_then(|id| play.buddies.map.values().position(|b| b.id == id)),
                IdOrName::Name(name) => play
                    .buddies
                    .map
                    .values()
                    .position(|b| b.name.eq_ignore_ascii_case(name)),
            };
            let Some(idx) = idx else {
                return refuse("not_found", "no such friend on your friend list");
            };
            let Some(pkt) = play.buddies.remove(idx as i32) else {
                return refuse("not_found", "no such friend on your friend list");
            };
            play.session.send(&pkt);
            sent()
        }
        A7Action::FriendAccept => {
            let active = a7::friend_request().filter(|r| !r.expired()).or_else(|| {
                (play.mode == 17 && play.buddies.request != u32::MAX).then(|| a7::FriendRequestSnap {
                    id: play.buddies.request,
                    name: play.buddies.request_name.clone(),
                    opened: std::time::Instant::now(),
                    held_by: None,
                })
            });
            let Some(req) = active else {
                return refuse("not_ready", "no incoming friend request to accept");
            };
            if play.buddies.count() >= 32 {
                return refuse("not_ready", "your friend list is full (32)");
            }
            let text = confirm::friend_accept_text(&req.name);
            ask_confirm(
                Act::FriendAccept { request_id: req.id, name: req.name },
                text,
                &["friend_request", "buddy_list"],
            )
        }
        A7Action::FriendRefuse => {
            let id = a7::friend_request()
                .map(|r| r.id)
                .or_else(|| (play.mode == 17 && play.buddies.request != u32::MAX).then_some(play.buddies.request));
            let Some(id) = id else {
                return refuse("not_ready", "no incoming friend request to refuse");
            };
            refuse_held_friend(play, id, "refused");
            sent()
        }
        A7Action::IgnoreAdd { name } => {
            if play.player.name.eq_ignore_ascii_case(name) {
                return refuse("invalid_argument", "cannot ignore yourself");
            }
            if play.ignore.list.iter().any(|e| e.name.eq_ignore_ascii_case(name)) {
                return refuse("not_ready", format!("{name} is already on your ignore list"));
            }
            let pkt = Ignore::add(name);
            play.session.send(&pkt);
            sent()
        }
        A7Action::IgnoreRemove { target } => {
            let pkt = match target {
                IdOrName::Id(id) => u32::try_from(*id)
                    .ok()
                    .filter(|id| play.ignore.list.iter().any(|e| e.id == *id))
                    .map(Ignore::remove_id),
                IdOrName::Name(name) => play.ignore.remove(name),
            };
            let Some(pkt) = pkt else {
                return refuse("not_found", "no such player on your ignore list");
            };
            play.session.send(&pkt);
            sent()
        }
        A7Action::GuildInvite { name } => {
            let me = play.dump.player.id as i32;
            match Cmds::can_invite(&play.guild, me) {
                0 => {}
                3 => return refuse("not_ready", "you are not in a guild"),
                11 => return refuse("not_ready", "the guild roster is full"),
                5 => return refuse("permission_denied", "your guild rank cannot invite players"),
                _ => return refuse("not_ready", "cannot invite to guild"),
            }
            match Cmds::invite(&play.player.name, name, &play.guild) {
                Ok(pkt) => {
                    play.session.send(&pkt);
                    sent()
                }
                Err(_) => refuse("invalid_argument", "cannot invite yourself to the guild"),
            }
        }
        A7Action::GuildPromote { target } => {
            let Some((id, _)) = resolve_guild_member(play, target) else {
                return refuse("not_found", "no such member in the guild roster");
            };
            let me = play.dump.player.id as i32;
            match Cmds::can_promote(&play.guild, me, id) {
                0 => {
                    play.session.send(&Cmds::member_op(4, id));
                    sent()
                }
                3 => refuse("not_ready", "you are not in a guild"),
                _ => refuse("permission_denied", "your guild rank cannot promote that member"),
            }
        }
        A7Action::GuildDemote { target } => {
            let Some((id, _)) = resolve_guild_member(play, target) else {
                return refuse("not_found", "no such member in the guild roster");
            };
            let me = play.dump.player.id as i32;
            match Cmds::can_demote(&play.guild, me, id) {
                0 => {
                    play.session.send(&Cmds::member_op(5, id));
                    sent()
                }
                3 => refuse("not_ready", "you are not in a guild"),
                12 => refuse("not_ready", "that member is already at the lowest guild rank"),
                _ => refuse("permission_denied", "your guild rank cannot demote that member"),
            }
        }
        A7Action::GuildKick { target } => {
            let Some((id, name)) = resolve_guild_member(play, target) else {
                return refuse("not_found", "no such member in the guild roster");
            };
            let me = play.dump.player.id as i32;
            match Cmds::can_kick(&play.guild, me, id) {
                0 => {
                    let text = confirm::guild_kick_text(&name);
                    ask_confirm(Act::GuildKick { member_id: id, name }, text, &["guild", "buddy_list"])
                }
                3 => refuse("not_ready", "you are not in a guild"),
                _ => refuse("permission_denied", "your guild rank cannot kick that member"),
            }
        }
        A7Action::GuildSetMotd { text } => {
            let me = play.dump.player.id as i32;
            match Cmds::can_lead(&play.guild, me) {
                0 => {
                    play.session.send(&Cmds::text(11, text));
                    sent()
                }
                3 => refuse("not_ready", "you are not in a guild"),
                _ => refuse("permission_denied", "only the guild leader can change the message of the day"),
            }
        }
        A7Action::GuildAccept => {
            if play.guild.in_guild {
                return refuse("not_ready", "you are already in a guild");
            }
            let active = a7::guild_invite().filter(|r| !r.expired()).or_else(|| {
                (play.mode == 20 && play.guild_cmds.inviter != u32::MAX).then(|| a7::GuildInviteSnap {
                    inviter_id: play.guild_cmds.inviter,
                    inviter: play.guild_cmds.name.clone(),
                    guild: play.guild_cmds.guild.clone(),
                    opened: std::time::Instant::now(),
                    held_by: None,
                })
            });
            let Some(inv) = active else {
                return refuse("not_ready", "no incoming guild invitation to accept");
            };
            let text = confirm::guild_accept_text(&inv.inviter, &inv.guild);
            ask_confirm(
                Act::GuildAccept { inviter_id: inv.inviter_id, inviter: inv.inviter, guild: inv.guild },
                text,
                &["guild_invite", "guild", "buddy_list"],
            )
        }
        A7Action::GuildRefuse => {
            let id = a7::guild_invite()
                .map(|r| r.inviter_id)
                .or_else(|| (play.mode == 20 && play.guild_cmds.inviter != u32::MAX).then_some(play.guild_cmds.inviter));
            let Some(id) = id else {
                return refuse("not_ready", "no incoming guild invitation to refuse");
            };
            refuse_held_guild(play, id, "refused");
            sent()
        }
        A7Action::WhoRequest => {
            let pkt = play.who.open();
            play.session.send(&pkt);
            a7::set_who(build_who(play), false);
            sent()
        }
        A7Action::LfgSearch { kind, races, classes, min_level, max_level } => {
            if !crate::live::features().live() {
                return refuse("unavailable", "this server does not offer group search");
            }
            if play.group.in_group && !play.group.leader && !play.members_invite() {
                return refuse("not_ready", "only the group's leader can search for players");
            }
            if play.group.in_group && play.group.count() >= 4 {
                return refuse("not_ready", "your group is already full");
            }
            let (def_min, def_max) = lfg_range(play);
            let min = min_level.unwrap_or(def_min.clamp(1, 60) as u8);
            let max = max_level.unwrap_or(def_max.clamp(1, 60) as u8).max(min);
            let q = Query {
                kind: kind.unwrap_or(if play.group.in_group { 0 } else { 1 }),
                races: *races,
                classes: *classes,
                min,
                max,
            };
            let now = play.session.now() as i64;
            let Some(pkt) = play.live.part.search(&q, now) else {
                return refuse("busy", "a group search is already in progress");
            };
            play.session.send(&pkt);
            refresh_lfg(play, false);
            sent()
        }
        A7Action::LfgRegister => {
            if !crate::live::features().live() {
                return refuse("unavailable", "this server does not offer group search");
            }
            if play.group.in_group && !play.group.leader {
                return refuse("not_ready", "only the group's leader can register for group search");
            }
            play.session.send(&Lfg::register());
            sent()
        }
        A7Action::LfgUnregister => {
            if !crate::live::features().live() {
                return refuse("unavailable", "this server does not offer group search");
            }
            if play.group.in_group && !play.group.leader {
                return refuse("not_ready", "only the group's leader can unregister from group search");
            }
            play.session.send(&Lfg::unregister());
            sent()
        }
    }
}

/// Carries out a player-confirmed A7 action from `a1_play::carry_out`.
pub(crate) fn carry_out(play: &mut Play, act: &Act) -> Result<(), String> {
    match act {
        Act::GuildKick { member_id, .. } => {
            if !play.guild.in_guild {
                return Err("you are no longer in a guild".into());
            }
            if !play.guild.members.contains_key(member_id) {
                return Err("that player is no longer in the guild roster".into());
            }
            let me = play.dump.player.id as i32;
            if Cmds::can_kick(&play.guild, me, *member_id) != 0 {
                return Err("your guild rank can no longer kick that member".into());
            }
            play.session.send(&Cmds::member_op(6, *member_id));
            Ok(())
        }
        Act::FriendAccept { request_id, .. } => {
            let live = a7::friend_request().filter(|r| r.id == *request_id && !r.expired());
            if live.is_none() && !(play.mode == 17 && play.buddies.request == *request_id) {
                return Err("that friend request is no longer pending".into());
            }
            if play.buddies.count() >= 32 {
                return Err("your friend list is full".into());
            }
            play.buddies.request = *request_id;
            let req = play.buddies.accept();
            play.session.send(&req);
            play.buddies.request = u32::MAX;
            play.buddies.request_name.clear();
            play.buddy_wnd.hide();
            if play.mode == 17 {
                play.mode = 0;
            }
            a7::clear_friend_request();
            window_sessions::sync_held_request(TakeoverKind::FriendRequest, None, "accepted");
            Ok(())
        }
        Act::GuildAccept { inviter_id, .. } => {
            if play.guild.in_guild {
                return Err("you are already in a guild".into());
            }
            let live = a7::guild_invite().filter(|r| r.inviter_id == *inviter_id && !r.expired());
            if live.is_none() && !(play.mode == 20 && play.guild_cmds.inviter == *inviter_id) {
                return Err("that guild invitation is no longer pending".into());
            }
            play.guild_cmds.inviter = *inviter_id;
            let m = play.guild_cmds.join();
            play.session.send(&m);
            play.guild_wnd.reset();
            play.guild_open = None;
            if play.mode == 20 {
                play.mode = 0;
            }
            a7::clear_guild_invite();
            window_sessions::sync_held_request(TakeoverKind::GuildInvite, None, "accepted");
            Ok(())
        }
        _ => Err("not an A7 confirmation".into()),
    }
}
