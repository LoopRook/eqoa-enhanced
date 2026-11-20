// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! The player's own map waypoint as mods move it (`eqoa.map.set_waypoint` / `clear_waypoint`). The pin itself
//! is the map's (`Map::set_waypoint`, the path a map click takes); this remembers which mod put the current one
//! there, per world (the map keeps one pin per world), so `eqoa.map.get().waypoint` can name its owner and
//! label, a mod clears only its own pins, and a disabled mod's still-owned pins go away in every world.
//! Ownership lasts only while that exact pin is continuously present: every game step drops, for good, the
//! record of a pin that is absent or differs (moved, reached, cleared, or cleared and re-pinned by the player),
//! and any set by another mod replaces the record. Known limit: a player who clears and re-pins the exact
//! same point between two 30 Hz game steps is not seen; catching that needs one hook line in the map's own
//! waypoint setter (upstream), which this round does not take.
use std::cell::RefCell;
use std::hash::{Hash, Hasher};

struct Owned {
    vm: u64,
    addon: String,
    world: i32,
    at: [f32; 2],
    label: Option<String>,
    released: bool,
}

thread_local! {
    // One entry per world, ordered by placement for clear_waypoint's unaddressed call.
    static OWNED: RefCell<Vec<Owned>> = const { RefCell::new(Vec::new()) };
}

/// A mod's `set_waypoint` was applied: `world`'s pin is now `vm`'s, replacing any earlier owner there.
pub(crate) fn placed(vm: u64, addon: &str, world: i32, at: [f32; 2], label: Option<String>) {
    OWNED.with(|pins| {
        let mut pins = pins.borrow_mut();
        pins.retain(|pin| pin.world != world);
        pins.push(Owned {
            vm,
            addon: addon.to_owned(),
            world,
            at,
            label,
            released: false,
        });
    });
}

/// Ownership ends permanently once a game step observes a missing or changed pin.
pub(crate) fn reconcile(current: impl Fn(i32) -> Option<[f32; 2]>) {
    OWNED.with(|pins| {
        pins.borrow_mut()
            .retain(|pin| current(pin.world) == Some(pin.at))
    });
}

/// The owning mod and label of `world`'s pin, now at `current`; none for the player's own pin.
pub(crate) fn owner(world: i32, current: Option<[f32; 2]>) -> Option<(String, Option<String>)> {
    OWNED.with(|pins| {
        let mut pins = pins.borrow_mut();
        let index = pins.iter().position(|pin| pin.world == world)?;
        if Some(pins[index].at) != current {
            pins.remove(index);
            return None;
        }
        let pin = &pins[index];
        Some((pin.addon.clone(), pin.label.clone()))
    })
}

/// The world of the pin `vm` placed most recently and still owns (`current` reads the map).
pub(crate) fn owned_by(vm: u64, current: impl Fn(i32) -> Option<[f32; 2]>) -> Option<i32> {
    reconcile(current);
    OWNED.with(|pins| {
        pins.borrow()
            .iter()
            .rev()
            .find(|pin| pin.vm == vm && !pin.released)
            .map(|pin| pin.world)
    })
}

/// `world`'s pin was taken away by its owner's own `clear_waypoint`.
pub(crate) fn cleared(world: i32) {
    OWNED.with(|pins| pins.borrow_mut().retain(|pin| pin.world != world));
}

/// A mod's VM ended: the pins it still owns go away after the next step's dispatch (the player's stay).
pub(crate) fn release_vm(vm: u64) {
    OWNED.with(|pins| {
        for pin in pins.borrow_mut().iter_mut().filter(|pin| pin.vm == vm) {
            pin.released = true;
        }
    });
}

/// Consumed after dispatch, so a same-step replacement has already retired the old record.
pub(crate) fn take_released(current: impl Fn(i32) -> Option<[f32; 2]>) -> Vec<i32> {
    OWNED.with(|pins| {
        let mut released = Vec::new();
        pins.borrow_mut().retain(|pin| {
            if !pin.released {
                return true;
            }
            if current(pin.world) == Some(pin.at) {
                released.push(pin.world);
            }
            false
        });
        released
    })
}

/// Who owns which pin, for the map record's change key.
pub(crate) fn hash_owner(h: &mut impl Hasher) {
    OWNED.with(|pins| {
        for pin in pins.borrow().iter() {
            (pin.vm, pin.world, pin.at.map(f32::to_bits), &pin.label).hash(h);
        }
    });
}

#[cfg(test)]
pub(crate) fn reset_for_test() {
    OWNED.with(|pins| pins.borrow_mut().clear());
}
