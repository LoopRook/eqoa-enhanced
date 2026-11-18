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

const PENDING_OPEN_TIMEOUT: Duration = Duration::from_secs(10);
const HIDDEN_SESSION_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum TakeoverKind {
    Loot = 0,
    Merchant = 1,
    Bank = 2,
}

impl TakeoverKind {
    pub(crate) const ALL: [Self; 3] = [Self::Loot, Self::Merchant, Self::Bank];

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Loot => "loot",
            Self::Merchant => "merchant",
            Self::Bank => "bank",
        }
    }

    pub(crate) fn parse(s: &str) -> std::result::Result<Self, (&'static str, String)> {
        match s {
            "loot" => Ok(Self::Loot),
            "merchant" => Ok(Self::Merchant),
            "bank" => Ok(Self::Bank),
            _ => Err((
                "invalid_argument",
                format!("window takeover kind must be 'loot', 'merchant', or 'bank', got '{s}'"),
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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HiddenSession {
    pub active: bool,
    pub vm: u64,
    pub scope: TakeoverScope,
    pub target_id: u32,
    pub opened_at_pos: Option<[u32; 3]>,
    pub last_activity: Instant,
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
        }
    }
}

/// The target the player had before a mod's `open_loot`/`talk` retargeted (M1): put back when the
/// mod is done, if the player has not chosen another target meanwhile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RestoreTarget {
    pub vm: u64,
    pub prev: u32,
    pub mod_target: u32,
}

#[derive(Default)]
struct TakeoverState {
    holders: [Option<Holder>; 3],
    pending_opens: [Option<PendingOpen>; 3],
    pending_close: [bool; 3],
    /// A hidden session ended and its close (0x0016 for a corpse, the state reset) is still to be
    /// done: it waits for a moment when it cannot disturb a native window (M3).
    close_owed: [bool; 3],
    hidden: [HiddenSession; 3],
    restore: Vec<RestoreTarget>,
}

impl TakeoverState {
    fn has_activity(&self) -> bool {
        self.holders.iter().any(Option::is_some)
            || self.pending_opens.iter().any(Option::is_some)
            || self.pending_close.iter().any(|&c| c)
            || self.close_owed.iter().any(|&c| c)
            || self.hidden.iter().any(|h| h.active)
            || !self.restore.is_empty()
    }
}

thread_local! {
    static STATE: RefCell<TakeoverState> = RefCell::new(TakeoverState::default());
    static HAS_ACTIVITY: Cell<bool> = const { Cell::new(false) };
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

pub(crate) fn takeover(
    vm: u64,
    addon_id: &str,
    kind: TakeoverKind,
    scope: TakeoverScope,
) -> std::result::Result<(), (&'static str, String)> {
    with_state_mut(|s| {
        let idx = kind as usize;
        if let Some(existing) = &mut s.holders[idx] {
            if existing.vm == vm {
                existing.scope = scope;
                // asking again while holding renews a live hidden session's idle timer (L5): a mod
                // that is still showing the session keeps it
                if s.hidden[idx].active && s.hidden[idx].vm == vm {
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
            s.pending_opens[idx] = None;
            s.pending_close[idx] = true;
            true
        } else {
            false
        }
    })
}

pub(crate) fn release_all_for_vm(vm: u64) -> bool {
    with_state_mut(|s| {
        let mut any = false;
        for idx in 0..3 {
            if s.holders[idx].as_ref().is_some_and(|h| h.vm == vm) {
                s.holders[idx] = None;
                s.pending_opens[idx] = None;
                s.pending_close[idx] = true;
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
    with_state_mut(|s| {
        for idx in 0..3 {
            if s.holders[idx].take().is_some() {
                s.pending_opens[idx] = None;
                s.pending_close[idx] = true;
            }
        }
    });
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
    if !has_activity() {
        return false;
    }
    with_state_mut(|s| {
        let idx = kind as usize;
        if s.holders[idx].as_ref().is_none_or(|h| h.vm != vm) {
            return false;
        }
        s.pending_opens[idx] = Some(PendingOpen {
            vm,
            target_id,
            expires_at: Instant::now() + PENDING_OPEN_TIMEOUT,
            request_seen: false,
            foreign: false,
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

pub(crate) fn is_hidden_active(kind: TakeoverKind) -> bool {
    if !has_activity() {
        return false;
    }
    with_state(|s| s.holders[kind as usize].is_some() && s.hidden[kind as usize].active)
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
                        };
                        known == Some(pending.target_id)
                    }
                };
                if matched {
                    s.pending_opens[idx] = None;
                    s.pending_close[idx] = false;
                    s.hidden[idx] = HiddenSession {
                        active: true,
                        vm: holder_vm,
                        scope: holder_scope,
                        target_id: pending.target_id,
                        opened_at_pos: self_pos.map(|p| p.map(f32::to_bits)),
                        last_activity: now,
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
        if holder_scope == TakeoverScope::All {
            s.pending_close[idx] = false;
            s.hidden[idx] = HiddenSession {
                active: true,
                vm: holder_vm,
                scope: TakeoverScope::All,
                target_id: observed_target.or(fallback_player_target).unwrap_or(u32::MAX),
                opened_at_pos: self_pos.map(|p| p.map(f32::to_bits)),
                last_activity: now,
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
            s.hidden[idx] = HiddenSession::default();
            s.pending_close[idx] = true;
        }
    });
}

/// One pass over the sessions: pending opens nobody still waits for (`awaiting`: the mod's request
/// is refused, timed out or gone) are dropped, hidden sessions that are over (released, mod gone,
/// timed out, walked away from, window gone: `still_valid`) are reset and their close becomes owed.
pub(crate) fn sync_sessions_with<F, W>(mut still_valid: F, awaiting: W)
where
    F: FnMut(TakeoverKind, u32, Option<[f32; 3]>) -> bool,
    W: Fn(u64, TakeoverKind) -> bool,
{
    if !has_activity() {
        return;
    }
    let now = Instant::now();
    with_state_mut(|s| {
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
                let timed_out = s.hidden[idx].scope == TakeoverScope::Own
                    && now.saturating_duration_since(s.hidden[idx].last_activity)
                        >= HIDDEN_SESSION_TIMEOUT;
                let lost = closed[idx]
                    || s.holders[idx].as_ref().is_none_or(|h| h.vm != s.hidden[idx].vm)
                    || timed_out
                    || !still_valid(kind, s.hidden[idx].target_id, opened_pos);
                if lost {
                    s.hidden[idx] = HiddenSession::default();
                    s.close_owed[idx] = true;
                }
            } else if closed[idx] {
                s.close_owed[idx] = true;
            }
        }
    });
}

/// The closes owed by ended hidden sessions, taken (cleared) so the caller sends each exactly once.
/// Call only when closing cannot disturb a native window of the player's (M3).
pub(crate) fn take_owed_closes() -> [bool; 3] {
    if !has_activity() {
        return [false; 3];
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
                || s.close_owed.iter().any(|&c| c)
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
    STATE.with(|s| *s.borrow_mut() = TakeoverState::default());
    HAS_ACTIVITY.with(|a| a.set(false));
    #[cfg(test)]
    STATE_BORROWS.with(|n| n.set(0));
}
