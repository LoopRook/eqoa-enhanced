// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Window takeover registry for native loot, merchant, and bank windows.
//!
//! When a mod claims takeover of a supported kind (`loot`, `merchant`, `bank`),
//! incoming server sessions keep their data state alive while suppressing the
//! native UI window so `Play.mode` remains `0` and takes no native draw, keys,
//! or pointer.

use std::cell::RefCell;

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

#[derive(Default)]
struct TakeoverState {
    holders: [Option<Holder>; 3],
    pending_close: [bool; 3],
}

thread_local! {
    static STATE: RefCell<TakeoverState> = RefCell::new(TakeoverState::default());
}

pub(crate) fn takeover(
    vm: u64,
    addon_id: &str,
    kind: TakeoverKind,
) -> std::result::Result<(), (&'static str, String)> {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
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
    STATE.with(|s| {
        let mut s = s.borrow_mut();
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
    STATE.with(|s| {
        let mut s = s.borrow_mut();
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
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        for idx in 0..3 {
            if s.holders[idx].take().is_some() {
                s.pending_close[idx] = true;
            }
        }
    });
}

pub(crate) fn is_taken(kind: TakeoverKind) -> bool {
    STATE.with(|s| s.borrow().holders[kind as usize].is_some())
}

pub(crate) fn holder_of(kind: TakeoverKind) -> Option<String> {
    STATE.with(|s| {
        s.borrow().holders[kind as usize]
            .as_ref()
            .map(|h| h.addon_id.clone())
    })
}

pub(crate) fn drain_pending_closes() -> [bool; 3] {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().pending_close))
}

pub(crate) fn reset_all_for_test() {
    STATE.with(|s| *s.borrow_mut() = TakeoverState::default());
}
