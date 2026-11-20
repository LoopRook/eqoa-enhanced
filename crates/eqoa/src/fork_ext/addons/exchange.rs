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
    static PENDING_OWNERS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Enabled owners in the current manager frame whose VMs have yet to start.
pub(super) struct PendingOwners(Vec<String>);
impl Drop for PendingOwners {
    fn drop(&mut self) {
        PENDING_OWNERS.with(|pending| pending.replace(std::mem::take(&mut self.0)));
    }
}
pub(super) fn pending_owners<'a>(ids: impl IntoIterator<Item = &'a str>) -> PendingOwners {
    PendingOwners(
        PENDING_OWNERS
            .with(|pending| pending.replace(ids.into_iter().map(str::to_owned).collect())),
    )
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
/// A pending owner returns `Ok(None)` until its live VM can authorize the read.
pub(super) fn read(path: &str) -> Result<Option<PathBuf>, String> {
    let (id, key) = path
        .strip_prefix('@')
        .and_then(|s| s.split_once('/'))
        .ok_or("shared JSON path must be @mod_id/data/path")?;
    OWNERS.with(|owners| {
        let owners = owners.borrow();
        let Some(owner) = owners.iter().rev().find(|o| {
            o.mounts.addon.id == id && o.bus.upgrade().is_some_and(|b| b.borrow().active)
        }) else {
            if PENDING_OWNERS.with(|pending| pending.borrow().iter().any(|owner| owner == id)) {
                return Ok(None);
            }
            return Err("shared file owner is missing or disabled".into());
        };
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
        Ok(Some(path))
    })
}
pub(crate) fn saved(mounts: &Mounts, key: &Path) {
    let path = key.to_string_lossy().replace('\\', "/");
    if mounts.addon.extensions.shares.contains(&path) {
        native_events::shared_changed(&mounts.addon.id, &path);
    }
}

/// Copy a small JSON command payload before queueing; no Lua value crosses VMs.
fn payload(
    value: mlua::Value,
    depth: usize,
    left: &mut usize,
    bytes: &mut usize,
) -> Result<serde_json::Value, String> {
    use mlua::Value;
    if depth > 8 || *left == 0 {
        return Err("command payload exceeds depth/node quota".into());
    }
    *left -= 1;
    *bytes = bytes
        .checked_sub(1)
        .ok_or("command payload exceeds 4 KiB")?;
    Ok(match value {
        Value::Nil => serde_json::Value::Null,
        Value::Boolean(v) => v.into(),
        Value::Integer(v) => v.into(),
        Value::Number(v) => serde_json::Number::from_f64(v)
            .ok_or("nonfinite command number")?
            .into(),
        Value::String(v) => {
            if v.as_bytes().len() > 4096 {
                return Err("command string exceeds quota".into());
            }
            *bytes = bytes
                .checked_sub(v.as_bytes().len())
                .ok_or("command payload exceeds 4 KiB")?;
            v.to_str()
                .map_err(|_| "command strings must be UTF-8")?
                .to_string()
                .into()
        }
        Value::Table(t) => {
            let len = t.raw_len();
            if len > *left {
                return Err("command array exceeds quota".into());
            }
            if len > 0 {
                let mut array = Vec::with_capacity(len);
                for i in 1..=len {
                    array.push(payload(
                        t.raw_get(i).map_err(|e| e.to_string())?,
                        depth + 1,
                        left,
                        bytes,
                    )?);
                }
                let mut count = 0;
                for pair in t.pairs::<Value, Value>() {
                    let (k, _) = pair.map_err(|e| e.to_string())?;
                    if !matches!(k, Value::Integer(i) if i>0 && i as usize<=len) {
                        return Err("command arrays cannot have named keys".into());
                    }
                    count += 1;
                    if count > len {
                        return Err("invalid command array".into());
                    }
                }
                array.into()
            } else {
                let mut object = serde_json::Map::new();
                for pair in t.pairs::<Value, Value>() {
                    let (key, value) = pair.map_err(|e| e.to_string())?;
                    let Value::String(key) = key else {
                        return Err("command object keys must be strings".into());
                    };
                    if key.as_bytes().len() > 256 {
                        return Err("command key exceeds quota".into());
                    }
                    *bytes = bytes
                        .checked_sub(key.as_bytes().len())
                        .ok_or("command payload exceeds 4 KiB")?;
                    object.insert(
                        key.to_str()
                            .map_err(|_| "command key must be UTF-8")?
                            .to_string(),
                        payload(value, depth + 1, left, bytes)?,
                    );
                }
                object.into()
            }
        }
        _ => return Err("command arguments must be JSON values".into()),
    })
}
pub(crate) fn install(host: &super::host::Host) -> mlua::Result<()> {
    use super::native::{bind, error};
    bind(
        &host.lua,
        "mods",
        "send",
        host.lua.create_function(
            |lua, (target, command, args): (String, String, Option<mlua::Value>)| {
                let Some(mounts) = lua.app_data_ref::<Mounts>().map(|m| m.clone()) else {
                    return error(
                        lua,
                        "eqoa.mods.send",
                        "unavailable",
                        "mod identity unavailable",
                    );
                };
                let args = match payload(args.unwrap_or(mlua::Value::Nil), 0, &mut 256, &mut 4096) {
                    Ok(args) => args,
                    Err(e) => return error(lua, "eqoa.mods.send", "quota", &e),
                };
                if serde_json::to_vec(&args)
                    .map_err(mlua::Error::external)?
                    .len()
                    > 4096
                {
                    return error(
                        lua,
                        "eqoa.mods.send",
                        "quota",
                        "command payload exceeds 4 KiB",
                    );
                }
                let result = OWNERS.with(|owners| {
                    let owners = owners.borrow();
                    let owner = owners
                        .iter()
                        .rev()
                        .find(|o| {
                            o.mounts.addon.id == target
                                && o.bus.upgrade().is_some_and(|b| b.borrow().active)
                        })
                        .ok_or(("unavailable", "target mod is missing or disabled"))?;
                    if !owner.mounts.addon.extensions.commands.contains(&command) {
                        return Err(("invalid_argument", "target did not declare this command"));
                    }
                    let data =
                        serde_json::json!({"from":mounts.addon.id,"command":command,"args":args});
                    let bus = owner
                        .bus
                        .upgrade()
                        .ok_or(("unavailable", "target VM has ended"))?;
                    native_events::command(&bus, data, || delegate_player(lua))
                        .map_err(|_| ("quota", "target command queue is full"))
                });
                match result {
                    Ok(()) => lua.pack_multi((true, mlua::Value::Nil)),
                    Err((code, reason)) => error(lua, "eqoa.mods.send", code, reason),
                }
            },
        )?,
    )
}

// One physical activation is shared by all observing VMs. State 0 is direct,
// 1 is transferred to a declared command, and 2 is spent/expired.
type Action = std::sync::Arc<std::sync::atomic::AtomicU8>;
#[derive(Default)]
struct PlayerGrant {
    vm: u64,
    kind: u8,
    action: Option<Action>,
}
thread_local! {static PLAYER:RefCell<PlayerGrant>=RefCell::new(PlayerGrant::default());}
pub(super) struct PlayerScope(Option<PlayerGrant>);
impl Drop for PlayerScope {
    fn drop(&mut self) {
        let current = PLAYER.with(|p| p.replace(self.0.take().unwrap()));
        if current.kind == 2 {
            if let Some(action) = current.action {
                let _ = action.compare_exchange(
                    1,
                    2,
                    std::sync::atomic::Ordering::AcqRel,
                    std::sync::atomic::Ordering::Acquire,
                );
            }
        }
    }
}
pub(super) fn player_scope(
    vm: u64,
    direct: bool,
    delegated: bool,
    action: Option<Action>,
) -> PlayerScope {
    let grant = PlayerGrant {
        vm,
        kind: if direct {
            1
        } else if delegated {
            2
        } else {
            0
        },
        action,
    };
    PlayerScope(Some(PLAYER.with(|p| p.replace(grant))))
}
pub(super) fn has_player(lua: &mlua::Lua) -> bool {
    let vm = native_events::bus(lua).borrow().vm;
    PLAYER.with(|p| {
        let p = p.borrow();
        p.vm == vm
            && p.kind != 0
            && p.action.as_ref().is_some_and(|a| {
                a.load(std::sync::atomic::Ordering::Acquire) == if p.kind == 1 { 0 } else { 1 }
            })
    })
}
pub(super) fn spend_player(lua: &mlua::Lua, _delegation: bool) -> bool {
    let vm = native_events::bus(lua).borrow().vm;
    PLAYER.with(|p| {
        let p = p.borrow();
        p.vm == vm
            && p.kind != 0
            && p.action.as_ref().is_some_and(|a| {
                a.compare_exchange(
                    if p.kind == 1 { 0 } else { 1 },
                    2,
                    std::sync::atomic::Ordering::AcqRel,
                    std::sync::atomic::Ordering::Acquire,
                )
                .is_ok()
            })
    })
}
fn delegate_player(lua: &mlua::Lua) -> Option<Action> {
    let vm = native_events::bus(lua).borrow().vm;
    PLAYER.with(|p| {
        let p = p.borrow();
        if p.vm != vm || p.kind != 1 {
            return None;
        }
        let action = p.action.as_ref()?;
        action
            .compare_exchange(
                0,
                1,
                std::sync::atomic::Ordering::AcqRel,
                std::sync::atomic::Ordering::Acquire,
            )
            .ok()
            .map(|_| action.clone())
    })
}
