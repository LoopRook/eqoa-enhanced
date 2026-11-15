// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! The Forge adapters' snapshot objects (an ability, an item, a bar slot) are tables of closures: one
//! closure per method, about twenty for an ability, made again for every `GetAbility` call every frame.
//! An object is a plain picture of a `Copy` snapshot, so while the snapshot it was made from is
//! unchanged the same table serves; the moment it differs a new one is made.
use mlua::{Lua, Result, Table};
use std::{any::Any, collections::HashMap};

/// Objects kept per VM; a bar has a few dozen, so this is never reached in play.
const MAX_OBJECTS: usize = 1024;

#[derive(Default)]
struct Cache(HashMap<(&'static str, u64), (Box<dyn Any>, Table)>);

/// The object of `kind` and `key` for `snap`: the one made last time when `snap` is what it was
/// made from, otherwise `make()` and remembered.
pub(super) fn memo<S: PartialEq + Clone + 'static>(
    lua: &Lua,
    kind: &'static str,
    key: u64,
    snap: &S,
    make: impl FnOnce() -> Result<Table>,
) -> Result<Table> {
    if let Some(cache) = lua.app_data_ref::<Cache>()
        && let Some((old, table)) = cache.0.get(&(kind, key))
        && old.downcast_ref::<S>() == Some(snap)
    {
        return Ok(table.clone());
    }
    let table = make()?;
    if lua.app_data_ref::<Cache>().is_none() {
        lua.set_app_data(Cache::default());
    }
    if let Some(mut cache) = lua.app_data_mut::<Cache>() {
        if cache.0.len() >= MAX_OBJECTS {
            cache.0.clear();
        }
        cache.0.insert((kind, key), (Box::new(snap.clone()), table.clone()));
    }
    Ok(table)
}
