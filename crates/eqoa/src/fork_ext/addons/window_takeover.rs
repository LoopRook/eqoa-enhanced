// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Window takeover registry for native loot, merchant, and bank windows.
//!
//! `eqoa.windows.takeover(kind, {scope?})` supports `"own"` (default) and `"all"`.
//! With `scope = "own"`, only server sessions opened by the holding mod's own
//! `open_loot`/`talk` requests are hidden (player-started opens show normally).
//! With `scope = "all"`, every session of that kind (including player-started
//! opens) is hidden for mods drawing a full replacement window.

use std::cell::{Cell, RefCell};
use std::time::{Duration, Instant};

/// One always-on `takeover:` line, like the game's own `play: 0x....` lines: stderr, and (tests only) a buffer a test
/// reads back with `take_log_for_test`.
pub(crate) fn log(line: String) {
    eprintln!("{line}");
    #[cfg(test)]
    TEST_LOG.with(|l| l.borrow_mut().push(line));
}

#[cfg(test)]
thread_local! {
    static TEST_LOG: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// The `takeover:` lines logged on this thread since the last call.
#[cfg(test)]
pub(crate) fn take_log_for_test() -> Vec<String> {
    TEST_LOG.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

const PENDING_OPEN_TIMEOUT: Duration = Duration::from_secs(10);
const HIDDEN_SESSION_TIMEOUT: Duration = Duration::from_secs(15);
/// How long after a mod-ended corpse session the server's own list of that corpse (junk left on it) stays the mod's.
const LIST_GRACE: Duration = Duration::from_secs(3);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum TakeoverKind {
    Loot = 0,
    Merchant = 1,
    Bank = 2,
    RaidReady = 3,
    RaidRoll = 4,
    // round A2: NPC dialogs and the game's yes/no offers (`dialog_sessions`)
    TextDialog = 5,
    ResponseDialog = 6,
    GroupInvite = 7,
    ReviveOffer = 8,
    SummonOffer = 9,
    // round A7: server-pushed social prompts
    FriendRequest = 10,
    GuildInvite = 11,
    /// Round A6: the Start key's menu. A client-side window with no server session: a mod holds it whole (`all`).
    StartMenu = 12,
    /// Round A6: the chat line the Enter key opens (the chat purpose of the game's edit box only).
    ChatEntry = 13,
    /// Round A13: the full world map the map key opens. Client-side, no server session: a mod holds it whole (`all`).
    Map = 14,
    /// Round A11: the server's mercenary inventory and spell windows (modes 61, 62), the dye picker (63) and the
    /// fishing window (36), held whole (`all`) as server-opened sessions (`dialog_sessions`, `a11_play`).
    MercItems = 15,
    MercSpells = 16,
    DyePicker = 17,
    Fishing = 18,
}

impl TakeoverKind {
    /// The three server-window kinds (loot, merchant, bank) that carry hidden server sessions and batch hooks.
    pub(crate) const ALL: [Self; 3] = [Self::Loot, Self::Merchant, Self::Bank];
    /// Every takeover kind, including server-pushed raid strips (`raid_ready`, `raid_roll`).
    pub(crate) const ALL_KINDS: [Self; 19] = [
        Self::MercItems,
        Self::MercSpells,
        Self::DyePicker,
        Self::Fishing,
        Self::Loot,
        Self::Merchant,
        Self::Bank,
        Self::RaidReady,
        Self::RaidRoll,
        Self::StartMenu,
        Self::ChatEntry,
        Self::Map,
        Self::TextDialog,
        Self::ResponseDialog,
        Self::GroupInvite,
        Self::ReviveOffer,
        Self::SummonOffer,
        Self::FriendRequest,
        Self::GuildInvite,
    ];
    /// The NPC dialogs and offers of round A2: scope `all` only, held as `dialog_sessions`.
    pub(crate) const DIALOGS: [Self; 9] = [
        Self::MercItems,
        Self::MercSpells,
        Self::DyePicker,
        Self::Fishing,
        Self::TextDialog,
        Self::ResponseDialog,
        Self::GroupInvite,
        Self::ReviveOffer,
        Self::SummonOffer,
    ];

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Loot => "loot",
            Self::Merchant => "merchant",
            Self::Bank => "bank",
            Self::RaidReady => "raid_ready",
            Self::RaidRoll => "raid_roll",
            Self::StartMenu => "start_menu",
            Self::ChatEntry => "chat_entry",
            Self::Map => "map",
            Self::TextDialog => "text_dialog",
            Self::ResponseDialog => "response_dialog",
            Self::GroupInvite => "group_invite",
            Self::ReviveOffer => "revive_offer",
            Self::SummonOffer => "summon_offer",
            Self::FriendRequest => "friend_request",
            Self::GuildInvite => "guild_invite",
            Self::MercItems => "mercenary_items",
            Self::MercSpells => "mercenary_spells",
            Self::DyePicker => "dye_picker",
            Self::Fishing => "fishing",
        }
    }

    pub(crate) fn parse(s: &str) -> std::result::Result<Self, (&'static str, String)> {
        match s {
            "loot" => Ok(Self::Loot),
            "merchant" => Ok(Self::Merchant),
            "bank" => Ok(Self::Bank),
            "raid_ready" => Ok(Self::RaidReady),
            "raid_roll" => Ok(Self::RaidRoll),
            "start_menu" => Ok(Self::StartMenu),
            "chat_entry" => Ok(Self::ChatEntry),
            "map" => Ok(Self::Map),
            "text_dialog" => Ok(Self::TextDialog),
            "response_dialog" => Ok(Self::ResponseDialog),
            "group_invite" => Ok(Self::GroupInvite),
            "revive_offer" => Ok(Self::ReviveOffer),
            "summon_offer" => Ok(Self::SummonOffer),
            "friend_request" => Ok(Self::FriendRequest),
            "guild_invite" => Ok(Self::GuildInvite),
            "mercenary_items" => Ok(Self::MercItems),
            "mercenary_spells" => Ok(Self::MercSpells),
            "dye_picker" => Ok(Self::DyePicker),
            "fishing" => Ok(Self::Fishing),
            _ if super::window_kinds::is_window(s) => Err((
                "unsupported",
                format!(
                    "the game's '{s}' window cannot be taken over in this version (only 'loot', 'merchant', \
                     'bank', 'raid_ready', 'raid_roll', 'text_dialog', 'response_dialog', 'group_invite',                      'revive_offer', 'summon_offer', 'friend_request', 'guild_invite', 'start_menu', 'chat_entry' and 'map'); more windows follow the window-replacement roadmap"
                ),
            )),
            _ => Err((
                "invalid_argument",
                format!(
                    "window takeover kind must be 'loot', 'merchant', 'bank', 'raid_ready', 'raid_roll', 'text_dialog', 'response_dialog', 'group_invite', 'revive_offer', 'summon_offer', 'friend_request', 'guild_invite', 'start_menu', 'chat_entry' or 'map', got '{s}'"
                ),
            )),
        }
    }

    pub(crate) const fn from_mode(mode: i32) -> Option<Self> {
        match mode {
            1 => Some(Self::Loot),
            10 => Some(Self::Merchant),
            11 => Some(Self::Bank),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) enum TakeoverScope {
    #[default]
    Own,
    All,
}

impl TakeoverScope {
    pub(crate) fn parse(s: &str) -> std::result::Result<Self, (&'static str, String)> {
        match s {
            "own" => Ok(Self::Own),
            "all" => Ok(Self::All),
            _ => Err((
                "invalid_argument",
                format!("window takeover scope must be 'own' or 'all', got '{s}'"),
            )),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Holder {
    vm: u64,
    addon_id: String,
    scope: TakeoverScope,
}

#[derive(Clone, Copy, Debug)]
struct PendingOpen {
    vm: u64,
    target_id: u32,
    expires_at: Instant,
    /// The mod's own loot request (0x0015) went out after the claim was armed.
    request_seen: bool,
    /// Another loot request followed it while the claim waited: the reply cannot be told apart.
    foreign: bool,
    /// Sent by id (`open_loot` with `keep_target`): the player's target is not the corpse, and is not to be.
    by_id: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HiddenSession {
    pub active: bool,
    pub vm: u64,
    pub scope: TakeoverScope,
    pub target_id: u32,
    pub opened_at_pos: Option<[u32; 3]>,
    pub last_activity: Instant,
    /// The player asked for a corpse themselves (a loot request no claim of the holder stood behind) while
    /// this session was up: the next list is theirs, not the session's continuing.
    pub player_asked: bool,
    /// Merchant: a sale or buy the holder sent in this session waits for its answer, and the server answers
    /// one with the merchant's list again (owner test fix 1). That list is the session going on; a list with
    /// no such answer owed (the player talking to the merchant themselves) is the player's window.
    pub relist_owed: bool,
    /// The session was opened by id: the player's target is not the corpse, so the server's relists are the
    /// session's by the corpse id alone (never by what the player has targeted).
    pub by_id: bool,
}

impl Default for HiddenSession {
    fn default() -> Self {
        Self {
            active: false,
            vm: 0,
            scope: TakeoverScope::Own,
            target_id: u32::MAX,
            opened_at_pos: None,
            last_activity: Instant::now(),
            player_asked: false,
            relist_owed: false,
            by_id: false,
        }
    }
}

/// The target the player had before a mod's `open_loot`/`talk` retargeted (M1): put back when the
/// mod is done, if the player has not chosen another target meanwhile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct OwedClose {
    pub reason: &'static str,
    pub had_session: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RestoreTarget {
    pub vm: u64,
    pub prev: u32,
    pub mod_target: u32,
}

/// A mod's hidden corpse session just ended with its last take: the server closes the corpse and lists it again by
/// itself (what is left on it), and that list must not reach the player as the game's window. Held a moment;
/// never for a corpse the player asks for themselves.
#[derive(Clone, Copy, Debug)]
struct ListGrace {
    target_id: u32,
    until: Instant,
    player_asked: bool,
    by_id: bool,
}

#[derive(Default)]
struct TakeoverState {
    grace_loot: Option<ListGrace>,
    /// The merchant's list still owed for the mod's last sale or buy when its hidden session ended (the
    /// sale's answer can follow the action's end): held hidden for `LIST_GRACE`, then closed again quietly.
    grace_merchant: Option<ListGrace>,
    holders: [Option<Holder>; 19],
    pending_opens: [Option<PendingOpen>; 3],
    /// Why a kind's hidden session is to end (set by release, the mod finishing, the holder going).
    pending_close: [Option<&'static str>; 3],
    /// A hidden session ended and its close (0x0016 for a corpse, the state reset) is still to be
    /// done: it waits for a moment when it cannot disturb a native window (M3). Carries the reason
    /// and whether a hidden session was really up (no session, nothing to close or to send).
    close_owed: [Option<OwedClose>; 3],
    hidden: [HiddenSession; 3],
    restore: Vec<RestoreTarget>,
}

impl TakeoverState {
    fn has_activity(&self) -> bool {
        self.grace_loot.is_some()
            || self.grace_merchant.is_some()
            || self.holders.iter().any(Option::is_some)
            || self.pending_opens.iter().any(Option::is_some)
            || self.pending_close.iter().any(Option::is_some)
            || self.close_owed.iter().any(Option::is_some)
            || self.hidden.iter().any(|h| h.active)
            || !self.restore.is_empty()
    }
}

thread_local! {
    static STATE: RefCell<TakeoverState> = RefCell::new(TakeoverState::default());
    static HAS_ACTIVITY: Cell<bool> = const { Cell::new(false) };
    /// The "Use the game's windows" switch; none until first read from the fork's settings.
    static USE_GAME_WINDOWS: Cell<Option<bool>> = const { Cell::new(None) };
    #[cfg(test)]
    static STATE_BORROWS: Cell<usize> = const { Cell::new(0) };
}

fn count_borrow() {
    #[cfg(test)]
    STATE_BORROWS.with(|n| n.set(n.get().saturating_add(1)));
}

fn with_state<R>(f: impl FnOnce(&TakeoverState) -> R) -> R {
    count_borrow();
    STATE.with(|s| f(&s.borrow()))
}

fn with_state_mut<R>(f: impl FnOnce(&mut TakeoverState) -> R) -> R {
    count_borrow();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let out = f(&mut s);
        HAS_ACTIVITY.with(|a| a.set(s.has_activity()));
        out
    })
}

pub(crate) fn has_activity() -> bool {
    HAS_ACTIVITY.with(Cell::get)
}

#[cfg(test)]
pub(crate) fn take_state_borrows_for_test() -> usize {
    STATE_BORROWS.with(|n| n.replace(0))
}

/// The fork's own settings file (`fork_ext/settings.rs`); the switch is its `use_game_windows` line, and
/// every other line in it is kept as it is.
const SETTINGS_FILE: &str = "enhanced.opt";
const SWITCH_KEY: &str = "use_game_windows=";

pub(crate) fn use_game_windows() -> bool {
    USE_GAME_WINDOWS.with(|value| {
        value.get().unwrap_or_else(|| {
            let saved = crate::settings::read_config(SETTINGS_FILE).is_some_and(|bytes| {
                String::from_utf8_lossy(&bytes)
                    .lines()
                    .any(|line| line.strip_prefix(SWITCH_KEY) == Some("on"))
            });
            value.set(Some(saved));
            saved
        })
    })
}

fn save_switch(on: bool) {
    let old = crate::settings::read_config(SETTINGS_FILE).unwrap_or_default();
    let mut text = String::new();
    for line in String::from_utf8_lossy(&old).lines() {
        if !line.starts_with(SWITCH_KEY) {
            text.push_str(line);
            text.push('\n');
        }
    }
    text.push_str(SWITCH_KEY);
    text.push_str(if on { "on" } else { "off" });
    text.push('\n');
    if let Err(error) = crate::settings::write_config(SETTINGS_FILE, text.as_bytes()) {
        log(format!("takeover: the game's windows switch was not saved: {error}"));
    }
}

/// What a restart does to the switch's in-memory value: forgotten, read again on first use.
#[cfg(test)]
pub(crate) fn forget_switch_for_test() {
    USE_GAME_WINDOWS.with(|value| value.set(None));
}

/// The player's "Use the game's windows" switch, an emergency override for full replacements: saved with the
/// fork's settings. It governs only `scope = "all"` claims (owner, 2026-11-20, option a): turning it on releases
/// every `all` claim and stops each running batch that worked through one, as its Stop button would, and while
/// it is on an `all` claim is refused `unavailable`. Helper claims (`own`, hiding only the sessions the mod
/// itself opened) are untouched.
pub(crate) fn set_use_game_windows(on: bool) {
    USE_GAME_WINDOWS.with(|value| value.set(Some(on)));
    save_switch(on);
    if on {
        // round A1: every window a mod has open closes at once (told to its mod), a question it asked is
        // answered No, and the held corpse, merchant and bank sessions end with the claims released below
        super::window_sessions::end_all("game_windows");
    }
    if on && has_activity() {
        let released = release_claims_where("released by the player", |scope| scope == TakeoverScope::All);
        super::action_broker::stop_batches_of_released_claims(&released);
    }
}

pub(crate) fn takeover(
    vm: u64,
    addon_id: &str,
    kind: TakeoverKind,
    scope: TakeoverScope,
) -> std::result::Result<(), (&'static str, String)> {
    if scope == TakeoverScope::All && use_game_windows() {
        return Err((
            "unavailable",
            "the player is using the game's windows: no mod replaces them".into(),
        ));
    }
    with_state_mut(|s| {
        let idx = kind as usize;
        if let Some(existing) = &mut s.holders[idx] {
            if existing.vm == vm {
                existing.scope = scope;
                // round A2: asking again while holding keeps a dialog the mod is showing from going idle
                super::dialog_sessions::touch_vm(vm);
                // asking again while holding renews a live hidden session's idle timer (L5): a mod
                // that is still showing the session keeps it
                if idx < 3 && s.hidden[idx].active && s.hidden[idx].vm == vm {
                    s.hidden[idx].last_activity = Instant::now();
                }
                return Ok(());
            }
            return Err((
                "busy",
                format!(
                    "{} window is already taken over by '{}'",
                    kind.name(),
                    existing.addon_id
                ),
            ));
        }
        s.holders[idx] = Some(Holder {
            vm,
            addon_id: addon_id.to_string(),
            scope,
        });
        Ok(())
    })
}

pub(crate) fn release(vm: u64, kind: TakeoverKind) -> bool {
    with_state_mut(|s| {
        let idx = kind as usize;
        if s.holders[idx].as_ref().is_some_and(|h| h.vm == vm) {
            s.holders[idx] = None;
            if idx < 3 {
                s.pending_opens[idx] = None;
                s.pending_close[idx] = Some("released by the mod");
            }
            true
        } else {
            false
        }
    })
}

pub(crate) fn release_all_for_vm(vm: u64) -> bool {
    with_state_mut(|s| {
        let mut any = false;
        for kind in TakeoverKind::ALL_KINDS {
            let idx = kind as usize;
            if s.holders[idx].as_ref().is_some_and(|h| h.vm == vm) {
                s.holders[idx] = None;
                if idx < 3 {
                    s.pending_opens[idx] = None;
                    s.pending_close[idx] = Some("mod released or went away");
                }
                any = true;
            }
        }
        any
    })
}

pub(crate) fn release_vm(vm: u64) {
    let _ = release_all_for_vm(vm);
}

pub(crate) fn release_all() {
    // round A2: the dialogs and offers in the registry are the old world's
    super::dialog_sessions::world_changed();
    // round A1: the mod windows, held sessions, controller focus and any question end with the world
    super::window_sessions::world_changed();
    let _ = release_claims_where("zoning", |_| true);
    // a by-id open of the world left that still waits for its answer decides nothing in the next one
    crate::fork_ext::loot_by_id::clear();
    // a list after the mod's last take or sale belongs to the zone left: a new zone may reuse its entity id
    with_state_mut(|s| {
        s.grace_loot = None;
        s.grace_merchant = None;
    });
}

/// Releases every claim whose scope `which` selects; returns who held what.
fn release_claims_where(reason: &'static str, which: impl Fn(TakeoverScope) -> bool) -> Vec<(u64, TakeoverKind)> {
    with_state_mut(|s| {
        let mut released = Vec::new();
        for kind in TakeoverKind::ALL_KINDS {
            let idx = kind as usize;
            if s.holders[idx].as_ref().is_some_and(|h| which(h.scope))
                && let Some(holder) = s.holders[idx].take()
            {
                if idx < 3 {
                    s.pending_opens[idx] = None;
                    s.pending_close[idx] = Some(reason);
                }
                released.push((holder.vm, kind));
            }
        }
        released
    })
}

pub(crate) fn is_taken(kind: TakeoverKind) -> bool {
    if !has_activity() {
        return false;
    }
    with_state(|s| s.holders[kind as usize].is_some())
}

pub(crate) fn holder_vm(kind: TakeoverKind) -> Option<u64> {
    if !has_activity() {
        return None;
    }
    with_state(|s| s.holders[kind as usize].as_ref().map(|h| h.vm))
}

pub(crate) fn holder_of(kind: TakeoverKind) -> Option<String> {
    if !has_activity() {
        return None;
    }
    with_state(|s| {
        s.holders[kind as usize]
            .as_ref()
            .map(|h| h.addon_id.clone())
    })
}

pub(crate) fn arm_pending_open(kind: TakeoverKind, vm: u64, target_id: u32) -> bool {
    arm_pending_open_with(kind, vm, target_id, false)
}

/// `arm_pending_open` for a corpse asked for by id: the open and the session it starts never depend on the player's
/// target (`open_loot` with `keep_target`).
pub(crate) fn arm_pending_open_by_id(vm: u64, corpse_id: u32) -> bool {
    arm_pending_open_with(TakeoverKind::Loot, vm, corpse_id, true)
}

fn arm_pending_open_with(kind: TakeoverKind, vm: u64, target_id: u32, by_id: bool) -> bool {
    if !has_activity() {
        return false;
    }
    with_state_mut(|s| {
        let idx = kind as usize;
        if s.holders[idx].as_ref().is_none_or(|h| h.vm != vm) {
            return false;
        }
        if kind == TakeoverKind::Loot {
            s.grace_loot = None;
        }
        if kind == TakeoverKind::Merchant {
            s.grace_merchant = None;
        }
        s.pending_opens[idx] = Some(PendingOpen {
            vm,
            target_id,
            expires_at: Instant::now() + PENDING_OPEN_TIMEOUT,
            request_seen: false,
            foreign: false,
            by_id,
        });
        true
    })
}

pub(crate) fn clear_pending_open(kind: TakeoverKind) {
    if !has_activity() {
        return;
    }
    with_state_mut(|s| {
        s.pending_opens[kind as usize] = None;
    });
}

/// The holder's sale or buy just went to the server in its hidden merchant session: the merchant's list the
/// server answers it with is owed to the session (`decide_open`).
pub(crate) fn note_merchant_transaction_sent() {
    if !has_activity() {
        return;
    }
    with_state_mut(|s| {
        let h = &mut s.hidden[TakeoverKind::Merchant as usize];
        if h.active {
            h.relist_owed = true;
        }
    });
}

/// The holder of `kind` while its hidden session is up: its `(vm, addon)`.
pub(crate) fn hidden_holder(kind: TakeoverKind) -> Option<(u64, String)> {
    if !has_activity() {
        return None;
    }
    with_state(|s| {
        let idx = kind as usize;
        s.holders[idx]
            .as_ref()
            .filter(|h| s.hidden[idx].active && s.hidden[idx].vm == h.vm)
            .map(|h| (h.vm, h.addon_id.clone()))
    })
}

/// Round A1 `pass`: the hidden session of `kind` is handed to the game's own window. The session just stops being
/// hidden: nothing is closed and nothing is sent (the server's corpse, merchant or bank stays open and is now the
/// player's native window). The mod keeps its claim for the next opening. False when `vm` has no hidden session of it.
pub(crate) fn hand_back(vm: u64, kind: TakeoverKind) -> bool {
    if !TakeoverKind::ALL.contains(&kind) {
        return false;
    }
    with_state_mut(|s| {
        let idx = kind as usize;
        if s.holders[idx].as_ref().is_some_and(|h| h.vm == vm) && s.hidden[idx].active && s.hidden[idx].vm == vm {
            s.hidden[idx] = HiddenSession::default();
            s.pending_opens[idx] = None;
            s.pending_close[idx] = None;
            s.close_owed[idx] = None;
            true
        } else {
            false
        }
    })
}

pub(crate) fn is_hidden_active(kind: TakeoverKind) -> bool {
    if !has_activity() {
        return false;
    }
    with_state(|s| s.holders[kind as usize].is_some() && s.hidden[kind as usize].active)
}

/// The corpse a by-id open of the holder is for: its hidden session, or the open still waiting for its list. The player's
/// target is not this corpse (that is the point of `keep_target`), so everything that elsewhere reads "the corpse" from
/// the target reads it from here first.
pub(crate) fn by_id_corpse() -> Option<u32> {
    if !has_activity() {
        return None;
    }
    with_state(|s| {
        let idx = TakeoverKind::Loot as usize;
        // a corpse the player asked for themselves meanwhile (a pending open another loot request overlapped, a session
        // the player's own request interrupted) is the player's: never the mod's source, never the mod's to loot or close
        if s.holders[idx].is_some() && s.hidden[idx].active && s.hidden[idx].by_id && !s.hidden[idx].player_asked {
            return Some(s.hidden[idx].target_id);
        }
        s.pending_opens[idx].filter(|p| p.by_id && !p.foreign).map(|p| p.target_id)
    })
}

pub(crate) fn hidden_bank_session() -> Option<u32> {
    if !has_activity() {
        return None;
    }
    with_state(|s| {
        let idx = TakeoverKind::Bank as usize;
        (s.holders[idx].is_some() && s.hidden[idx].active).then_some(s.hidden[idx].target_id)
    })
}

/// What became of an incoming server open (`0x0018`, `0x00B7`, `0x124D`) the takeover looked at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OpenDecision {
    /// Nobody holds this kind: nothing to decide.
    NoHolder,
    Accepted(&'static str),
    Declined(&'static str),
}

impl OpenDecision {
    pub(crate) fn accepted(self) -> bool {
        matches!(self, Self::Accepted(_))
    }
}

/// A loot request (0x0015) was just sent by the client. The first one after a claim was armed is the
/// mod's own (the claim is armed in the same step that sends it); any further one while the claim
/// waits makes the reply ambiguous.
pub(crate) fn note_loot_request(target_id: u32) {
    if !has_activity() {
        return;
    }
    with_state_mut(|s| {
        if let Some(p) = s.pending_opens[TakeoverKind::Loot as usize].as_mut() {
            if p.request_seen || target_id != p.target_id {
                p.foreign = true;
            } else {
                p.request_seen = true;
            }
        } else {
            // no claim of the holder's stands behind this request: the player's own
            if s.hidden[TakeoverKind::Loot as usize].active {
                s.hidden[TakeoverKind::Loot as usize].player_asked = true;
            }
            if let Some(g) = s.grace_loot.as_mut() {
                g.player_asked = true;
            }
        }
    });
}

/// Decides an incoming server open (`0x0018`, `0x00B7`, `0x124D`): hidden when
/// - `holder.scope == TakeoverScope::Own` and a pending `open_loot`/`talk` exists whose target the
///   open is **known** to match: loot, when no other loot request overlapped the mod's (the client
///   knows its own requests, whatever the server's request bookkeeping says; with an overlap, the
///   observed id decides); merchant: the observed shop id; bank: the player's target. An open that
///   cannot be tied to the pending request is left to the native window, OR
/// - `holder.scope == TakeoverScope::All` (both mod-started and player-started opens).
pub(crate) fn decide_open(
    kind: TakeoverKind,
    observed_target: Option<u32>,
    fallback_player_target: Option<u32>,
    self_pos: Option<[f32; 3]>,
) -> OpenDecision {
    if !has_activity() {
        return OpenDecision::NoHolder;
    }
    let now = Instant::now();
    with_state_mut(|s| {
        let idx = kind as usize;
        if let Some(p) = s.pending_opens[idx]
            && p.expires_at <= now
        {
            s.pending_opens[idx] = None;
        }
        if kind == TakeoverKind::Loot
            && let Some(g) = s.grace_loot
        {
            if g.until <= now {
                s.grace_loot = None;
            } else if s.pending_opens[idx].is_none()
                && !s.hidden[idx].active
                && (g.by_id || fallback_player_target == Some(g.target_id))
            {
                s.grace_loot = None;
                if !g.player_asked {
                    // closed again at once, quietly (the corpse is the server's to forget)
                    s.close_owed[idx] = Some(OwedClose {
                        reason: "the server listed the corpse again after the mod's last take",
                        had_session: true,
                    });
                    return OpenDecision::Accepted(
                        "the server lists the corpse again after the mod's last take",
                    );
                }
            }
        }
        // The answer to the mod's last sale or buy, after its hidden session ended (and the claim may be
        // released): the session's, closed again at once, quietly.
        if kind == TakeoverKind::Merchant
            && let Some(g) = s.grace_merchant
        {
            if g.until <= now {
                s.grace_merchant = None;
            } else if s.pending_opens[idx].is_none()
                && !s.hidden[idx].active
                && observed_target == Some(g.target_id)
            {
                s.grace_merchant = None;
                s.close_owed[idx] = Some(OwedClose {
                    reason: "the server listed the merchant again after the mod's last sale or buy",
                    had_session: true,
                });
                return OpenDecision::Accepted(
                    "the server lists the merchant again after the mod's last sale or buy",
                );
            }
        }
        let Some((holder_vm, holder_scope)) = s.holders[idx].as_ref().map(|h| (h.vm, h.scope))
        else {
            s.pending_opens[idx] = None;
            return OpenDecision::NoHolder;
        };
        let mut why: &'static str = "no request of the holder is waiting for this window";
        if let Some(pending) = s.pending_opens[idx] {
            if pending.vm != holder_vm {
                s.pending_opens[idx] = None;
            } else {
                let matched = match holder_scope {
                    TakeoverScope::All => {
                        observed_target.is_none_or(|obs| obs == pending.target_id)
                    }
                    TakeoverScope::Own => {
                        let known = match kind {
                            TakeoverKind::Bank => fallback_player_target,
                            TakeoverKind::Merchant => observed_target,
                            TakeoverKind::Loot if !pending.foreign => Some(pending.target_id),
                            TakeoverKind::Loot => observed_target,
                            _ => None,
                        };
                        known == Some(pending.target_id)
                    }
                };
                if matched {
                    s.pending_opens[idx] = None;
                    s.pending_close[idx] = None;
                    s.hidden[idx] = HiddenSession {
                        active: true,
                        vm: holder_vm,
                        scope: holder_scope,
                        target_id: pending.target_id,
                        opened_at_pos: self_pos.map(|p| p.map(f32::to_bits)),
                        last_activity: now,
                        player_asked: false,
                        relist_owed: false,
                        by_id: pending.by_id,
                    };
                    return OpenDecision::Accepted("answers the holder's own request");
                }
                why = if pending.foreign {
                    "another loot request overlapped the holder's; the reply cannot be told apart"
                } else {
                    "does not match the holder's pending request (other target or unknown)"
                };
            }
        }
        // The live server closes a corpse after every take (0x0016) and lists it again by itself (0x0018, no
        // request of the client before it): that list is the holder's hidden session going on, not a window
        // for the player. Unless the player asked for a corpse themselves meanwhile (`player_asked`).
        if kind == TakeoverKind::Loot && holder_scope == TakeoverScope::Own {
            let h = &mut s.hidden[idx];
            if h.active && h.vm == holder_vm && (h.by_id || fallback_player_target == Some(h.target_id)) {
                if h.player_asked {
                    h.player_asked = false;
                    why = "the player asked for this corpse themselves";
                } else {
                    h.last_activity = now;
                    return OpenDecision::Accepted(
                        "the server lists the holder's hidden corpse again (it closes and lists it after each take)",
                    );
                }
            }
        }
        // The server answers a sale or buy with the merchant's list again (owner test 2026-11-20: three
        // 0x00B7 for one talk, one per sale): while the holder's own sale waits for it, that list is the
        // hidden merchant session going on. Without one owed, it is the player's own talk to that merchant.
        if kind == TakeoverKind::Merchant && holder_scope == TakeoverScope::Own {
            let h = &mut s.hidden[idx];
            if h.active && h.vm == holder_vm && observed_target == Some(h.target_id) {
                if h.relist_owed {
                    h.relist_owed = false;
                    h.last_activity = now;
                    return OpenDecision::Accepted(
                        "the server lists the holder's hidden merchant again after its sale or buy",
                    );
                }
                why = "no sale or buy of the holder's waits for this list (the player's own talk)";
            }
        }
        if holder_scope == TakeoverScope::All {
            s.pending_close[idx] = None;
            s.hidden[idx] = HiddenSession {
                active: true,
                vm: holder_vm,
                scope: TakeoverScope::All,
                target_id: observed_target.or(fallback_player_target).unwrap_or(u32::MAX),
                opened_at_pos: self_pos.map(|p| p.map(f32::to_bits)),
                last_activity: now,
                player_asked: false,
                relist_owed: false,
                by_id: false,
            };
            return OpenDecision::Accepted(
                "holder takes over every window of this kind (scope all)",
            );
        }
        OpenDecision::Declined(why)
    })
}

pub(crate) fn try_accept_open(
    kind: TakeoverKind,
    observed_target: Option<u32>,
    fallback_player_target: Option<u32>,
    self_pos: Option<[f32; 3]>,
) -> bool {
    decide_open(kind, observed_target, fallback_player_target, self_pos).accepted()
}

pub(crate) fn touch_hidden_session(kind: TakeoverKind) {
    if !has_activity() {
        return;
    }
    let now = Instant::now();
    with_state_mut(|s| {
        let idx = kind as usize;
        if s.hidden[idx].active {
            s.hidden[idx].last_activity = now;
        }
    });
}

pub(crate) fn end_session_for_vm(vm: u64, kind: TakeoverKind) {
    if !has_activity() {
        return;
    }
    with_state_mut(|s| {
        let idx = kind as usize;
        if s.hidden[idx].active
            && s.hidden[idx].vm == vm
            && s.hidden[idx].scope == TakeoverScope::Own
        {
            if kind == TakeoverKind::Loot {
                s.grace_loot = Some(ListGrace {
                    target_id: s.hidden[idx].target_id,
                    until: Instant::now() + LIST_GRACE,
                    // a request the player sent while the session was up is still theirs when the grace begins
                    player_asked: s.hidden[idx].player_asked,
                    by_id: s.hidden[idx].by_id,
                });
            }
            if kind == TakeoverKind::Merchant && s.hidden[idx].relist_owed {
                s.grace_merchant = Some(ListGrace {
                    target_id: s.hidden[idx].target_id,
                    until: Instant::now() + LIST_GRACE,
                    player_asked: false,
                    by_id: false,
                });
            }
            s.hidden[idx] = HiddenSession::default();
            s.pending_close[idx] = Some("the mod's action or batch finished");
            s.close_owed[idx] = Some(OwedClose {
                reason: "the mod's action or batch finished",
                had_session: true,
            });
        }
    });
}

/// One pass over the sessions: pending opens nobody still waits for (`awaiting`: the mod's request
/// is refused, timed out or gone) are dropped, hidden sessions that are over (released, mod gone,
/// timed out, walked away from, window gone: `still_valid`) are reset and their close becomes owed.
pub(crate) fn sync_sessions_with<F, W>(mut still_valid: F, awaiting: W)
where
    F: FnMut(TakeoverKind, u32, Option<[f32; 3]>) -> Result<(), &'static str>,
    W: Fn(u64, TakeoverKind) -> bool,
{
    if !has_activity() {
        return;
    }
    let now = Instant::now();
    with_state_mut(|s| {
        if s.grace_loot.is_some_and(|g| g.until <= now) {
            s.grace_loot = None;
        }
        if s.grace_merchant.is_some_and(|g| g.until <= now) {
            s.grace_merchant = None;
        }
        let closed = std::mem::take(&mut s.pending_close);
        for kind in TakeoverKind::ALL {
            let idx = kind as usize;
            if let Some(p) = s.pending_opens[idx]
                && (p.expires_at <= now
                    || s.holders[idx].as_ref().is_none_or(|h| h.vm != p.vm)
                    || !awaiting(p.vm, kind))
            {
                s.pending_opens[idx] = None;
            }
            if s.hidden[idx].active {
                let opened_pos = s.hidden[idx].opened_at_pos.map(|b| b.map(f32::from_bits));
                let reason: Option<&'static str> = if let Some(r) = closed[idx] {
                    Some(r)
                } else if s.holders[idx]
                    .as_ref()
                    .is_none_or(|h| h.vm != s.hidden[idx].vm)
                {
                    Some("the holding mod is gone")
                } else if s.hidden[idx].scope == TakeoverScope::Own
                    && now.saturating_duration_since(s.hidden[idx].last_activity)
                        >= HIDDEN_SESSION_TIMEOUT
                {
                    Some("idle timeout (15 s without use)")
                } else {
                    still_valid(kind, s.hidden[idx].target_id, opened_pos).err()
                };
                if let Some(reason) = reason {
                    s.hidden[idx] = HiddenSession::default();
                    // an earlier owed close (the action finished) keeps its reason
                    if s.close_owed[idx].is_none() {
                        s.close_owed[idx] = Some(OwedClose {
                            reason,
                            had_session: true,
                        });
                    }
                }
            } else if let Some(reason) = closed[idx] {
                // released with no hidden session up: nothing to close, nothing to send
                if s.close_owed[idx].is_none() {
                    s.close_owed[idx] = Some(OwedClose {
                        reason,
                        had_session: false,
                    });
                }
            }
        }
    });
}

/// The closes owed by ended hidden sessions, taken (cleared) so the caller sends each exactly once.
/// Call only when closing cannot disturb a native window of the player's (M3).
pub(crate) fn take_owed_closes() -> [Option<OwedClose>; 3] {
    if !has_activity() {
        return [None; 3];
    }
    with_state_mut(|s| std::mem::take(&mut s.close_owed))
}

/// A mod's `open_loot`/`talk` moved the player's target from `prev` to `mod_target` (M1). The first
/// `prev` of a mod's run is kept; `mod_target` follows the latest retarget.
pub(crate) fn note_retarget(vm: u64, prev: u32, mod_target: u32) {
    with_state_mut(|s| {
        if let Some(e) = s.restore.iter_mut().find(|e| e.vm == vm) {
            e.mod_target = mod_target;
        } else if prev != 0 && prev != u32::MAX && prev != mod_target {
            s.restore.push(RestoreTarget { vm, prev, mod_target });
        }
    });
}

/// The retargets whose mod is done: no request waits for an open, no hidden session of the mod
/// lives, and `vm_idle(vm)` (no queued or in-flight action, no batch). Taken, so each is put back once.
pub(crate) fn take_restorable<I: Fn(u64) -> bool>(vm_idle: I) -> Vec<RestoreTarget> {
    if !has_activity() {
        return Vec::new();
    }
    with_state_mut(|s| {
        let busy = |s: &TakeoverState, vm: u64| {
            s.pending_opens.iter().flatten().any(|p| p.vm == vm)
                || s.hidden.iter().any(|h| h.active && h.vm == vm)
                || s.close_owed.iter().any(Option::is_some)
                || !vm_idle(vm)
        };
        let mut out = Vec::new();
        let mut keep = Vec::new();
        for e in std::mem::take(&mut s.restore) {
            if busy(s, e.vm) {
                keep.push(e);
            } else {
                out.push(e);
            }
        }
        s.restore = keep;
        out
    })
}

pub(crate) fn set_hidden_inactive(kind: TakeoverKind) {
    if !has_activity() {
        return;
    }
    with_state_mut(|s| {
        s.hidden[kind as usize] = HiddenSession::default();
    });
}

#[cfg(test)]
pub(crate) fn activate_hidden_for_test(vm: u64, kind: TakeoverKind, target_id: u32) -> bool {
    arm_pending_open(kind, vm, target_id)
        && try_accept_open(kind, Some(target_id), Some(target_id), None)
}

/// A clock seam: the live hidden session last used `idle` ago (round 7H pins the idle timeout through it).
#[cfg(test)]
pub(crate) fn age_hidden_session_for_test(kind: TakeoverKind, idle: Duration) {
    with_state_mut(|s| {
        let idx = kind as usize;
        if s.hidden[idx].active {
            s.hidden[idx].last_activity = Instant::now() - idle;
        }
    });
}

#[cfg(test)]
pub(crate) fn expire_hidden_session_for_test(kind: TakeoverKind) {
    with_state_mut(|s| {
        let idx = kind as usize;
        if s.hidden[idx].active {
            s.hidden[idx].last_activity = Instant::now() - HIDDEN_SESSION_TIMEOUT - Duration::from_secs(1);
        }
    });
}

pub(crate) fn reset_all_for_test() {
    USE_GAME_WINDOWS.with(|value| value.set(Some(false)));
    STATE.with(|s| *s.borrow_mut() = TakeoverState::default());
    HAS_ACTIVITY.with(|a| a.set(false));
    #[cfg(test)]
    STATE_BORROWS.with(|n| n.set(0));
}
