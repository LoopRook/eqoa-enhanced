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
                    native_events::command(&bus, data, || spend_player(lua, true))
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

// A player activation is scoped to one native callback boundary. A delegated
// command may spend it on a file action, but cannot delegate it again.
thread_local! { static PLAYER: std::cell::Cell<(u64,u8)> = const { std::cell::Cell::new((0,0)) }; }
pub(super) struct PlayerScope((u64, u8));
impl Drop for PlayerScope {
    fn drop(&mut self) {
        PLAYER.with(|p| p.set(self.0));
    }
}
pub(super) fn player_scope(vm: u64, direct: bool, delegated: bool) -> PlayerScope {
    PlayerScope(PLAYER.with(|p| {
        p.replace((
            vm,
            if direct {
                1
            } else if delegated {
                2
            } else {
                0
            },
        ))
    }))
}
pub(super) fn has_player(lua: &mlua::Lua) -> bool {
    let vm = native_events::bus(lua).borrow().vm;
    PLAYER.with(|p| {
        let (owner, kind) = p.get();
        owner == vm && kind != 0
    })
}
pub(super) fn spend_player(lua: &mlua::Lua, delegation: bool) -> bool {
    let vm = native_events::bus(lua).borrow().vm;
    PLAYER.with(|p| {
        let (owner, kind) = p.get();
        if owner == vm && kind != 0 && (!delegation || kind == 1) {
            p.set((vm, 0));
            true
        } else {
            false
        }
    })
}
