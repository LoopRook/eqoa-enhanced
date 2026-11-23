// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A7: community reads, events, actions and request sessions (friends, ignore, guild, who, group search).
//! Plain data and Lua bindings only: `fork_ext/a7_play.rs` fills this from `Play` and carries out actions through the
//! game's own public builders (`social.rs`, `guild.rs`, `wholist.rs`, `groupsearch.rs`), so nothing here knows `Play`.
//!
//! Incoming friend requests (`friend_request`) and guild invitations (`guild_invite`) time out after 45 seconds in the
//! game only while their native box is drawn. When a mod holds one hidden via `eqoa.windows.takeover(..., {scope="all"})`,
//! the fork tracks the 45-second deadline here and `a7_play::tick` refuses it with the game's own refusal bytes if it
//! expires or cannot be handed back, so a request is never left hanging. Accepting a friend request or guild invitation,
//! and kicking a guild member, always go through A1's client-drawn "Are you sure?" confirm (`confirm.rs`), starting on
//! No, so only the player's own click or key press can accept or kick.
use super::{
    action_broker::{ActionKind, enqueue_lua, enqueue_lua_with},
    host::Host,
    native::{bind, describe, error, record},
    native_core::CORE,
    window_takeover::{self, TakeoverKind},
};
use mlua::{Lua, Result, Value};
use serde_json::{Value as Json, json};
use std::cell::RefCell;
use std::time::{Duration, Instant};

const ROUND: u8 = 8;

/// The game's own request timeout (`social.rs:555`, `guild.rs:654`): 45 seconds.
pub(crate) const REQUEST_TIMEOUT: Duration = Duration::from_secs(45);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum IdOrName {
    Id(i64),
    Name(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum A7Action {
    FriendAdd { name: String },
    FriendRemove { target: IdOrName },
    FriendAccept,
    FriendRefuse,
    IgnoreAdd { name: String },
    IgnoreRemove { target: IdOrName },
    GuildInvite { name: String },
    GuildPromote { target: IdOrName },
    GuildDemote { target: IdOrName },
    GuildKick { target: IdOrName },
    GuildSetMotd { text: String },
    GuildAccept,
    GuildRefuse,
    WhoRequest,
    LfgSearch {
        /// `Some(0)` players for leader's group, `Some(1)` groups for solo player, `None` inferred from group state.
        kind: Option<u8>,
        races: i32,
        classes: i32,
        min_level: Option<u8>,
        max_level: Option<u8>,
    },
    LfgRegister,
    LfgUnregister,
}

impl A7Action {
    pub(crate) const fn name(&self) -> &'static str {
        match self {
            Self::FriendAdd { .. } => "friend_add",
            Self::FriendRemove { .. } => "friend_remove",
            Self::FriendAccept => "friend_accept",
            Self::FriendRefuse => "friend_refuse",
            Self::IgnoreAdd { .. } => "ignore_add",
            Self::IgnoreRemove { .. } => "ignore_remove",
            Self::GuildInvite { .. } => "guild_invite",
            Self::GuildPromote { .. } => "guild_promote",
            Self::GuildDemote { .. } => "guild_demote",
            Self::GuildKick { .. } => "guild_kick",
            Self::GuildSetMotd { .. } => "guild_set_motd",
            Self::GuildAccept => "guild_accept",
            Self::GuildRefuse => "guild_refuse",
            Self::WhoRequest => "who_request",
            Self::LfgSearch { .. } => "lfg_search",
            Self::LfgRegister => "lfg_register",
            Self::LfgUnregister => "lfg_unregister",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FriendEntry {
    pub id: i32,
    pub name: String,
    pub online: bool,
    pub class_id: i32,
    pub class_name: Option<String>,
    pub race_id: i32,
    pub race_name: Option<String>,
    pub level: i32,
    pub world: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct IgnoredEntry {
    pub id: u32,
    pub name: String,
}

#[derive(Clone, Debug)]
pub(crate) struct FriendRequestSnap {
    pub id: u32,
    pub name: String,
    pub opened: Instant,
    pub held_by: Option<(u64, String)>,
}

impl FriendRequestSnap {
    pub(crate) fn seconds_remaining(&self) -> f64 {
        let elapsed = self.opened.elapsed().as_secs_f64();
        (REQUEST_TIMEOUT.as_secs_f64() - elapsed).max(0.0)
    }

    pub(crate) fn expired(&self) -> bool {
        self.opened.elapsed() >= REQUEST_TIMEOUT
    }

    fn json(&self) -> Json {
        json!({
            "id": self.id,
            "name": self.name,
            "seconds_remaining": self.seconds_remaining(),
            "duration": REQUEST_TIMEOUT.as_secs_f64(),
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GuildMemberSnap {
    pub id: i32,
    pub name: String,
    pub class_id: i32,
    pub class_name: Option<String>,
    pub race_id: i32,
    pub race_name: Option<String>,
    pub level: i32,
    pub rank: i32,
    pub rank_name: String,
    pub online: bool,
    pub can_promote: bool,
    pub can_demote: bool,
    pub can_kick: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct GuildSnap {
    pub in_guild: bool,
    pub name: String,
    pub motd: String,
    pub show_chat: bool,
    pub self_rank: Option<i32>,
    pub self_rank_name: Option<String>,
    pub can_invite: bool,
    pub can_set_motd: bool,
    pub members: Vec<GuildMemberSnap>,
}

#[derive(Clone, Debug)]
pub(crate) struct GuildInviteSnap {
    pub inviter_id: u32,
    pub inviter: String,
    pub guild: String,
    pub opened: Instant,
    pub held_by: Option<(u64, String)>,
}

impl GuildInviteSnap {
    pub(crate) fn seconds_remaining(&self) -> f64 {
        let elapsed = self.opened.elapsed().as_secs_f64();
        (REQUEST_TIMEOUT.as_secs_f64() - elapsed).max(0.0)
    }

    pub(crate) fn expired(&self) -> bool {
        self.opened.elapsed() >= REQUEST_TIMEOUT
    }

    fn json(&self) -> Json {
        json!({
            "inviter_id": self.inviter_id,
            "inviter": self.inviter,
            "guild": self.guild,
            "seconds_remaining": self.seconds_remaining(),
            "duration": REQUEST_TIMEOUT.as_secs_f64(),
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct WhoEntrySnap {
    pub id: u32,
    pub name: String,
    pub race_id: i32,
    pub race_name: Option<String>,
    pub class_id: i32,
    pub class_name: Option<String>,
    pub level: i32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct WhoSnap {
    pub pending: bool,
    pub received: u64,
    pub entries: Vec<WhoEntrySnap>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LfgEntrySnap {
    pub id: u32,
    pub name: String,
    pub race_id: i32,
    pub race_name: Option<String>,
    pub class_id: i32,
    pub class_name: Option<String>,
    pub level: i32,
    pub world_id: i32,
    pub position: [f32; 3],
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct LfgSnap {
    pub available: bool,
    pub busy: bool,
    pub registered: Option<bool>,
    pub invites_open: Option<bool>,
    pub members_invite: Option<bool>,
    pub default_min_level: i32,
    pub default_max_level: i32,
    pub entries: Vec<LfgEntrySnap>,
}

#[derive(Default)]
struct State {
    generation: u64,
    friends: Vec<FriendEntry>,
    friends_hash: Option<u64>,
    ignored: Vec<IgnoredEntry>,
    ignored_hash: Option<u64>,
    guild: GuildSnap,
    guild_hash: Option<u64>,
    who: WhoSnap,
    lfg: LfgSnap,
    lfg_meta_hash: Option<u64>,
    friend_request: Option<FriendRequestSnap>,
    guild_invite: Option<GuildInviteSnap>,
    /// Set by `end_event` when an incoming 0x076F or 0x057A should be hidden before `draw_hud`.
    pending_hide_friend: bool,
    pending_hide_guild: bool,
    /// If a second 0x076F or 0x057A arrived while a hidden request was already held, refuse the new one as busy
    /// and restore the first held request on `Play`.
    refuse_second_friend: Option<u32>,
    refuse_second_guild: Option<u32>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

pub(crate) fn forget() {
    STATE.with(|s| *s.borrow_mut() = State::default());
}

thread_local! {
    /// Held requests whose world ended (zoning, a teleport, a new world entry): still to be refused to the server
    /// by the next step, which has the `Play` to send with. Kept apart from `STATE` so a generation change or
    /// `forget` cannot wipe them before that.
    static RESET_REFUSE: RefCell<(Vec<u32>, Vec<u32>)> = const { RefCell::new((Vec::new(), Vec::new())) };
}

/// The world is ending: remember the ids of the held requests so the next step refuses them (`take_reset_refusals`).
pub(crate) fn note_reset_refusals() {
    let (friend, guild) = STATE.with(|s| {
        let s = s.borrow();
        (
            s.friend_request.as_ref().filter(|r| r.held_by.is_some()).map(|r| r.id),
            s.guild_invite.as_ref().filter(|r| r.held_by.is_some()).map(|r| r.inviter_id),
        )
    });
    RESET_REFUSE.with(|r| {
        let mut r = r.borrow_mut();
        if let Some(id) = friend {
            r.0.push(id);
        }
        if let Some(id) = guild {
            r.1.push(id);
        }
    });
}

pub(crate) fn take_reset_refusals() -> (Vec<u32>, Vec<u32>) {
    RESET_REFUSE.with(|r| std::mem::take(&mut *r.borrow_mut()))
}

pub(crate) fn enter_generation() -> bool {
    let generation = CORE.with(|c| c.borrow().generation);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.generation != generation {
            *s = State { generation, ..State::default() };
            true
        } else {
            false
        }
    })
}

fn emit(name: &str, data: Json) {
    if super::native_events::interested(name) {
        CORE.with(|c| c.borrow_mut().emit(name, data));
    }
}

fn friends_json(list: &[FriendEntry]) -> Json {
    Json::Array(
        list.iter()
            .map(|f| {
                json!({
                    "id": f.id,
                    "name": f.name,
                    "online": f.online,
                    "class_id": f.class_id,
                    "class_name": f.class_name,
                    "race_id": f.race_id,
                    "race_name": f.race_name,
                    "level": f.level,
                    "world": f.world,
                })
            })
            .collect(),
    )
}

fn ignored_json(list: &[IgnoredEntry]) -> Json {
    Json::Array(
        list.iter()
            .map(|i| json!({"id": i.id, "name": i.name}))
            .collect(),
    )
}

fn guild_json(g: &GuildSnap) -> Json {
    json!({
        "in_guild": g.in_guild,
        "name": g.name,
        "motd": g.motd,
        "show_chat": g.show_chat,
        "self_rank": g.self_rank,
        "self_rank_name": g.self_rank_name,
        "can_invite": g.can_invite,
        "can_set_motd": g.can_set_motd,
        "members": g.members.iter().map(|m| json!({
            "id": m.id,
            "name": m.name,
            "class_id": m.class_id,
            "class_name": m.class_name,
            "race_id": m.race_id,
            "race_name": m.race_name,
            "level": m.level,
            "rank": m.rank,
            "rank_name": m.rank_name,
            "online": m.online,
            "can_promote": m.can_promote,
            "can_demote": m.can_demote,
            "can_kick": m.can_kick,
        })).collect::<Vec<_>>(),
    })
}

fn who_entries_json(entries: &[WhoEntrySnap]) -> Json {
    Json::Array(
        entries
            .iter()
            .map(|e| {
                json!({
                    "id": e.id,
                    "name": e.name,
                    "race_id": e.race_id,
                    "race_name": e.race_name,
                    "class_id": e.class_id,
                    "class_name": e.class_name,
                    "level": e.level,
                })
            })
            .collect(),
    )
}

fn who_json(w: &WhoSnap) -> Json {
    json!({
        "pending": w.pending,
        "received": w.received,
        "entries": who_entries_json(&w.entries),
    })
}

fn lfg_entries_json(entries: &[LfgEntrySnap]) -> Json {
    Json::Array(
        entries
            .iter()
            .map(|e| {
                json!({
                    "id": e.id,
                    "name": e.name,
                    "race_id": e.race_id,
                    "race_name": e.race_name,
                    "class_id": e.class_id,
                    "class_name": e.class_name,
                    "level": e.level,
                    "world_id": e.world_id,
                    "position": {"x": e.position[0], "y": e.position[1], "z": e.position[2]},
                })
            })
            .collect(),
    )
}

fn lfg_json(l: &LfgSnap) -> Json {
    json!({
        "available": l.available,
        "busy": l.busy,
        "registered": l.registered,
        "invites_open": l.invites_open,
        "members_invite": l.members_invite,
        "default_min_level": l.default_min_level,
        "default_max_level": l.default_max_level,
        "entries": lfg_entries_json(&l.entries),
    })
}

pub(crate) fn sync_friends(hash: u64, build: impl FnOnce() -> Vec<FriendEntry>) {
    let change = STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.friends_hash == Some(hash) {
            return None;
        }
        let had_prev = s.friends_hash.is_some();
        s.friends_hash = Some(hash);
        let new = build();
        if s.friends == new {
            return None;
        }
        let before = friends_json(&s.friends);
        s.friends = new;
        had_prev.then(|| (before, friends_json(&s.friends)))
    });
    if let Some((before, after)) = change {
        emit("friends.changed", json!({"before": before, "after": after, "changed": ["records"]}));
    }
}

pub(crate) fn sync_ignored(hash: u64, build: impl FnOnce() -> Vec<IgnoredEntry>) {
    let change = STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.ignored_hash == Some(hash) {
            return None;
        }
        let had_prev = s.ignored_hash.is_some();
        s.ignored_hash = Some(hash);
        let new = build();
        if s.ignored == new {
            return None;
        }
        let before = ignored_json(&s.ignored);
        s.ignored = new;
        had_prev.then(|| (before, ignored_json(&s.ignored)))
    });
    if let Some((before, after)) = change {
        emit("ignored.changed", json!({"before": before, "after": after, "changed": ["records"]}));
    }
}

pub(crate) fn sync_guild(hash: u64, build: impl FnOnce() -> GuildSnap) {
    let change = STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.guild_hash == Some(hash) {
            return None;
        }
        let had_prev = s.guild_hash.is_some();
        s.guild_hash = Some(hash);
        let new = build();
        if s.guild == new {
            return None;
        }
        let mut changed = Vec::new();
        if s.guild.in_guild != new.in_guild {
            changed.push("in_guild");
        }
        if s.guild.name != new.name {
            changed.push("name");
        }
        if s.guild.motd != new.motd {
            changed.push("motd");
        }
        if s.guild.show_chat != new.show_chat {
            changed.push("show_chat");
        }
        if s.guild.members != new.members {
            changed.push("members");
        }
        if changed.is_empty() {
            changed.push("records");
        }
        let before = guild_json(&s.guild);
        s.guild = new;
        had_prev.then(|| (before, guild_json(&s.guild), changed))
    });
    if let Some((before, after, changed)) = change {
        emit("guild.changed", json!({"before": before, "after": after, "changed": changed}));
    }
}

pub(crate) fn set_who(snap: WhoSnap, fire_result: bool) {
    let payload = STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.who = snap;
        fire_result.then(|| {
            json!({
                "entries": who_entries_json(&s.who.entries),
                "count": s.who.entries.len(),
                "received": s.who.received,
            })
        })
    });
    if let Some(data) = payload {
        emit("who.result", data);
    }
}

pub(crate) fn sync_lfg(meta_hash: u64, build: impl FnOnce() -> LfgSnap, fire_result: bool) {
    let (changed_ev, result_ev) = STATE.with(|s| {
        let mut s = s.borrow_mut();
        if !fire_result && s.lfg_meta_hash == Some(meta_hash) {
            return (None, None);
        }
        let had_prev = s.lfg_meta_hash.is_some();
        s.lfg_meta_hash = Some(meta_hash);
        let new = build();
        let meta_changed = (
            s.lfg.available,
            s.lfg.busy,
            s.lfg.registered,
            s.lfg.invites_open,
            s.lfg.members_invite,
        ) != (
            new.available,
            new.busy,
            new.registered,
            new.invites_open,
            new.members_invite,
        );
        let before = (had_prev && meta_changed).then(|| lfg_json(&s.lfg));
        s.lfg = new;
        let changed_ev = before.map(|b| (b, lfg_json(&s.lfg)));
        let result_ev = fire_result.then(|| {
            json!({
                "entries": lfg_entries_json(&s.lfg.entries),
                "count": s.lfg.entries.len(),
            })
        });
        (changed_ev, result_ev)
    });
    if let Some((before, after)) = changed_ev {
        emit("lfg.changed", json!({"before": before, "after": after, "changed": ["state"]}));
    }
    if let Some(data) = result_ev {
        emit("lfg.result", data);
    }
}

// ---- incoming friend_request / guild_invite tracking & 45s timeout ----

pub(crate) fn on_incoming_friend_request(id: u32, name: String) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        // If we are already holding a hidden friend request, keep it and refuse the new one as busy.
        if s.friend_request.as_ref().is_some_and(|r| r.held_by.is_some()) {
            s.refuse_second_friend = Some(id);
            s.pending_hide_friend = true;
            return;
        }
        let holder = (!window_takeover::use_game_windows())
            .then(|| {
                window_takeover::holder_vm(TakeoverKind::FriendRequest)
                    .zip(window_takeover::holder_of(TakeoverKind::FriendRequest))
            })
            .flatten();
        let held = holder.is_some();
        s.friend_request = Some(FriendRequestSnap {
            id,
            name,
            opened: Instant::now(),
            held_by: holder,
        });
        if held {
            s.pending_hide_friend = true;
        }
    });
}

pub(crate) fn on_incoming_guild_invite(inviter_id: u32, inviter: String, guild: String) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.guild_invite.as_ref().is_some_and(|r| r.held_by.is_some()) {
            s.refuse_second_guild = Some(inviter_id);
            s.pending_hide_guild = true;
            return;
        }
        let holder = (!window_takeover::use_game_windows())
            .then(|| {
                window_takeover::holder_vm(TakeoverKind::GuildInvite)
                    .zip(window_takeover::holder_of(TakeoverKind::GuildInvite))
            })
            .flatten();
        let held = holder.is_some();
        s.guild_invite = Some(GuildInviteSnap {
            inviter_id,
            inviter,
            guild,
            opened: Instant::now(),
            held_by: holder,
        });
        if held {
            s.pending_hide_guild = true;
        }
    });
}

pub(crate) fn take_pending_hide() -> (bool, bool, Option<u32>, Option<u32>) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        (
            std::mem::take(&mut s.pending_hide_friend),
            std::mem::take(&mut s.pending_hide_guild),
            s.refuse_second_friend.take(),
            s.refuse_second_guild.take(),
        )
    })
}

pub(crate) fn friend_request() -> Option<FriendRequestSnap> {
    STATE.with(|s| s.borrow().friend_request.clone())
}

pub(crate) fn guild_invite() -> Option<GuildInviteSnap> {
    STATE.with(|s| s.borrow().guild_invite.clone())
}

pub(crate) fn held_friend_holder() -> Option<(u64, String)> {
    STATE.with(|s| s.borrow().friend_request.as_ref().and_then(|r| r.held_by.clone()))
}

pub(crate) fn held_guild_holder() -> Option<(u64, String)> {
    STATE.with(|s| s.borrow().guild_invite.as_ref().and_then(|r| r.held_by.clone()))
}

pub(crate) fn clear_friend_request() {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.friend_request = None;
        s.pending_hide_friend = false;
        s.refuse_second_friend = None;
    });
}

pub(crate) fn clear_guild_invite() {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.guild_invite = None;
        s.pending_hide_guild = false;
        s.refuse_second_guild = None;
    });
}

pub(crate) fn hand_back_friend_request() -> Option<FriendRequestSnap> {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let req = s.friend_request.as_mut()?;
        req.held_by = None;
        Some(req.clone())
    })
}

pub(crate) fn hand_back_guild_invite() -> Option<GuildInviteSnap> {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let inv = s.guild_invite.as_mut()?;
        inv.held_by = None;
        Some(inv.clone())
    })
}

/// Keeps native (unheld) mode 17 / mode 20 prompts readable via `social.friend_request()` / `guild.invite()`.
pub(crate) fn sync_native_requests(native_friend: Option<(u32, String)>, native_guild: Option<(u32, String, String)>) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.friend_request.as_ref().is_none_or(|r| r.held_by.is_none()) {
            match (s.friend_request.as_ref(), native_friend) {
                (Some(cur), Some((id, _))) if cur.id == id => {}
                (_, Some((id, name))) => {
                    s.friend_request = Some(FriendRequestSnap {
                        id,
                        name,
                        opened: Instant::now(),
                        held_by: None,
                    });
                }
                (_, None) => s.friend_request = None,
            }
        }
        if s.guild_invite.as_ref().is_none_or(|r| r.held_by.is_none()) {
            match (s.guild_invite.as_ref(), native_guild) {
                (Some(cur), Some((id, _, _))) if cur.inviter_id == id => {}
                (_, Some((inviter_id, inviter, guild))) => {
                    s.guild_invite = Some(GuildInviteSnap {
                        inviter_id,
                        inviter,
                        guild,
                        opened: Instant::now(),
                        held_by: None,
                    });
                }
                (_, None) => s.guild_invite = None,
            }
        }
    });
}

#[cfg(test)]
pub(crate) fn age_friend_request_for_test(by: Duration) {
    STATE.with(|s| {
        if let Some(r) = s.borrow_mut().friend_request.as_mut() {
            r.opened -= by;
        }
    });
}

#[cfg(test)]
pub(crate) fn age_guild_invite_for_test(by: Duration) {
    STATE.with(|s| {
        if let Some(r) = s.borrow_mut().guild_invite.as_mut() {
            r.opened -= by;
        }
    });
}

#[cfg(test)]
pub(crate) fn seed_for_test(
    friends: Vec<FriendEntry>,
    ignored: Vec<IgnoredEntry>,
    guild: GuildSnap,
    who: WhoSnap,
    lfg: LfgSnap,
) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.friends = friends;
        s.friends_hash = Some(1);
        s.ignored = ignored;
        s.ignored_hash = Some(1);
        s.guild = guild;
        s.guild_hash = Some(1);
        s.who = who;
        s.lfg = lfg;
        s.lfg_meta_hash = Some(1);
    });
}

// ---- Lua parsing helpers ----

fn in_world() -> bool {
    CORE.with(|c| c.borrow().view.in_world)
}

fn not_in_world(lua: &Lua, symbol: &str) -> Result<mlua::MultiValue> {
    error(lua, symbol, "not_in_world", "enter a loaded world first")
}

fn parse_name(val: &Value, max_bytes: usize, what: &str) -> std::result::Result<String, (&'static str, String)> {
    let Some(s) = val.as_string().and_then(|s| s.to_str().ok().map(|s| s.trim().to_string())) else {
        return Err(("invalid_argument", format!("{what} must be a non-empty string")));
    };
    if s.is_empty() || s.len() > max_bytes || s.chars().any(char::is_control) {
        return Err((
            "invalid_argument",
            format!("{what} must be 1..={max_bytes} printable bytes with no control characters"),
        ));
    }
    Ok(s)
}

fn parse_id_or_name(val: &Value, what: &str) -> std::result::Result<IdOrName, (&'static str, String)> {
    match val {
        Value::Integer(n) => Ok(IdOrName::Id(*n)),
        Value::Number(n) if n.is_finite() && n.fract() == 0.0 && *n >= i32::MIN as f64 && *n <= u32::MAX as f64 => {
            Ok(IdOrName::Id(*n as i64))
        }
        Value::String(_) => Ok(IdOrName::Name(parse_name(val, 32, what)?)),
        _ => Err(("invalid_argument", format!("{what} must be an integer id or player name"))),
    }
}

fn parse_opt_int(v: Value, what: &str) -> std::result::Result<Option<i64>, (&'static str, String)> {
    match v {
        Value::Nil => Ok(None),
        Value::Integer(n) => Ok(Some(n)),
        Value::Number(n) if n.is_finite() && n.fract() == 0.0 => Ok(Some(n as i64)),
        _ => Err(("invalid_argument", format!("{what} must be a whole number"))),
    }
}

// ---- Lua installation ----

pub(crate) fn install(host: &Host) -> Result<()> {
    let lua = &host.lua;

    // Record shapes
    for (path, fields) in [
        (
            "eqoa.social.Friend",
            "id name online class_id class_name race_id race_name level world",
        ),
        ("eqoa.social.Ignored", "id name"),
        (
            "eqoa.social.FriendRequest",
            "id name seconds_remaining duration",
        ),
        (
            "eqoa.guild.Guild",
            "in_guild name motd show_chat self_rank self_rank_name can_invite can_set_motd members",
        ),
        (
            "eqoa.guild.GuildMember",
            "id name class_id class_name race_id race_name level rank rank_name online can_promote can_demote can_kick",
        ),
        (
            "eqoa.guild.GuildInvite",
            "inviter_id inviter guild seconds_remaining duration",
        ),
        ("eqoa.who.WhoState", "pending received entries"),
        (
            "eqoa.who.WhoEntry",
            "id name race_id race_name class_id class_name level",
        ),
        (
            "eqoa.lfg.LfgState",
            "available busy registered invites_open members_invite default_min_level default_max_level entries",
        ),
        (
            "eqoa.lfg.LfgEntry",
            "id name race_id race_name class_id class_name level world_id position",
        ),
    ] {
        describe(lua, path, "supported", Some(ROUND), "");
        for field in fields.split_whitespace() {
            describe(lua, &format!("{path}.{field}"), "supported", Some(ROUND), "");
        }
    }

    // ---- Reads: eqoa.social.* ----
    bind(
        lua,
        "social",
        "friends",
        lua.create_function(|lua, ()| {
            if !in_world() {
                return not_in_world(lua, "eqoa.social.friends");
            }
            let value = STATE.with(|s| friends_json(&s.borrow().friends));
            lua.pack_multi((record(lua, "eqoa.social.Friend", &value)?, Value::Nil))
        })?,
    )?;
    bind(
        lua,
        "social",
        "ignored",
        lua.create_function(|lua, ()| {
            if !in_world() {
                return not_in_world(lua, "eqoa.social.ignored");
            }
            let value = STATE.with(|s| ignored_json(&s.borrow().ignored));
            lua.pack_multi((record(lua, "eqoa.social.Ignored", &value)?, Value::Nil))
        })?,
    )?;
    bind(
        lua,
        "social",
        "friend_request",
        lua.create_function(|lua, ()| {
            if !in_world() {
                return not_in_world(lua, "eqoa.social.friend_request");
            }
            let value = STATE.with(|s| s.borrow().friend_request.as_ref().map(FriendRequestSnap::json));
            match value {
                Some(v) => lua.pack_multi((record(lua, "eqoa.social.FriendRequest", &v)?, Value::Nil)),
                None => lua.pack_multi((Value::Nil, Value::Nil)),
            }
        })?,
    )?;

    // ---- Reads: eqoa.guild.* ----
    bind(
        lua,
        "guild",
        "get",
        lua.create_function(|lua, ()| {
            if !in_world() {
                return not_in_world(lua, "eqoa.guild.get");
            }
            let value = STATE.with(|s| guild_json(&s.borrow().guild));
            lua.pack_multi((record(lua, "eqoa.guild.Guild", &value)?, Value::Nil))
        })?,
    )?;
    bind(
        lua,
        "guild",
        "invite",
        lua.create_function(|lua, ()| {
            if !in_world() {
                return not_in_world(lua, "eqoa.guild.invite");
            }
            let value = STATE.with(|s| s.borrow().guild_invite.as_ref().map(GuildInviteSnap::json));
            match value {
                Some(v) => lua.pack_multi((record(lua, "eqoa.guild.GuildInvite", &v)?, Value::Nil)),
                None => lua.pack_multi((Value::Nil, Value::Nil)),
            }
        })?,
    )?;

    // ---- Reads & acting calls: eqoa.who.* ----
    bind(
        lua,
        "who",
        "get",
        lua.create_function(|lua, ()| {
            if !in_world() {
                return not_in_world(lua, "eqoa.who.get");
            }
            let value = STATE.with(|s| who_json(&s.borrow().who));
            lua.pack_multi((record(lua, "eqoa.who.WhoState", &value)?, Value::Nil))
        })?,
    )?;
    bind(
        lua,
        "who",
        "request",
        lua.create_function(|lua, ()| {
            enqueue_lua(lua, "eqoa.who.request", ActionKind::A7(A7Action::WhoRequest))
        })?,
    )?;

    // ---- Reads & acting calls: eqoa.lfg.* ----
    bind(
        lua,
        "lfg",
        "get",
        lua.create_function(|lua, ()| {
            if !in_world() {
                return not_in_world(lua, "eqoa.lfg.get");
            }
            let value = STATE.with(|s| lfg_json(&s.borrow().lfg));
            lua.pack_multi((record(lua, "eqoa.lfg.LfgState", &value)?, Value::Nil))
        })?,
    )?;
    bind(
        lua,
        "lfg",
        "search",
        lua.create_function(|lua, opts: Option<Value>| {
            enqueue_lua_with(lua, "eqoa.lfg.search", || {
                let mut kind = None;
                let mut races: i32 = -1;
                let mut classes: i32 = -1;
                let mut min_level: Option<u8> = None;
                let mut max_level: Option<u8> = None;
                match opts {
                    None | Some(Value::Nil) => {}
                    Some(Value::Table(t)) => {
                        match t.get::<Value>("kind").map_err(|_| ("invalid_argument", "invalid kind".into()))? {
                            Value::Nil => {}
                            Value::String(s) => match s.to_str().ok().as_deref() {
                                Some("players") => kind = Some(0),
                                Some("groups") => kind = Some(1),
                                _ => {
                                    return Err((
                                        "invalid_argument",
                                        "kind must be 'players' or 'groups'".into(),
                                    ));
                                }
                            },
                            _ => return Err(("invalid_argument", "kind must be 'players' or 'groups'".into())),
                        }
                        if let Some(v) = parse_opt_int(
                            t.get::<Value>("races").map_err(|_| ("invalid_argument", "invalid races".into()))?,
                            "races",
                        )? {
                            races = i32::try_from(v)
                                .map_err(|_| ("invalid_argument", "races mask out of i32 range".into()))?;
                        }
                        if let Some(v) = parse_opt_int(
                            t.get::<Value>("classes").map_err(|_| ("invalid_argument", "invalid classes".into()))?,
                            "classes",
                        )? {
                            classes = i32::try_from(v)
                                .map_err(|_| ("invalid_argument", "classes mask out of i32 range".into()))?;
                        }
                        if let Some(v) = parse_opt_int(
                            t.get::<Value>("min_level").map_err(|_| ("invalid_argument", "invalid min_level".into()))?,
                            "min_level",
                        )? {
                            if !(1..=60).contains(&v) {
                                return Err(("out_of_range", "min_level must be 1..60".into()));
                            }
                            min_level = Some(v as u8);
                        }
                        if let Some(v) = parse_opt_int(
                            t.get::<Value>("max_level").map_err(|_| ("invalid_argument", "invalid max_level".into()))?,
                            "max_level",
                        )? {
                            if !(1..=60).contains(&v) {
                                return Err(("out_of_range", "max_level must be 1..60".into()));
                            }
                            max_level = Some(v as u8);
                        }
                        if let (Some(lo), Some(hi)) = (min_level, max_level)
                            && lo > hi
                        {
                            return Err(("invalid_argument", "min_level cannot exceed max_level".into()));
                        }
                    }
                    Some(_) => return Err(("invalid_argument", "options must be a table or nil".into())),
                }
                Ok(ActionKind::A7(A7Action::LfgSearch {
                    kind,
                    races,
                    classes,
                    min_level,
                    max_level,
                }))
            })
        })?,
    )?;
    bind(
        lua,
        "lfg",
        "register",
        lua.create_function(|lua, ()| {
            enqueue_lua(lua, "eqoa.lfg.register", ActionKind::A7(A7Action::LfgRegister))
        })?,
    )?;
    bind(
        lua,
        "lfg",
        "unregister",
        lua.create_function(|lua, ()| {
            enqueue_lua(lua, "eqoa.lfg.unregister", ActionKind::A7(A7Action::LfgUnregister))
        })?,
    )?;

    // ---- Actions: eqoa.actions.* ----
    bind(
        lua,
        "actions",
        "friend_add",
        lua.create_function(|lua, name: Value| {
            enqueue_lua_with(lua, "eqoa.actions.friend_add", || {
                Ok(ActionKind::A7(A7Action::FriendAdd {
                    name: parse_name(&name, 24, "friend name")?,
                }))
            })
        })?,
    )?;
    bind(
        lua,
        "actions",
        "friend_remove",
        lua.create_function(|lua, who: Value| {
            enqueue_lua_with(lua, "eqoa.actions.friend_remove", || {
                Ok(ActionKind::A7(A7Action::FriendRemove {
                    target: parse_id_or_name(&who, "friend")?,
                }))
            })
        })?,
    )?;
    bind(
        lua,
        "actions",
        "friend_accept",
        lua.create_function(|lua, ()| {
            enqueue_lua(lua, "eqoa.actions.friend_accept", ActionKind::A7(A7Action::FriendAccept))
        })?,
    )?;
    bind(
        lua,
        "actions",
        "friend_refuse",
        lua.create_function(|lua, ()| {
            enqueue_lua(lua, "eqoa.actions.friend_refuse", ActionKind::A7(A7Action::FriendRefuse))
        })?,
    )?;
    bind(
        lua,
        "actions",
        "ignore_add",
        lua.create_function(|lua, name: Value| {
            enqueue_lua_with(lua, "eqoa.actions.ignore_add", || {
                Ok(ActionKind::A7(A7Action::IgnoreAdd {
                    name: parse_name(&name, 32, "player name")?,
                }))
            })
        })?,
    )?;
    bind(
        lua,
        "actions",
        "ignore_remove",
        lua.create_function(|lua, who: Value| {
            enqueue_lua_with(lua, "eqoa.actions.ignore_remove", || {
                Ok(ActionKind::A7(A7Action::IgnoreRemove {
                    target: parse_id_or_name(&who, "ignored player")?,
                }))
            })
        })?,
    )?;
    bind(
        lua,
        "actions",
        "guild_invite",
        lua.create_function(|lua, name: Value| {
            enqueue_lua_with(lua, "eqoa.actions.guild_invite", || {
                Ok(ActionKind::A7(A7Action::GuildInvite {
                    name: parse_name(&name, 24, "player name")?,
                }))
            })
        })?,
    )?;
    bind(
        lua,
        "actions",
        "guild_promote",
        lua.create_function(|lua, member: Value| {
            enqueue_lua_with(lua, "eqoa.actions.guild_promote", || {
                Ok(ActionKind::A7(A7Action::GuildPromote {
                    target: parse_id_or_name(&member, "guild member")?,
                }))
            })
        })?,
    )?;
    bind(
        lua,
        "actions",
        "guild_demote",
        lua.create_function(|lua, member: Value| {
            enqueue_lua_with(lua, "eqoa.actions.guild_demote", || {
                Ok(ActionKind::A7(A7Action::GuildDemote {
                    target: parse_id_or_name(&member, "guild member")?,
                }))
            })
        })?,
    )?;
    bind(
        lua,
        "actions",
        "guild_kick",
        lua.create_function(|lua, member: Value| {
            enqueue_lua_with(lua, "eqoa.actions.guild_kick", || {
                Ok(ActionKind::A7(A7Action::GuildKick {
                    target: parse_id_or_name(&member, "guild member")?,
                }))
            })
        })?,
    )?;
    bind(
        lua,
        "actions",
        "guild_set_motd",
        lua.create_function(|lua, text: Value| {
            enqueue_lua_with(lua, "eqoa.actions.guild_set_motd", || {
                let Some(s) = text.as_string().and_then(|s| s.to_str().ok().map(|s| s.to_string())) else {
                    return Err(("invalid_argument", "message of the day must be a string".into()));
                };
                if s.encode_utf16().count() > 64 || s.chars().any(char::is_control) {
                    return Err((
                        "invalid_argument",
                        "message of the day must be at most 64 UTF-16 characters with no line breaks or control characters".into(),
                    ));
                }
                Ok(ActionKind::A7(A7Action::GuildSetMotd { text: s }))
            })
        })?,
    )?;
    bind(
        lua,
        "actions",
        "guild_accept",
        lua.create_function(|lua, ()| {
            enqueue_lua(lua, "eqoa.actions.guild_accept", ActionKind::A7(A7Action::GuildAccept))
        })?,
    )?;
    bind(
        lua,
        "actions",
        "guild_refuse",
        lua.create_function(|lua, ()| {
            enqueue_lua(lua, "eqoa.actions.guild_refuse", ActionKind::A7(A7Action::GuildRefuse))
        })?,
    )?;

    Ok(())
}
