// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Native menu 3, not an inferred equipped/active weapon set.
use super::host::Host;
use mlua::{Function, MultiValue, Table};
use std::cell::RefCell;

#[derive(Clone, Copy)]
pub struct Slot {
    pub populated: bool,
    pub icon: i32,
    pub badge: i32,
    pub name: [u8; 64],
    pub name_len: u8,
}
impl Default for Slot {
    fn default() -> Self {
        Self {
            populated: false,
            icon: -1,
            badge: -1,
            name: [0; 64],
            name_len: 0,
        }
    }
}

#[derive(Clone, Copy, Default)]
pub struct WeaponSetsSnapshot {
    pub available: bool,
    pub selected: i32,
    pub slots: [Slot; crate::gear::ENTRIES],
}

impl WeaponSetsSnapshot {
    pub fn from_native(
        gear: &crate::gear::Gear,
        inv: &net::dump::ItemList,
        menus: &crate::hudmenu::HudMenus,
        ready: bool,
        words: &vi::strtab::StringTable,
        language: i32,
    ) -> Self {
        if !ready || !gear.on {
            return Self::default();
        }
        Self {
            available: true,
            selected: menus.menus[3].sel,
            slots: std::array::from_fn(|i| {
                let populated = i == crate::gear::BARE_HANDS as usize || !gear.empty(i as i64);
                let text = gear.name(i as i64, words, language);
                let mut name = [0; 64];
                let mut len = text.len().min(name.len());
                while !text.is_char_boundary(len) {
                    len -= 1;
                }
                name[..len].copy_from_slice(&text.as_bytes()[..len]);
                Slot {
                    populated,
                    icon: gear.icon(i as i64, inv),
                    badge: gear.badge(i as i64, inv),
                    name,
                    name_len: len as u8,
                }
            }),
        }
    }
}

thread_local! { static PENDING: RefCell<Vec<i64>> = const { RefCell::new(Vec::new()) }; }
pub fn take_pending() -> Vec<i64> {
    PENDING.with(|p| std::mem::take(&mut *p.borrow_mut()))
}

pub fn install(host: &Host) -> mlua::Result<()> {
    let lua = &host.lua;
    let eqoa = match lua.globals().get::<Option<Table>>("eqoa")? {
        Some(t) => t,
        None => lua.create_table()?,
    };
    let api = lua.create_table()?;
    let state = host.snapshot.clone();
    api.set(
        "IsAvailable",
        lua.create_function(move |_, ()| Ok(state.get().weapon_sets.available))?,
    )?;
    let state = host.snapshot.clone();
    api.set(
        "GetCount",
        lua.create_function(move |_, ()| {
            Ok(if state.get().weapon_sets.available {
                crate::gear::ENTRIES
            } else {
                0
            })
        })?,
    )?;
    let state = host.snapshot.clone();
    api.set(
        "GetSelected",
        lua.create_function(move |_, ()| {
            let s = state.get().weapon_sets;
            Ok(s.available.then_some(s.selected))
        })?,
    )?;
    for (name, badge) in [("GetIcon", false), ("GetBadge", true)] {
        let state = host.snapshot.clone();
        api.set(
            name,
            lua.create_function(move |lua, i: i64| {
                let s = state.get().weapon_sets;
                let Some(slot) = usize::try_from(i)
                    .ok()
                    .and_then(|i| s.slots.get(i))
                    .filter(|slt| s.available && slt.populated)
                else {
                    return Ok(MultiValue::new());
                };
                let id = if badge { slot.badge } else { slot.icon };
                if id == -1 {
                    return Ok(MultiValue::new());
                }
                let icon: Table = lua.named_registry_value("__forge_icon_mod")?;
                let get: Function = icon.get("GetTexture")?;
                get.call::<MultiValue>(id)
            })?,
        )?;
    }
    let state = host.snapshot.clone();
    api.set(
        "GetName",
        lua.create_function(move |_, i: i64| {
            let s = state.get().weapon_sets;
            Ok(usize::try_from(i)
                .ok()
                .and_then(|i| s.slots.get(i))
                .filter(|slot| s.available && slot.populated)
                .map(|slot| {
                    String::from_utf8_lossy(&slot.name[..slot.name_len as usize]).into_owned()
                }))
        })?,
    )?;
    let state = host.snapshot.clone();
    api.set(
        "IsEmpty",
        lua.create_function(move |_, i: i64| {
            let s = state.get().weapon_sets;
            Ok(!s.available
                || usize::try_from(i)
                    .ok()
                    .and_then(|i| s.slots.get(i))
                    .is_none_or(|slot| !slot.populated))
        })?,
    )?;
    let state = host.snapshot.clone();
    api.set(
        "Use",
        lua.create_function(move |_, i: i64| {
            let snap = state.get();
            let s = snap.weapon_sets;
            if !s.available
                || !matches!(snap.ui_mode, 0 | 38)
                || usize::try_from(i)
                    .ok()
                    .and_then(|i| s.slots.get(i))
                    .is_none_or(|slot| !slot.populated)
            {
                return Ok(false);
            }
            Ok(PENDING.with(|p| {
                let mut p = p.borrow_mut();
                if p.len() >= 32 {
                    return false;
                }
                p.push(i);
                true
            }))
        })?,
    )?;
    eqoa.set("WeaponSets", api)?;
    lua.globals().set("eqoa", eqoa)
}
