// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! A native record built in one protected Lua call.
//!
//! An addon's Lua runs under a memory limit, so every table operation `mlua` does is a protected
//! call of its own (about half a microsecond each): a creature's record, with its 20 fields and two
//! positions, took about 27 microseconds. Here the whole record is put together by raw stack
//! operations inside one protected call, the guard tables, `$ref` userdata and everything else that
//! needs `mlua` made beforehand. What comes out is what `native::record_slow` builds: a read-only
//! proxy over a backing table whose metatable reports a missing field.
//!
//! Anything this does not do (a number that is not a finite float, a record nested too deep, a Lua
//! without a native session) is left to `record_slow`.
use mlua::{Function, Lua, Table, Value, Variadic, ffi};
use serde_json::Value as Json;

/// Deeper than any record the API makes; deeper ones take the slow path.
const MAX_DEPTH: usize = 12;

struct Plan {
    /// the missing-field metatable of each object, in the order the build meets them
    shapes: Vec<Table>,
    /// each `$ref` userdata, in the order the build meets them
    refs: Vec<Value>,
    readonly: Option<Function>,
}

/// The record for `value`, or None when it is not one this builds.
pub(super) fn build(lua: &Lua, path: &str, value: &Json) -> mlua::Result<Option<Value>> {
    if lua.app_data_ref::<super::native::Shared>().is_none() {
        return Ok(None);
    }
    let mut plan = Plan { shapes: Vec::new(), refs: Vec::new(), readonly: None };
    if !prepare(lua, path, value, 0, &mut plan)? {
        return Ok(None);
    }
    let Some(readonly) = plan.readonly.take().or_else(|| {
        // arrays of plain values have no guard; the stack still wants slot 1
        lua.create_function(|_, ()| Ok(())).ok()
    }) else {
        return Ok(None);
    };
    let shapes = plan.shapes.len();
    let mut args: Vec<Value> = Vec::with_capacity(shapes + plan.refs.len());
    args.extend(plan.shapes.into_iter().map(Value::Table));
    args.extend(plan.refs);
    // SAFETY: the closure only calls Lua stack functions on `state` with the stack slots the
    // arguments gave it, holds no value with a destructor across a call that can raise (only
    // references and integers), and leaves one result on the stack.
    let built: Value = unsafe {
        lua.exec_raw((readonly, Variadic::from_iter(args)), |state| {
            if ffi::lua_checkstack(state, 64) == 0 {
                ffi::lua_settop(state, 0);
                ffi::lua_pushnil(state);
                return;
            }
            let mut at = Walk { shape: 0, reference: 0, shapes: shapes as i32 };
            push(state, value, &mut at);
            ffi::lua_replace(state, 1);
            ffi::lua_settop(state, 1);
        })?
    };
    #[cfg(test)]
    BUILDS.with(|n| n.set(n.get() + 1));
    Ok(match built {
        Value::Nil => None,
        built => Some(built),
    })
}

#[cfg(test)]
thread_local! {static BUILDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };}
/// Records the raw build made on this thread (test evidence that the fast path ran).
#[cfg(test)]
pub(super) fn builds() -> usize {
    BUILDS.with(|n| n.get())
}

/// The first walk, in `mlua`'s terms: what the raw one needs ready, in the order it will ask.
/// False when the value has something the raw build does not do.
fn prepare(lua: &Lua, path: &str, value: &Json, depth: usize, plan: &mut Plan) -> mlua::Result<bool> {
    if depth > MAX_DEPTH {
        return Ok(false);
    }
    match value {
        Json::Number(n) => Ok(n.as_f64().is_some()),
        Json::Null | Json::Bool(_) | Json::String(_) => Ok(true),
        Json::Array(items) => {
            for item in items {
                if !prepare(lua, path, item, depth + 1, plan)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        Json::Object(fields) if fields.contains_key("$ref") => {
            plan.refs.push(super::native_state::make_ref(lua, value)?);
            Ok(true)
        }
        Json::Object(fields) => {
            let sig = super::native::signature(path, true, fields.keys().map(String::as_str));
            let (missing, readonly) =
                super::native::guard_parts(lua, sig, path, || Some(fields.keys().cloned().collect()))?;
            plan.shapes.push(missing);
            plan.readonly.get_or_insert(readonly);
            for (k, v) in fields {
                let child = super::native::nested_type(path, k).unwrap_or(path);
                if !prepare(lua, child, v, depth + 1, plan)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
    }
}

struct Walk {
    /// the next object's shape (0-based), and the next `$ref`'s
    shape: i32,
    reference: i32,
    /// how many shapes there are (the `$ref`s follow them on the stack)
    shapes: i32,
}

/// Pushes the Lua value for `value` (nothing for a null: the caller skips those).
///
/// # Safety
/// Called inside `exec_raw`'s protected call, with the stack laid out as `build` set it up:
/// slot 1 the read-only guard, then the shapes, then the `$ref`s.
unsafe fn push(state: *mut ffi::lua_State, value: &Json, at: &mut Walk) {
    unsafe {
        match value {
            Json::Null => {}
            Json::Bool(b) => ffi::lua_pushboolean(state, *b as std::os::raw::c_int),
            Json::Number(n) => ffi::lua_pushnumber(state, n.as_f64().unwrap_or(0.0)),
            Json::String(s) => ffi::lua_pushlstring_(state, s.as_ptr().cast(), s.len()),
            Json::Array(items) => {
                ffi::lua_createtable(state, items.len() as std::os::raw::c_int, 0);
                for (i, item) in items.iter().enumerate() {
                    if item.is_null() {
                        continue;
                    }
                    push(state, item, at);
                    ffi::lua_rawseti_(state, -2, (i + 1) as std::os::raw::c_int);
                }
            }
            Json::Object(fields) if fields.contains_key("$ref") => {
                ffi::lua_pushvalue(state, 2 + at.shapes + at.reference);
                at.reference += 1;
            }
            Json::Object(fields) => {
                let shape = 2 + at.shape;
                at.shape += 1;
                // the backing table: the fields, and the missing-field metatable
                ffi::lua_createtable(state, 0, fields.len() as std::os::raw::c_int);
                for (k, v) in fields {
                    if v.is_null() {
                        continue;
                    }
                    ffi::lua_pushlstring_(state, k.as_ptr().cast(), k.len());
                    push(state, v, at);
                    ffi::lua_rawset(state, -3);
                }
                ffi::lua_pushvalue(state, shape);
                ffi::lua_setmetatable(state, -2);
                // the proxy: reads go to the backing table, writes raise
                ffi::lua_createtable(state, 0, 0);
                ffi::lua_createtable(state, 0, 3);
                ffi::lua_pushvalue(state, -3);
                ffi::lua_setfield(state, -2, c"__index".as_ptr());
                ffi::lua_pushvalue(state, 1);
                ffi::lua_setfield(state, -2, c"__newindex".as_ptr());
                ffi::lua_pushboolean(state, 0);
                ffi::lua_setfield(state, -2, c"__metatable".as_ptr());
                ffi::lua_setmetatable(state, -2);
                // the proxy replaces the backing table on the stack
                ffi::lua_replace(state, -2);
            }
        }
    }
}

/// An event's envelope table (`name`, `seq`, `time`, `tick`, `frame`, `generation`, `data`) in one
/// protected call, where `set` made seven.
pub(super) fn envelope_table(lua: &Lua, name: &str, numbers: [f64; 4], seq: f64, data: Value) -> mlua::Result<Table> {
    // SAFETY: as in `build`: only Lua stack functions on `state`, no value with a destructor held
    // across a call that can raise, one result left.
    unsafe {
        lua.exec_raw(data, |state| {
            ffi::lua_createtable(state, 0, 7);
            ffi::lua_pushlstring_(state, c"name".as_ptr(), 4);
            ffi::lua_pushlstring_(state, name.as_ptr().cast(), name.len());
            ffi::lua_rawset(state, -3);
            for (key, value) in [(c"seq", seq), (c"time", numbers[0]), (c"tick", numbers[1]), (c"frame", numbers[2]), (c"generation", numbers[3])] {
                ffi::lua_pushnumber(state, value);
                ffi::lua_setfield(state, -2, key.as_ptr());
            }
            ffi::lua_pushvalue(state, 1);
            ffi::lua_setfield(state, -2, c"data".as_ptr());
            ffi::lua_replace(state, 1);
            ffi::lua_settop(state, 1);
        })
    }
}
