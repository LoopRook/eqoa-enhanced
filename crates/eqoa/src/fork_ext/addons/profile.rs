//! Profiles contain bounded plain Lua data, never executable serializer output.
use mlua::{Lua, Result, Table, Value};
use serde_json::{Value as Json, json};

pub(super) fn encode(value: Value) -> Result<Json> {
    fn visit(value: Value, depth: usize, left: &mut usize) -> Result<Json> {
        if depth > 16 || *left == 0 {
            return Err(mlua::Error::runtime("profile depth/entry limit exceeded"));
        }
        *left -= 1;
        Ok(match value {
            Value::Nil => Json::Null,
            Value::Boolean(v) => json!(v),
            Value::Integer(v) => json!(v),
            Value::Number(v) if v.is_finite() => json!(v),
            Value::String(v) if v.as_bytes().len() <= 16384 => json!(v.to_str()?.as_ref()),
            Value::Table(t) => {
                let mut entries = Vec::new();
                for pair in t.pairs::<Value, Value>() {
                    let (key, value) = pair?;
                    if !matches!(key, Value::String(_) | Value::Integer(_) | Value::Number(_)) {
                        return Err(mlua::Error::runtime(
                            "profile table keys must be strings or numbers",
                        ));
                    }
                    entries.push(json!([
                        visit(key, depth + 1, left)?,
                        visit(value, depth + 1, left)?
                    ]));
                }
                json!({"table":entries})
            }
            _ => return Err(mlua::Error::runtime("profile contains unsupported value")),
        })
    }
    visit(value, 0, &mut 4096)
}

pub(super) fn decode(lua: &Lua, value: &Json) -> Result<Value> {
    fn visit(lua: &Lua, value: &Json, depth: usize, left: &mut usize) -> Result<Value> {
        if depth > 16 || *left == 0 {
            return Err(mlua::Error::runtime("profile depth/entry limit exceeded"));
        }
        *left -= 1;
        Ok(match value {
            Json::Null => Value::Nil,
            Json::Bool(v) => Value::Boolean(*v),
            Json::Number(v) => Value::Number(
                v.as_f64()
                    .ok_or_else(|| mlua::Error::runtime("invalid number"))?,
            ),
            Json::String(v) if v.len() <= 16384 => Value::String(lua.create_string(v)?),
            Json::Object(o) if o.len() == 1 && o.contains_key("table") => {
                let entries = o["table"]
                    .as_array()
                    .ok_or_else(|| mlua::Error::runtime("invalid profile table"))?;
                let table: Table = lua.create_table()?;
                for entry in entries {
                    let pair = entry
                        .as_array()
                        .filter(|a| a.len() == 2)
                        .ok_or_else(|| mlua::Error::runtime("invalid profile entry"))?;
                    let key = visit(lua, &pair[0], depth + 1, left)?;
                    if !matches!(key, Value::String(_) | Value::Number(_)) {
                        return Err(mlua::Error::runtime("invalid profile key"));
                    }
                    table.set(key, visit(lua, &pair[1], depth + 1, left)?)?;
                }
                Value::Table(table)
            }
            _ => return Err(mlua::Error::runtime("invalid profile value")),
        })
    }
    visit(lua, value, 0, &mut 4096)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn forge_profile_roundtrips_numeric_keys_and_rejects_cycles_and_functions() {
        let lua = Lua::new();
        let value: Value = lua
            .load("return { spacing=60, colors={1,0.5,0,1}, enabled=true }")
            .eval()
            .unwrap();
        let result = decode(&lua, &encode(value).unwrap()).unwrap();
        let table = result.as_table().unwrap();
        assert_eq!(table.get::<i32>("spacing").unwrap(), 60);
        assert_eq!(
            table.get::<Table>("colors").unwrap().get::<f32>(2).unwrap(),
            0.5
        );
        assert!(encode(lua.load("local t={};t.self=t;return t").eval().unwrap()).is_err());
        assert!(encode(lua.load("return {run=function() end}").eval().unwrap()).is_err());
        assert!(decode(&lua, &json!({"arbitrary":"object"})).is_err());
    }
}
