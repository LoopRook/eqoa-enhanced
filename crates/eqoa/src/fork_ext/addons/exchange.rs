//! Cross-mod read-only private publications. Authority comes from a live VM,
//! never a manifest found on disk or a sibling's guessed storage path.
use super::{native_events, sandbox::Mounts};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Weak,
};

struct Owner {
    mounts: Mounts,
    bus: Weak<RefCell<native_events::Bus>>,
}
thread_local! {
    static OWNERS: RefCell<Vec<Owner>> = const { RefCell::new(Vec::new()) };
}
pub(crate) fn register(lua: &mlua::Lua) {
    let Some(mounts) = lua.app_data_ref::<Mounts>().map(|m| m.clone()) else {
        return;
    };
    let Some(bus) = native_events::weak_bus(lua) else {
        return;
    };
    let vm = bus.upgrade().unwrap().borrow().vm;
    OWNERS.with(|owners| {
        let mut owners = owners.borrow_mut();
        owners.retain(|o| o.bus.upgrade().is_some_and(|b| b.borrow().vm != vm));
        owners.push(Owner { mounts, bus });
    });
}
pub(crate) fn read(path: &str) -> Result<PathBuf, String> {
    let (id, key) = path
        .strip_prefix('@')
        .and_then(|s| s.split_once('/'))
        .ok_or("shared JSON path must be @mod_id/data/path")?;
    OWNERS.with(|owners| {
        let owners = owners.borrow();
        let owner = owners
            .iter()
            .rev()
            .find(|o| o.mounts.addon.id == id && o.bus.upgrade().is_some_and(|b| b.borrow().active))
            .ok_or("shared file owner is missing or disabled")?;
        if !owner
            .mounts
            .addon
            .extensions
            .shares
            .iter()
            .any(|s| s == key)
        {
            return Err("file is not declared in the owner's shares".into());
        }
        // shares are physical private keys: do not prepend another data/.
        let normalized = owner
            .mounts
            .key(key.strip_prefix("data/").ok_or("invalid shared path")?)?;
        if normalized != Path::new(key) {
            return Err("invalid shared path".into());
        }
        let path = owner.mounts.read_path(&normalized)?;
        super::file_io::no_links(&path)?;
        Ok(path)
    })
}
pub(crate) fn saved(mounts: &Mounts, key: &Path) {
    let path = key.to_string_lossy().replace('\\', "/");
    if mounts.addon.extensions.shares.contains(&path) {
        native_events::shared_changed(&mounts.addon.id, &path);
    }
}
