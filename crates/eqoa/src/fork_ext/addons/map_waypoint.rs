// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! The player's own map waypoint as mods move it (`eqoa.map.set_waypoint` / `clear_waypoint`). The pin itself
//! is the map's (`Map::set_waypoint`, the path a map click takes); this remembers which mod put the current one
//! there, so `eqoa.map.get().waypoint` can name its owner and label, a mod clears only its own pin, and a
//! disabled mod's still-owned pin goes away. Ownership ends when the pin moves, is reached or is taken away.
use std::cell::RefCell;
use std::hash::{Hash, Hasher};

#[derive(Clone, Debug, PartialEq)]
struct Owned {
    vm: u64,
    addon: String,
    world: i32,
    at: [f32; 2],
    label: Option<String>,
}

thread_local! {
    static OWNED: RefCell<Option<Owned>> = const { RefCell::new(None) };
    /// Pins a disabled mod still owned, taken away at the next step that reaches the map.
    static ORPHANED: RefCell<Vec<(i32, [f32; 2])>> = const { RefCell::new(Vec::new()) };
}

/// A mod's `set_waypoint` was applied: `world`'s waypoint is now at `at`.
pub(crate) fn placed(vm: u64, addon: &str, world: i32, at: [f32; 2], label: Option<String>) {
    OWNED.with(|o| {
        *o.borrow_mut() = Some(Owned {
            vm,
            addon: addon.to_owned(),
            world,
            at,
            label,
        })
    });
}

/// The owning mod and label of `world`'s waypoint, now at `current`; none for the player's own pin. A pin that
/// moved or went away since a mod put it there is the player's from then on.
pub(crate) fn owner(world: i32, current: Option<[f32; 2]>) -> Option<(String, Option<String>)> {
    OWNED.with(|o| {
        let mut o = o.borrow_mut();
        let owned = o.as_ref()?;
        if owned.world != world {
            return None;
        }
        if Some(owned.at) != current {
            *o = None;
            return None;
        }
        Some((owned.addon.clone(), owned.label.clone()))
    })
}

/// `vm`'s pin, when it is still the map's waypoint of its world (`current` reads the map).
pub(crate) fn owned_by(vm: u64, current: impl Fn(i32) -> Option<[f32; 2]>) -> Option<i32> {
    OWNED.with(|o| {
        let mut o = o.borrow_mut();
        let owned = o.as_ref().filter(|owned| owned.vm == vm)?;
        let world = owned.world;
        if current(world) != Some(owned.at) {
            *o = None;
            return None;
        }
        Some(world)
    })
}

/// `vm`'s pin was taken away by its own `clear_waypoint`.
pub(crate) fn cleared(vm: u64) {
    OWNED.with(|o| {
        let mut o = o.borrow_mut();
        if o.as_ref().is_some_and(|owned| owned.vm == vm) {
            *o = None;
        }
    });
}

/// A mod's VM ended: a pin it still owns goes away at the next step (the player's own pins stay).
pub(crate) fn release_vm(vm: u64) {
    OWNED.with(|o| {
        let mut o = o.borrow_mut();
        if let Some(owned) = o.take_if(|owned| owned.vm == vm) {
            ORPHANED.with(|r| r.borrow_mut().push((owned.world, owned.at)));
        }
    });
}

/// The pins disabled mods still owned; allocates nothing when there are none.
pub(crate) fn take_orphaned() -> Vec<(i32, [f32; 2])> {
    ORPHANED.with(|r| {
        let mut r = r.borrow_mut();
        if r.is_empty() {
            Vec::new()
        } else {
            std::mem::take(&mut *r)
        }
    })
}

/// Who owns the pin, for the map record's change key.
pub(crate) fn hash_owner(h: &mut impl Hasher) {
    OWNED.with(|o| {
        if let Some(owned) = &*o.borrow() {
            (owned.vm, owned.world, owned.at.map(f32::to_bits), &owned.label).hash(h);
        }
    });
}

#[cfg(test)]
pub(crate) fn reset_for_test() {
    OWNED.with(|o| *o.borrow_mut() = None);
    ORPHANED.with(|r| r.borrow_mut().clear());
}
