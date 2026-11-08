//! Settings use the manager's currently selected storage slot and atomic writer.
use super::{
    host::Host,
    native::{bind, error},
    native_events::{Token, token},
};
use mlua::{Function, Lua, Result, Value};
use serde_json::Value as Json;
use std::{cell::RefCell, rc::Rc};
pub(crate) type Writer = Rc<dyn Fn(&Json) -> std::result::Result<(), String>>;
#[derive(Default)]
pub(crate) struct Slot {
    pub value: Option<Json>,
    pub committed: Option<Json>,
    pub writer: Option<Writer>,
    save: Option<(Token, Function)>,
    load: Option<(Token, Function)>,
}
type Shared = Rc<RefCell<Slot>>;
pub(crate) fn slot(lua: &Lua) -> Shared {
    lua.app_data_ref::<Shared>()
        .expect("settings slot installed")
        .clone()
}
pub(crate) fn configure(host: &Host, value: Option<Json>, writer: Writer) {
    let s = slot(&host.lua);
    let mut s = s.borrow_mut();
    s.value = value;
    s.writer = Some(writer);
}
pub(crate) fn install(host: &Host) -> Result<()> {
    let lua = &host.lua;
    lua.set_app_data(Rc::new(RefCell::new(Slot::default())));
    bind(
        lua,
        "settings",
        "load",
        lua.create_function(|lua, ()| {
            let value = slot(lua).borrow().value.clone();
            lua.pack_multi((
                value
                    .map(|v| super::profile::decode(lua, &v))
                    .transpose()?
                    .unwrap_or(Value::Nil),
                Value::Nil,
            ))
        })?,
    )?;
    bind(
        lua,
        "settings",
        "save",
        lua.create_function(|lua, value: Value| {
            if let Err(e) = validate(&value, 0, &mut Vec::new()) {
                return error(lua, "eqoa.settings.save", "invalid_argument", &e);
            }
            let encoded = match super::profile::encode(value) {
                Ok(v) => v,
                Err(e) => return error(lua, "eqoa.settings.save", "quota", &e.to_string()),
            };
            if serde_json::to_vec(&encoded)
                .map_err(mlua::Error::external)?
                .len()
                > 1024 * 1024
            {
                return error(lua, "eqoa.settings.save", "quota", "settings exceed 1 MiB");
            }
            let writer = slot(lua).borrow().writer.clone();
            let Some(writer) = writer else {
                return error(
                    lua,
                    "eqoa.settings.save",
                    "unavailable",
                    "no active manager profile storage",
                );
            };
            if let Err(e) = writer(&encoded) {
                return error(lua, "eqoa.settings.save", "io_error", &e);
            }
            let shared = slot(lua);
            let mut s = shared.borrow_mut();
            s.value = Some(encoded.clone());
            s.committed = Some(encoded);
            lua.pack_multi((true, Value::Nil))
        })?,
    )?;
    for (name, is_save) in [("on_save", true), ("on_load", false)] {
        bind(
            lua,
            "settings",
            name,
            lua.create_function(move |lua, callback: Function| {
                let t = token(lua, "settings");
                let s = slot(lua);
                let mut s = s.borrow_mut();
                if is_save {
                    s.save = Some((t.clone(), callback));
                } else {
                    s.load = Some((t.clone(), callback));
                }
                lua.pack_multi((lua.create_userdata(t)?, Value::Nil))
            })?,
        )?;
    }
    Ok(())
}
pub(crate) fn callback(lua: &Lua, kind: i32, value: &Value) -> Option<Result<Value>> {
    let s = lua.app_data_ref::<Shared>()?.clone();
    if kind == 2 {
        let callback = s.borrow().save.as_ref().map(|(_, f)| f.clone());
        if let Some(callback) = callback {
            return Some(callback.call::<Value>(()).and_then(|v| {
                validate(&v, 0, &mut Vec::new()).map_err(mlua::Error::runtime)?;
                Ok(v)
            }));
        }
        let forge_has_save = lua
            .named_registry_value::<mlua::Table>("forge_callbacks")
            .ok()
            .and_then(|t| t.get::<Option<Function>>(3).ok())
            .flatten()
            .is_some();
        if !forge_has_save {
            if let Some(value) = &s.borrow().value {
                return Some(super::profile::decode(lua, value));
            }
        }
    } else if kind == 3 {
        match super::profile::encode(value.clone()) {
            Ok(v) => s.borrow_mut().value = Some(v),
            Err(e) => return Some(Err(e)),
        }
        let callback = s.borrow().load.as_ref().map(|(_, f)| f.clone());
        if let Some(callback) = callback {
            return Some(callback.call(value.clone()));
        }
    }
    None
}
fn validate(v: &Value, depth: usize, parents: &mut Vec<usize>) -> std::result::Result<(), String> {
    if depth > 32 {
        return Err("settings exceed depth 32".into());
    }
    match v {
        Value::Nil | Value::Boolean(_) | Value::String(_) | Value::Integer(_) => Ok(()),
        Value::Number(n) if n.is_finite() => Ok(()),
        Value::Table(t) => {
            if t.metatable().is_some() {
                return Err("settings tables cannot have metatables".into());
            }
            let id = t.to_pointer() as usize;
            if parents.contains(&id) {
                return Err("settings cannot contain cycles".into());
            }
            parents.push(id);
            let mut strings = false;
            let mut integers = Vec::new();
            for pair in t.clone().pairs::<Value, Value>() {
                let (k, v) = pair.map_err(|e| e.to_string())?;
                match k {
                    Value::String(_) => strings = true,
                    Value::Integer(i) if i > 0 => integers.push(i as u64),
                    Value::Number(n) if n.is_finite() && n > 0.0 && n.fract() == 0.0 => {
                        integers.push(n as u64)
                    }
                    _ => {
                        return Err("settings keys must be strings or dense positive indexes".into());
                    }
                };
                validate(&v, depth + 1, parents)?;
            }
            if strings && !integers.is_empty() {
                return Err("settings cannot mix string and array keys".into());
            }
            integers.sort_unstable();
            if integers.iter().enumerate().any(|(i, n)| *n != i as u64 + 1) {
                return Err("settings arrays must be dense".into());
            }
            parents.pop();
            Ok(())
        }
        _ => Err("settings require plain finite data without handles or functions".into()),
    }
}
