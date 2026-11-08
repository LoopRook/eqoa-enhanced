use super::super::{
    host::Host,
    native::{bind, error},
    native_events::{bus, token, valid_token},
};
use super::*;
use mlua::Value;
pub(super) fn pad_value(r: crate::input::Report) -> serde_json::Value {
    let d = r.data();
    let names = [
        "select", "l3", "r3", "start", "up", "right", "down", "left", "l2", "r2", "l1", "r1",
        "triangle", "circle", "cross", "square",
    ];
    serde_json::json!({"buttons":names.iter().enumerate().filter_map(|(i,n)|(r.buttons&(1<<i)!=0).then_some(n)).collect::<Vec<_>>(),"left":{"x":r.sticks[2],"y":r.sticks[3]},"right":{"x":r.sticks[0],"y":r.sticks[1]},"raw_left":{"x":d[6],"y":d[7]},"raw_right":{"x":d[4],"y":d[5]}})
}
pub(crate) fn release_vm(lua: &mlua::Lua) {
    let prefix = format!("\u{1f}{}:", bus(lua).borrow().vm);
    INPUT_STATE.with(|s| {
        let mut s = s.borrow_mut();
        let keys = s
            .key_claims
            .iter()
            .filter_map(|(k, v)| v.starts_with(&prefix).then_some(*k))
            .collect::<Vec<_>>();
        for key in keys {
            s.key_claims.remove(&key);
            s.suppress_shadow.remove(&key);
        }
        s.hook_claims.retain(|v| !v.starts_with(&prefix));
        s.text_capture_owners.retain(|v, _| !v.starts_with(&prefix));
        s.key_cursors.retain(|v, _| !v.starts_with(&prefix));
        s.dpad_claims.retain(|v| !v.starts_with(&prefix));
        s.dpad_requests.retain(|v, _| !v.starts_with(&prefix));
        s.dpad_rotated = s.dpad_requests.values().any(|v| *v);
        s.text_capture = !s.text_capture_owners.is_empty();
    });
}
pub(crate) fn install(host: &Host) -> mlua::Result<()> {
    let lua = &host.lua;
    let claims = Rc::new(RefCell::new(HashMap::<u64, u8>::new()));
    let claimed = claims.clone();
    bind(
        lua,
        "input",
        "claim",
        lua.create_function(move |lua, (key, suppress): (Value, Option<Value>)| {
            let Some(key) = key
                .as_string()
                .and_then(|s| s.to_str().ok().map(|s| s.to_string()))
            else {
                return error(
                    lua,
                    "eqoa.input.claim",
                    "invalid_argument",
                    "key must be an app key name",
                );
            };
            let code = match key.as_str() {
                "Enter" => Some(0x0d),
                "Tab" => Some(9),
                "Backspace" => Some(8),
                "Escape" => Some(0x1b),
                "Space" => Some(0x20),
                _ if key.len() == 1 && key.as_bytes()[0].is_ascii_graphic() => {
                    Some(key.as_bytes()[0])
                }
                _ => None,
            };
            let Some(code) = code else {
                return error(
                    lua,
                    "eqoa.input.claim",
                    "invalid_argument",
                    "unknown app key name",
                );
            };
            let suppress = match suppress {
                None | Some(Value::Nil) => false,
                Some(Value::Boolean(v)) => v,
                _ => {
                    return error(
                        lua,
                        "eqoa.input.claim",
                        "invalid_argument",
                        "suppress must be bool",
                    );
                }
            };
            if INPUT_STATE.with(|s| s.borrow().key_claims.contains_key(&code)) {
                return error(lua, "eqoa.input.claim", "busy", "key already has an owner");
            }
            let t = token(lua, "input");
            INPUT_STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.key_claims
                    .insert(code, format!("\u{1f}{}:native:{}", t.vm, t.id));
                s.suppress_shadow.insert(code, suppress);
            });
            claimed.borrow_mut().insert(t.id, code);
            lua.pack_multi((lua.create_userdata(t)?, Value::Nil))
        })?,
    )?;
    bind(
        lua,
        "input",
        "release",
        lua.create_function(move |lua, value: Value| {
            let Some(t) = valid_token(lua, &value, "input") else {
                return error(
                    lua,
                    "eqoa.input.release",
                    "stale_handle",
                    "token belongs to another VM or service",
                );
            };
            let Some(code) = claims.borrow_mut().remove(&t.id) else {
                return lua.pack_multi((false, Value::Nil));
            };
            let owner = format!("\u{1f}{}:native:{}", t.vm, t.id);
            let removed = INPUT_STATE.with(|s| {
                let mut s = s.borrow_mut();
                if s.key_claims.get(&code) == Some(&owner) {
                    s.key_claims.remove(&code);
                    s.suppress_shadow.remove(&code);
                    true
                } else {
                    false
                }
            });
            lua.pack_multi((removed, Value::Nil))
        })?,
    )?;
    bind(
        lua,
        "input",
        "capture_text",
        lua.create_function(|lua, value: Value| {
            let Value::Boolean(active) = value else {
                return error(
                    lua,
                    "eqoa.input.capture_text",
                    "invalid_argument",
                    "active must be bool",
                );
            };
            let owner = format!("\u{1f}{}:native:text", bus(lua).borrow().vm);
            INPUT_STATE.with(|s| {
                let mut s = s.borrow_mut();
                if active {
                    s.text_capture_owners.insert(owner, Instant::now());
                } else {
                    s.text_capture_owners.remove(&owner);
                }
                s.text_capture = !s.text_capture_owners.is_empty();
            });
            lua.pack_multi((true, Value::Nil))
        })?,
    )?;
    bind(
        lua,
        "input",
        "rotate_dpad",
        lua.create_function(|lua, value: Value| {
            let Value::Boolean(active) = value else {
                return error(
                    lua,
                    "eqoa.input.rotate_dpad",
                    "invalid_argument",
                    "active must be bool",
                );
            };
            if !super::super::native_core::CORE.with(|c| c.borrow().view.in_world) {
                return error(
                    lua,
                    "eqoa.input.rotate_dpad",
                    "not_in_world",
                    "enter a loaded world first",
                );
            }
            let owner = format!("\u{1f}{}:native:dpad", bus(lua).borrow().vm);
            INPUT_STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.dpad_requests.insert(owner, active);
                s.dpad_rotated = s.dpad_requests.values().any(|v| *v);
            });
            lua.pack_multi((true, Value::Nil))
        })?,
    )?;
    Ok(())
}
