// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Window takeover registry for native loot, merchant, and bank windows.
//!
//! When a mod claims takeover of a supported kind (`loot`, `merchant`, `bank`),
//! incoming server sessions keep their data state alive while suppressing the
//! native UI window so `Play.mode` remains `0` and takes no native draw, keys,
//! or pointer.

use std::cell::{Cell, RefCell};

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

#[derive(Clone, Debug, PartialEq, Eq)]
struct Holder {
    vm: u64,
    addon_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HiddenSession {
    pub active: bool,
    pub banker_id: u32,
}

impl Default for HiddenSession {
    fn default() -> Self {
        Self {
            active: false,
            banker_id: u32::MAX,
        }
    }
}

#[derive(Default)]
struct TakeoverState {
    holders: [Option<Holder>; 3],
    pending_close: [bool; 3],
    hidden: [HiddenSession; 3],
}

impl TakeoverState {
    fn has_activity(&self) -> bool {
        self.holders.iter().any(Option::is_some)
            || self.pending_close.iter().any(|&c| c)
            || self.hidden.iter().any(|h| h.active)
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
) -> std::result::Result<(), (&'static str, String)> {
    with_state_mut(|s| {
        let idx = kind as usize;
        if let Some(existing) = &s.holders[idx] {
            if existing.vm == vm {
                s.pending_close[idx] = false;
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
        s.pending_close[idx] = false;
        s.holders[idx] = Some(Holder {
            vm,
            addon_id: addon_id.to_string(),
        });
        Ok(())
    })
}

pub(crate) fn release(vm: u64, kind: TakeoverKind) -> bool {
    with_state_mut(|s| {
        let idx = kind as usize;
        if s.holders[idx].as_ref().is_some_and(|h| h.vm == vm) {
            s.holders[idx] = None;
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
        (s.holders[idx].is_some() && s.hidden[idx].active).then_some(s.hidden[idx].banker_id)
    })
}

pub(crate) fn activate_hidden(kind: TakeoverKind, banker_id: u32) -> bool {
    if !has_activity() {
        return false;
    }
    with_state_mut(|s| {
        let idx = kind as usize;
        if s.holders[idx].is_none() {
            return false;
        }
        s.hidden[idx].active = true;
        if kind == TakeoverKind::Bank {
            s.hidden[idx].banker_id = banker_id;
        }
        true
    })
}

pub(crate) fn sync_sessions<F>(mut on_end: F)
where
    F: FnMut(TakeoverKind, bool),
{
    if !has_activity() {
        return;
    }
    with_state_mut(|s| {
        let closed = std::mem::take(&mut s.pending_close);
        for kind in TakeoverKind::ALL {
            let idx = kind as usize;
            if closed[idx] && s.hidden[idx].active {
                s.hidden[idx] = HiddenSession::default();
                on_end(kind, true);
            } else if s.hidden[idx].active && s.holders[idx].is_none() {
                s.hidden[idx] = HiddenSession::default();
                on_end(kind, false);
            }
        }
    });
}

pub(crate) fn set_hidden_inactive(kind: TakeoverKind) {
    if !has_activity() {
        return;
    }
    with_state_mut(|s| {
        s.hidden[kind as usize] = HiddenSession::default();
    });
}

pub(crate) fn reset_all_for_test() {
    STATE.with(|s| *s.borrow_mut() = TakeoverState::default());
    HAS_ACTIVITY.with(|a| a.set(false));
    #[cfg(test)]
    STATE_BORROWS.with(|n| n.set(0));
}
