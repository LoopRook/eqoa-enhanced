// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round 6 window and quest actions (`eqoa.actions.drop_quest`, `mastery_*`, `close_window`,
//! `select_row`, `activate_row`, `dialog_answer`, `input_text`, `menu_command`): argument
//! parsing, handle capture and the queue-time report guard. The native execution is in the
//! private child adapter (`fork_ext/play.rs`).
use super::{
    action_broker::{ActionKind, WinTarget, enqueue_lua_with},
    gesture::is_forbidden_report_text,
    native::{bind, require_capability},
    native_core::CORE,
    native_r6a::resolve_in,
    native_state::Ref,
};
use mlua::{Lua, Result, Value};
use serde_json::{Value as Json, json};
use std::hash::{Hash, Hasher};

type Fail = (&'static str, String);

/// What identifies the rows a handle was taken from: kind, row text/enabled/command, tabs and
/// dialog content (not the selection, scroll or focus, which a player moves freely). A page
/// change alters it, so the old view is stale at dispatch.
pub(crate) fn signature(w: &Json) -> u64 {
    let rows: Vec<Json> = w["rows"]
        .as_array()
        .map(|r| {
            r.iter()
                .map(|x| json!([x["index"], x["text"], x["enabled"], x["command"]]))
                .collect()
        })
        .unwrap_or_default();
    let dialog_text = if w["dialog"]["kind"] == "input" {
        Json::Null
    } else {
        w["dialog"]["text"].clone()
    };
    // what an edit box or confirm is for: a replaced box is another window
    let d = &w["dialog"];
    let purpose = json!([
        d["tag"],
        d["title"],
        d["chat"],
        d["password"],
        d["style"],
        d["style_id"],
        d["prompt_kind"]
    ]);
    let s = json!([
        w["kind"],
        rows,
        w["tabs"],
        w["tab"],
        w["dialog"]["kind"],
        dialog_text,
        w["dialog"]["options"],
        purpose
    ])
    .to_string();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

/// `startmenu` commands for the support/report windows (`0x0B0B_1000..`).
fn report_command(command: &str) -> bool {
    command
        .split(':')
        .next()
        .and_then(|m| u32::from_str_radix(m.trim_start_matches("0x"), 16).ok())
        .is_some_and(|msg| msg & 0xFFFF_F000 == 0x0B0B_1000)
}

/// The synthetic report guard for window/dialog paths: a report form or its confirmation never
/// takes synthetic input. `text` is what an input would type.
pub(crate) fn report_guard(w: &Json, command: Option<&str>, text: Option<&str>) -> Option<&'static str> {
    if matches!(w["kind"].as_str(), Some("support_request" | "support_inbox")) {
        return Some("the support/report window does not take synthetic input");
    }
    if w["dialog"]["tag"].as_u64() == Some(0x0B0B_1002) {
        return Some("the report editor does not take synthetic input");
    }
    if command.is_some_and(report_command) {
        return Some("report routes are forbidden");
    }
    if let Some(t) = text
        && (is_forbidden_report_text(t) || is_forbidden_report_text(&format!("/t x {t}")))
    {
        return Some("synthetic report routes (/bug, #bug, /feature, #feature) are forbidden");
    }
    None
}

fn whole(v: &Value, what: &str, min: i64, max: i64) -> std::result::Result<i64, Fail> {
    let n = match v {
        Value::Integer(n) => *n,
        Value::Number(n) if n.is_finite() && n.fract() == 0.0 && n.abs() < 1e15 => *n as i64,
        _ => return Err(("invalid_argument", format!("{what} must be a whole number"))),
    };
    if !(min..=max).contains(&n) {
        return Err(("out_of_range", format!("{what} must be {min}..={max}")));
    }
    Ok(n)
}

/// Resolve a `WindowRef` to its current view entry and the handle's captured identity.
fn window(arg: &Value) -> std::result::Result<(WinTarget, Json), Fail> {
    let Some(ud) = arg.as_userdata() else {
        return Err(("invalid_argument", "expected a WindowRef".into()));
    };
    let r = ud
        .borrow::<Ref>()
        .map_err(|_| ("invalid_argument", "expected a WindowRef".to_string()))?;
    if r.kind != "window" {
        return Err(("invalid_argument", format!("expected a WindowRef, got {}", r.kind)));
    }
    let entry = CORE.with(|c| {
        let c = c.borrow();
        let list = c.view.windows_state["windows"].as_array()?;
        resolve_in(arg, list, c.generation, "window")
    });
    let Some(w) = entry else {
        return Err(("stale_handle", "window handle is stale or invalid".into()));
    };
    let target = WinTarget {
        key: r.key.clone(),
        incarnation: r.incarnation,
        sig: signature(&w),
    };
    Ok((target, w))
}

fn guard(w: &Json, command: Option<&str>, text: Option<&str>) -> std::result::Result<(), Fail> {
    match report_guard(w, command, text) {
        Some(why) => Err(("permission_denied", why.into())),
        None => Ok(()),
    }
}

fn row_count(w: &Json) -> i64 {
    w["rows"].as_array().map_or(0, |r| r.len() as i64)
}

/// The actions that can answer an NPC dialog: `dialog_answer` picks one of its options, and `activate_row`
/// walks to a row of any live window and presses confirm (a response dialog's options are its rows).
/// `select_row` only moves the cursor, and `menu_command` presses only rows that carry a command, which
/// dialog rows never do. Mods & Addons labels a mod "Can answer NPC dialogs" from this list.
pub(crate) const ANSWERS_DIALOGS: [&str; 2] = ["eqoa.actions.dialog_answer", "eqoa.actions.activate_row"];

pub(crate) fn install(lua: &Lua) -> Result<()> {
    bind(
        lua,
        "actions",
        "drop_quest",
        lua.create_function(|lua, arg: Value| {
            enqueue_lua_with(lua, "eqoa.actions.drop_quest", || {
                let Some(ud) = arg.as_userdata() else {
                    return Err(("invalid_argument", "expected a QuestRef".into()));
                };
                let r = ud
                    .borrow::<Ref>()
                    .map_err(|_| ("invalid_argument", "expected a QuestRef".to_string()))?;
                if r.kind != "quest" {
                    return Err(("invalid_argument", format!("expected a QuestRef, got {}", r.kind)));
                }
                let q = CORE
                    .with(|c| {
                        let c = c.borrow();
                        resolve_in(&arg, &c.view.quests, c.generation, "quest")
                    })
                    .ok_or(("stale_handle", "quest handle is stale or invalid".to_string()))?;
                Ok(ActionKind::DropQuest {
                    index: q["index"].as_u64().unwrap_or(0) as usize,
                    title: q["title"].as_str().unwrap_or("").to_string(),
                    revision: r.incarnation,
                    stamp: CORE.with(|c| c.borrow().quest_stamp),
                })
            })
        })?,
    )?;
    bind(
        lua,
        "actions",
        "mastery_share",
        lua.create_function(|lua, fraction: Value| {
            enqueue_lua_with(lua, "eqoa.actions.mastery_share", || {
                let f = match fraction {
                    Value::Integer(n) => n as f64,
                    Value::Number(n) => n,
                    _ => return Err(("invalid_argument", "fraction must be a number".into())),
                };
                if !f.is_finite() || !(0.0..=1.0).contains(&f) {
                    return Err(("out_of_range", "fraction must be 0..1".into()));
                }
                let pct = (f * 100.0).round();
                if (f * 100.0 - pct).abs() > 1e-6 {
                    return Err(("invalid_argument", "fraction must be in exact 0.01 steps".into()));
                }
                Ok(ActionKind::MasteryShare { percent: pct as i32 })
            })
        })?,
    )?;
    for (name, which) in [("mastery_browse", 0u8), ("mastery_buy", 1), ("mastery_refund", 2)] {
        bind(
            lua,
            "actions",
            name,
            lua.create_function(move |lua, id: Value| {
                enqueue_lua_with(lua, &format!("eqoa.actions.{name}"), || {
                    let skill_id = whole(&id, "skill_id", 0, i32::MAX as i64)? as i32;
                    Ok(match which {
                        0 => ActionKind::MasteryBrowse { skill_id },
                        1 => ActionKind::MasteryBuy { skill_id },
                        _ => ActionKind::MasteryRefund { skill_id },
                    })
                })
            })?,
        )?;
    }
    bind(
        lua,
        "actions",
        "close_window",
        lua.create_function(|lua, r: Value| {
            enqueue_lua_with(lua, "eqoa.actions.close_window", || {
                let (win, w) = window(&r)?;
                guard(&w, None, None)?;
                Ok(ActionKind::CloseWindow { win })
            })
        })?,
    )?;
    for (name, activate) in [("select_row", false), ("activate_row", true)] {
        bind(
            lua,
            "actions",
            name,
            lua.create_function(move |lua, (r, row): (Value, Value)| {
                enqueue_lua_with(lua, &format!("eqoa.actions.{name}"), || {
                    let (win, w) = window(&r)?;
                    let n = row_count(&w);
                    if n == 0 {
                        return Err(("unsupported", "window has no selectable rows".into()));
                    }
                    let row = whole(&row, "row", 1, n)? as usize;
                    let cmd = w["rows"][row - 1]["command"].as_str().map(str::to_string);
                    guard(&w, cmd.as_deref(), None)?;
                    Ok(if activate {
                        ActionKind::ActivateRow { win, row }
                    } else {
                        ActionKind::SelectRow { win, row }
                    })
                })
            })?,
        )?;
    }
    bind(
        lua,
        "actions",
        "dialog_answer",
        lua.create_function(|lua, (r, option): (Value, Value)| {
            enqueue_lua_with(lua, "eqoa.actions.dialog_answer", || {
                let (win, w) = window(&r)?;
                guard(&w, None, None)?;
                if w["dialog"]["kind"].is_null() || w["dialog"]["kind"] == "input" {
                    return Err(("unsupported", "window is not a text response or confirm dialog".into()));
                }
                let n = match w["dialog"]["kind"].as_str() {
                    Some("confirm") => match w["dialog"]["style"].as_str() {
                        Some("yes_no") => 2,
                        Some("ok" | "cancel") => 1,
                        _ => return Err(("unsupported", "unknown confirm style".into())),
                    },
                    // a Done-only text dialog (PrivChat) has no options; 1 is its Done
                    _ => match w["dialog"]["options"].as_array().map_or(0, |o| o.len() as i64) {
                        0 if w["kind"] == "text_dialog" => 1,
                        n => n,
                    },
                };
                if n == 0 {
                    return Err(("out_of_range", "dialog has no options".into()));
                }
                let option = whole(&option, "option", 1, n)? as usize;
                Ok(ActionKind::DialogAnswer { win, option })
            })
        })?,
    )?;
    bind(
        lua,
        "actions",
        "input_text",
        lua.create_function(|lua, (r, text, submit): (Value, String, Option<bool>)| {
            enqueue_lua_with(lua, "eqoa.actions.input_text", || {
                let (win, w) = window(&r)?;
                if w["dialog"]["kind"] != "input" {
                    return Err(("unsupported", "window has no native edit field".into()));
                }
                if w["dialog"]["password"].as_bool() != Some(false) {
                    return Err(("permission_denied", "password fields never take synthetic input".into()));
                }
                guard(&w, None, Some(&text))?;
                if text.contains('\0') || text.chars().count() > 512 {
                    return Err(("out_of_range", "text must be at most 512 characters without NUL".into()));
                }
                let submit = submit.unwrap_or(false);
                // A chat editor's line needs the grant for what it is, submitted now or left for the player to
                // send; dispatch judges again from what the game will actually send then (a Who-list Tell).
                if w["dialog"]["chat"] == true && !text.is_empty() {
                    require_capability(lua, "eqoa.actions.input_text", super::chat::line_capability(&text))?;
                }
                let grants = super::action_broker::LineGrants {
                    chat: require_capability(lua, "eqoa.actions.input_text", "eqoa.actions.chat").is_ok(),
                    command: require_capability(lua, "eqoa.actions.input_text", "eqoa.actions.command").is_ok(),
                };
                Ok(ActionKind::InputText {
                    win,
                    text,
                    submit,
                    grants,
                })
            })
        })?,
    )?;
    bind(
        lua,
        "actions",
        "menu_command",
        lua.create_function(|lua, (r, command, arg): (Value, String, Option<Value>)| {
            enqueue_lua_with(lua, "eqoa.actions.menu_command", || {
                let (win, w) = window(&r)?;
                guard(&w, Some(&command), None)?;
                let arg = match arg {
                    None | Some(Value::Nil) => 0,
                    Some(v) => whole(&v, "arg", i32::MIN as i64, i32::MAX as i64)?,
                };
                if arg != 0 {
                    return Err(("unsupported", "menu commands take no argument in this client".into()));
                }
                let found = w["rows"].as_array().is_some_and(|rows| {
                    rows.iter()
                        .any(|x| x["command"].as_str() == Some(command.as_str()) && x["enabled"] == true)
                });
                if !found {
                    return Err((
                        "unsupported",
                        format!("'{command}' is not an enabled command of this window"),
                    ));
                }
                Ok(ActionKind::MenuCommand { win, command })
            })
        })?,
    )?;
    Ok(())
}
