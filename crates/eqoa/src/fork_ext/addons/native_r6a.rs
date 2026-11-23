// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round 6 Part A native Lua bindings (`eqoa.quests.*`, `eqoa.mastery.*`, and later camera/windows).
use super::{
    host::Host,
    native::{bind, describe, diagnose, error, record, require_capability, session},
    native_core::{CORE, View},
    native_state::Ref,
    window_takeover::{self, TakeoverKind, TakeoverScope},
};
use mlua::{Lua, Result, Value};
use serde_json::{Value as Json, json};

pub(crate) fn resolve_in(value: &Value, list: &[Json], generation: u64, kind: &str) -> Option<Json> {
    let ud = value.as_userdata()?;
    let r = ud.borrow::<Ref>().ok()?;
    if r.kind != kind || r.generation != generation {
        return None;
    }
    list.iter()
        .find(|entry| {
            entry["ref"]["key"].as_str() == Some(r.key.as_str())
                && entry["ref"]["incarnation"].as_u64() == Some(r.incarnation)
        })
        .cloned()
}

fn world_read<F>(
    lua: &Lua,
    symbol: &str,
    path: &str,
    f: F,
) -> Result<mlua::MultiValue>
where
    F: FnOnce(&View) -> Json,
{
    let value = CORE.with(|c| {
        let c = c.borrow();
        c.view.in_world.then(|| f(&c.view))
    });
    match value {
        Some(v) => lua.pack_multi((record(lua, path, &v)?, Value::Nil)),
        None => error(lua, symbol, "not_in_world", "enter a loaded world first"),
    }
}

pub(crate) fn format_hint_str(button: &str, label: &str) -> Option<String> {
    crate::binds::BUTTONS
        .iter()
        .find(|(name, _)| *name == button)
        .map(|(_, glyph)| format!("{glyph}{label}"))
}

pub(crate) fn parse_hint_json(text: &str) -> Vec<Json> {
    let mut out: Vec<Json> = Vec::new();
    let mut cur_btn: &str = "text";
    let mut buf = String::new();
    let flush = |btn: &str, buf: &mut String, out: &mut Vec<Json>| {
        let label = buf.trim();
        if !label.is_empty() {
            out.push(json!({"button": btn, "label": label}));
        }
        buf.clear();
    };
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'|' {
            if let Some(next_ch) = text[i + 1..].chars().next() {
                let esc_len = 1 + next_ch.len_utf8();
                let esc = &text[i..i + esc_len];
                if esc == "|-" || esc == "|+" {
                    i += 2;
                    continue;
                }
                if esc == "||" {
                    buf.push('|');
                    i += 2;
                    continue;
                }
                if let Some((btn_name, _)) = crate::binds::BUTTONS.iter().find(|(_, g)| *g == esc)
                {
                    flush(cur_btn, &mut buf, &mut out);
                    cur_btn = btn_name;
                    i += esc_len;
                    continue;
                }
            }
        }
        let ch = text[i..].chars().next().unwrap();
        buf.push(ch);
        i += ch.len_utf8();
    }
    flush(cur_btn, &mut buf, &mut out);
    out
}

pub(crate) fn install(host: &Host) -> Result<()> {
    let lua = &host.lua;

    // eqoa.quests.*
    bind(
        lua,
        "quests",
        "list",
        lua.create_function(|lua, ()| {
            world_read(lua, "eqoa.quests.list", "eqoa.quests.Quest", |v| {
                json!(v.quests)
            })
        })?,
    )?;
    bind(
        lua,
        "quests",
        "get",
        lua.create_function(|lua, r: Value| {
            let (world, entry) = CORE.with(|c| {
                let c = c.borrow();
                (
                    c.view.in_world,
                    resolve_in(&r, &c.view.quests, c.generation, "quest"),
                )
            });
            if !world {
                return error(
                    lua,
                    "eqoa.quests.get",
                    "not_in_world",
                    "enter a loaded world first",
                );
            }
            let Some(entry) = entry else {
                return error(
                    lua,
                    "eqoa.quests.get",
                    "stale_handle",
                    "quest handle is stale or invalid",
                );
            };
            lua.pack_multi((record(lua, "eqoa.quests.Quest", &entry)?, Value::Nil))
        })?,
    )?;

    // eqoa.mastery.*
    bind(
        lua,
        "mastery",
        "get",
        lua.create_function(|lua, ()| {
            world_read(lua, "eqoa.mastery.get", "eqoa.mastery.Mastery", |v| {
                v.mastery.clone().unwrap_or(Json::Null)
            })
        })?,
    )?;
    bind(
        lua,
        "mastery",
        "skills",
        lua.create_function(|lua, ()| {
            let (world, skills) = CORE.with(|c| {
                let c = c.borrow();
                (c.view.in_world, c.view.mastery_skills.clone())
            });
            if !world {
                return error(
                    lua,
                    "eqoa.mastery.skills",
                    "not_in_world",
                    "enter a loaded world first",
                );
            }
            let Some(skills) = skills else {
                return error(
                    lua,
                    "eqoa.mastery.skills",
                    "unavailable",
                    "waiting for class mastery skill list (sub-op 8)",
                );
            };
            lua.pack_multi((
                record(lua, "eqoa.mastery.MasterySkill", &json!(skills))?,
                Value::Nil,
            ))
        })?,
    )?;
    bind(
        lua,
        "mastery",
        "selected",
        lua.create_function(|lua, ()| {
            let (world, node) = CORE.with(|c| {
                let c = c.borrow();
                (c.view.in_world, c.view.mastery_node.clone())
            });
            if !world {
                return error(
                    lua,
                    "eqoa.mastery.selected",
                    "not_in_world",
                    "enter a loaded world first",
                );
            }
            match node {
                Some(n) => lua.pack_multi((record(lua, "eqoa.mastery.MasteryNode", &n)?, Value::Nil)),
                None => lua.pack_multi((Value::Nil, Value::Nil)),
            }
        })?,
    )?;

    // eqoa.camera.*
    bind(
        lua,
        "camera",
        "get",
        lua.create_function(|lua, ()| {
            let (world, camera) = CORE.with(|c| {
                let c = c.borrow();
                (c.view.in_world, c.view.camera.clone())
            });
            if !world {
                return error(
                    lua,
                    "eqoa.camera.get",
                    "not_in_world",
                    "enter a loaded world first",
                );
            }
            if camera.is_null() {
                return error(
                    lua,
                    "eqoa.camera.get",
                    "unavailable",
                    "waiting for camera state from a walker frame",
                );
            }
            lua.pack_multi((record(lua, "eqoa.camera.Camera", &camera)?, Value::Nil))
        })?,
    )?;
    bind(
        lua,
        "camera",
        "compass",
        lua.create_function(|lua, ()| {
            let (world, compass) = CORE.with(|c| {
                let c = c.borrow();
                (c.view.in_world, c.view.camera_compass)
            });
            if !world {
                return error(
                    lua,
                    "eqoa.camera.compass",
                    "not_in_world",
                    "enter a loaded world first",
                );
            }
            let Some(compass) = compass else {
                return error(
                    lua,
                    "eqoa.camera.compass",
                    "unavailable",
                    "waiting for camera direction from a walker frame",
                );
            };
            lua.pack_multi((compass, Value::Nil))
        })?,
    )?;

    // eqoa.windows.*
    bind(
        lua,
        "windows",
        "get",
        lua.create_function(|lua, ()| {
            let wnd = CORE.with(|c| c.borrow().view.windows_state.clone());
            lua.pack_multi((record(lua, "eqoa.windows.Windows", &wnd)?, Value::Nil))
        })?,
    )?;
    bind(
        lua,
        "windows",
        "find",
        lua.create_function(|lua, kind: Value| {
            let Some(kind) = kind.as_string().and_then(|s| s.to_str().ok().map(|s| s.to_string()))
            else {
                return error(
                    lua,
                    "eqoa.windows.find",
                    "invalid_argument",
                    "kind must be a string",
                );
            };
            let found = CORE.with(|c| {
                let c = c.borrow();
                c.view.windows_state["windows"]
                    .as_array()
                    .and_then(|arr| {
                        arr.iter()
                            .find(|w| {
                                w["kind"].as_str() == Some(kind.as_str())
                                    && w["visible"].as_bool() == Some(true)
                            })
                            .cloned()
                    })
            });
            match found {
                Some(w) => lua.pack_multi((record(lua, "eqoa.windows.Window", &w)?, Value::Nil)),
                None => lua.pack_multi((Value::Nil, Value::Nil)),
            }
        })?,
    )?;
    bind(
        lua,
        "windows",
        "get_window",
        lua.create_function(|lua, r: Value| {
            let found = CORE.with(|c| {
                let c = c.borrow();
                let list = c.view.windows_state["windows"].as_array()?;
                resolve_in(&r, list, c.generation, "window")
            });
            let Some(w) = found else {
                return error(
                    lua,
                    "eqoa.windows.get_window",
                    "stale_handle",
                    "window handle is stale or invalid",
                );
            };
            lua.pack_multi((record(lua, "eqoa.windows.Window", &w)?, Value::Nil))
        })?,
    )?;
    bind(
        lua,
        "windows",
        "format_hint",
        lua.create_function(|lua, (button, label): (Value, Value)| {
            let Some(button) = button
                .as_string()
                .and_then(|s| s.to_str().ok().map(|s| s.to_string()))
            else {
                return error(
                    lua,
                    "eqoa.windows.format_hint",
                    "invalid_argument",
                    "button must be a string",
                );
            };
            let Some(label) = label
                .as_string()
                .and_then(|s| s.to_str().ok().map(|s| s.to_string()))
            else {
                return error(
                    lua,
                    "eqoa.windows.format_hint",
                    "invalid_argument",
                    "label must be a string",
                );
            };
            let Some(formatted) = format_hint_str(&button, &label) else {
                return error(
                    lua,
                    "eqoa.windows.format_hint",
                    "invalid_argument",
                    "unknown button name",
                );
            };
            lua.pack_multi((formatted, Value::Nil))
        })?,
    )?;
    bind(
        lua,
        "windows",
        "parse_hint",
        lua.create_function(|lua, text: Value| {
            let Some(text) = text
                .as_string()
                .and_then(|s| s.to_str().ok().map(|s| s.to_string()))
            else {
                return error(
                    lua,
                    "eqoa.windows.parse_hint",
                    "invalid_argument",
                    "text must be a string",
                );
            };
            let parsed = parse_hint_json(&text);
            lua.pack_multi((
                record(lua, "eqoa.windows.ButtonHint", &json!(parsed))?,
                Value::Nil,
            ))
        })?,
    )?;
    bind(
        lua,
        "windows",
        "takeover",
        lua.create_function(|lua, (kind, opts): (Value, Option<Value>)| {
            if let Err((code, msg)) =
                require_capability(lua, "eqoa.windows.takeover", "eqoa.windows.takeover")
            {
                return error(lua, "eqoa.windows.takeover", code, &msg);
            }
            let Some(kind_str) = kind
                .as_string()
                .and_then(|s| s.to_str().ok().map(|s| s.to_string()))
            else {
                return error(
                    lua,
                    "eqoa.windows.takeover",
                    "invalid_argument",
                    "kind must be 'loot', 'merchant', or 'bank'",
                );
            };
            if !super::window_kinds::is_window(&kind_str) {
                return error(lua, "eqoa.windows.takeover", "invalid_argument", "kind must name a game window");
            }
            let scope = match opts {
                None | Some(Value::Nil) => TakeoverScope::Own,
                Some(Value::Table(t)) => match t.get::<Value>("scope")? {
                    Value::Nil => TakeoverScope::Own,
                    Value::String(s) => {
                        let Ok(s) = s.to_str() else {
                            return error(
                                lua,
                                "eqoa.windows.takeover",
                                "invalid_argument",
                                "options.scope must be UTF-8",
                            );
                        };
                        match TakeoverScope::parse(&s) {
                            Ok(sc) => sc,
                            Err((code, msg)) => {
                                return error(lua, "eqoa.windows.takeover", code, &msg);
                            }
                        }
                    }
                    _ => {
                        return error(
                            lua,
                            "eqoa.windows.takeover",
                            "invalid_argument",
                            "options.scope must be 'own' or 'all'",
                        );
                    }
                },
                Some(_) => {
                    return error(
                        lua,
                        "eqoa.windows.takeover",
                        "invalid_argument",
                        "options must be a table or nil",
                    );
                }
            };
            if scope == TakeoverScope::Own && super::window_kinds::is_view(&kind_str) {
                return error(lua, "eqoa.windows.takeover", "invalid_argument", "a local view requires scope 'all'");
            }
            if scope == TakeoverScope::Own && super::window_kinds::is_strip(&kind_str) {
                return error(lua, "eqoa.windows.takeover", "invalid_argument", "a raid strip requires scope 'all'");
            }
            let parsed = match TakeoverKind::parse(&kind_str) {
                Ok(k) => k,
                Err((code, msg)) => return error(lua, "eqoa.windows.takeover", code, &msg),
            };
            let addon = {
                let session = session(lua);
                let session = session.borrow();
                (
                    session.addon.clone(),
                    session.replaces.iter().any(|kind| kind == parsed.name()),
                )
            };
            let (addon, declared) = addon;
            if scope == TakeoverScope::All && !declared {
                return error(
                    lua,
                    "eqoa.windows.takeover",
                    "not_declared",
                    &format!(
                        "scope 'all' for '{}' requires a matching 'replaces' entry in mod.json",
                        parsed.name()
                    ),
                );
            }
            let vm = super::native_events::bus(lua).borrow().vm;
            match window_takeover::takeover(vm, &addon, parsed, scope) {
                Ok(()) => lua.pack_multi((true, Value::Nil)),
                Err((code, msg)) => {
                    if code == "busy" {
                        diagnose(lua, "eqoa.windows.takeover", code, &msg);
                    }
                    error(lua, "eqoa.windows.takeover", code, &msg)
                }
            }
        })?,
    )?;
    bind(
        lua,
        "windows",
        "release",
        lua.create_function(|lua, kind: Value| {
            let vm = super::native_events::bus(lua).borrow().vm;
            match kind {
                Value::Nil => {
                    let released = window_takeover::release_all_for_vm(vm);
                    lua.pack_multi((released, Value::Nil))
                }
                Value::String(s) => {
                    let Ok(s) = s.to_str() else {
                        return error(
                            lua,
                            "eqoa.windows.release",
                            "invalid_argument",
                            "kind must be UTF-8",
                        );
                    };
                    let parsed = match TakeoverKind::parse(&s) {
                        Ok(k) => k,
                        Err((code, msg)) => return error(lua, "eqoa.windows.release", code, &msg),
                    };
                    let released = window_takeover::release(vm, parsed);
                    lua.pack_multi((released, Value::Nil))
                }
                _ => error(
                    lua,
                    "eqoa.windows.release",
                    "invalid_argument",
                    "kind must be 'loot', 'merchant', 'bank', or nil",
                ),
            }
        })?,
    )?;
    bind(
        lua,
        "windows",
        "taken",
        lua.create_function(|lua, kind: Value| match kind {
            Value::Nil => {
                let holders = lua.create_table()?;
                for k in TakeoverKind::ALL_KINDS {
                    if let Some(h) = window_takeover::holder_of(k) {
                        holders.set(k.name(), h)?;
                    }
                }
                let tbl = lua.create_table()?;
                tbl.set("loot", window_takeover::is_taken(TakeoverKind::Loot))?;
                tbl.set("merchant", window_takeover::is_taken(TakeoverKind::Merchant))?;
                tbl.set("bank", window_takeover::is_taken(TakeoverKind::Bank))?;
                tbl.set("raid_ready", window_takeover::is_taken(TakeoverKind::RaidReady))?;
                tbl.set("raid_roll", window_takeover::is_taken(TakeoverKind::RaidRoll))?;
                tbl.set("start_menu", window_takeover::is_taken(TakeoverKind::StartMenu))?;
                tbl.set("chat_entry", window_takeover::is_taken(TakeoverKind::ChatEntry))?;
                tbl.set("holders", holders)?;
                lua.pack_multi((tbl, Value::Nil))
            }
            Value::String(s) => {
                let Ok(s) = s.to_str() else {
                    return error(
                        lua,
                        "eqoa.windows.taken",
                        "invalid_argument",
                        "kind must be UTF-8",
                    );
                };
                let parsed = match TakeoverKind::parse(&s) {
                    Ok(k) => k,
                    Err((code, msg)) => return error(lua, "eqoa.windows.taken", code, &msg),
                };
                lua.pack_multi((window_takeover::is_taken(parsed), Value::Nil))
            }
            _ => error(
                lua,
                "eqoa.windows.taken",
                "invalid_argument",
                "kind must be 'loot', 'merchant', 'bank', or nil",
            ),
        })?,
    )?;

    for (path, fields) in [
        ("eqoa.quests.Quest", "ref index title"),
        ("eqoa.camera.Camera", "position heading pitch distance mode"),
        (
            "eqoa.windows.Windows",
            "mode native_mode busy start_menu_open any_window_open active windows",
        ),
        (
            "eqoa.windows.Window",
            "ref kind title visible focused rows selected_row scroll_top tabs tab hints dialog",
        ),
        ("eqoa.windows.MenuRow", "index text enabled item command"),
        ("eqoa.windows.ButtonHint", "button label"),
        (
            "eqoa.windows.Dialog",
            "kind text options selection tag title chat password style style_id prompt_kind",
        ),
        (
            "eqoa.mastery.Mastery",
            "available spent experience_share progress_raw pending stats bonuses",
        ),
        (
            "eqoa.mastery.MasterySkill",
            "id parent_key rank_key name category cost known owned master_class",
        ),
        (
            "eqoa.mastery.MasteryNode",
            "id skill_id parent_key name title category cost text known owned level_required points_spent_required minimum_stats",
        ),
    ] {
        describe(lua, path, "supported", Some(6), "");
        for field in fields.split_whitespace() {
            describe(lua, &format!("{path}.{field}"), "supported", Some(6), "");
        }
    }

    Ok(())
}
