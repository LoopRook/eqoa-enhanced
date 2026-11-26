// Copyright LoopRook. SPDX-License-Identifier: AGPL-3.0-or-later
//! Change events for the systems a mod could only poll (spec section 3, "The events"): `craft.changed`,
//! `mail.changed`, `auction.changed`, `raid.changed` and `factions.changed`. They have the shape of round A11's
//! `merc.changed`, `dye.changed` and `fishing.changed`: `{revision:uint, open:bool}`, no payload (a mod reads the
//! system with its own `get`). The write site that learns of a change (`native_a8::publish`, `native_a10`'s mail and
//! auction slots, `native_a5::set_factions`, `raid_strips::update`) calls `note`; `flush`, once per game iteration
//! (`fork_ext/play.rs`, `begin_iteration`), tells each system's pending change as ONE event however many writes
//! came in that step, and only to a mod that listens.
//!
//! Cost: a system nobody listens to costs one cached lookup per change at the write site (`native_events::interested`:
//! an O(1) test on each VM's subscription index) and nothing per frame. The raid roster, which has no write site of
//! its own, is compared with one hash per iteration and only while a mod listens to `raid.changed`.
use super::native_core::CORE;
use serde_json::json;
use std::cell::{Cell, RefCell};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Sys {
    Craft = 0,
    Mail = 1,
    Auction = 2,
    Raid = 3,
    Factions = 4,
}

impl Sys {
    pub(crate) const ALL: [Self; 5] = [Self::Craft, Self::Mail, Self::Auction, Self::Raid, Self::Factions];

    pub(crate) const fn event(self) -> &'static str {
        match self {
            Self::Craft => "craft.changed",
            Self::Mail => "mail.changed",
            Self::Auction => "auction.changed",
            Self::Raid => "raid.changed",
            Self::Factions => "factions.changed",
        }
    }
}

thread_local! {
    /// What each system's next event says: `(revision, open)`, the latest write of the step.
    static PENDING: RefCell<[Option<(u64, bool)>; 5]> = const { RefCell::new([None; 5]) };
    /// Whether any is pending: `flush` costs one load when none is.
    static ANY: Cell<bool> = const { Cell::new(false) };
    /// The revision counters of the systems that keep none of their own (the raid roster, the faction page).
    static COUNTERS: Cell<[u64; 5]> = const { Cell::new([0; 5]) };
}

/// Does some mod listen for `sys`' event? Callers that must do work to find a change ask first.
pub(crate) fn wanted(sys: Sys) -> bool {
    super::native_events::interested(sys.event())
}

/// The next revision of a system that counts its own changes.
pub(crate) fn next_revision(sys: Sys) -> u64 {
    COUNTERS.with(|c| {
        let mut all = c.get();
        all[sys as usize] += 1;
        c.set(all);
        all[sys as usize]
    })
}

/// A system's data changed (or went): its event is owed at the next `flush`. Several notes before it are one event.
pub(crate) fn note(sys: Sys, revision: u64, open: bool) {
    if !wanted(sys) {
        return;
    }
    PENDING.with(|p| p.borrow_mut()[sys as usize] = Some((revision, open)));
    ANY.with(|a| a.set(true));
}

/// Tells the pending changes, one event per system. Run once per game iteration.
pub(crate) fn flush() {
    if !ANY.with(Cell::get) {
        return;
    }
    ANY.with(|a| a.set(false));
    let owed = PENDING.with(|p| std::mem::take(&mut *p.borrow_mut()));
    for sys in Sys::ALL {
        let Some((revision, open)) = owed[sys as usize] else { continue };
        if !wanted(sys) {
            continue;
        }
        let name = sys.event();
        let data = json!({"revision": revision, "open": open});
        CORE.with(|c| {
            if let Ok(mut c) = c.try_borrow_mut() {
                c.emit(name, data);
            } else {
                super::window_sessions::queue_event(name, data);
            }
        });
    }
}

/// The world changed: a change of the old world is not told in the new one.
pub(crate) fn world_changed() {
    PENDING.with(|p| *p.borrow_mut() = [None; 5]);
    ANY.with(|a| a.set(false));
}

#[cfg(test)]
pub(crate) fn reset_for_test() {
    world_changed();
    COUNTERS.with(|c| c.set([0; 5]));
}

#[cfg(test)]
pub(crate) fn pending_for_test() -> bool {
    ANY.with(Cell::get)
}
