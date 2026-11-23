// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A6: the Start menu and the chat line as windows a mod can hold (`start_menu`, `chat_entry`), the chat
//! reads a chat window needs (`eqoa.chat.tabs`, `eqoa.chat.reply_to`, a line's `kind`), and the three actions
//! (`menu_command` on a session, `set_channel`, `chat_line`). Plain data and Lua bindings only:
//! `fork_ext/a6_play.rs` fills the state from `Play`, opens the sessions and carries the actions out.
//!
//! Chat lines are judged by the line that is sent (spec section 8): `chat_line` takes a line the player could
//! have typed into the game's box, so it needs `eqoa.actions.chat`; text beginning with `/` or `#` is refused here
//! (a command line is `eqoa.actions.command`, and the report routes are never sent). What a result means: all three
//! complete when the client has done what the game's own control does (`sent`, never `confirmed`).
use super::{
    action_broker::{ActionKind, enqueue_lua_with_grant},
    host::Host,
    native::{bind, describe, error, record},
    native_items::resolve_item_handle,
};
use mlua::{Lua, Result, Value};
use serde_json::{Value as Json, json};
use std::cell::RefCell;

const ROUND: u8 = 8;

/// The game's edit box holds a line shorter than this (`open_edit(.., 128, ..)`); a longer one is dropped silently by
/// `chat_line`, so the action refuses it instead.
pub(crate) const LINE_LIMIT: usize = 128;
/// Item links one line carries (`net::itemlink`'s most).
pub(crate) const LINKS_MOST: usize = 4;

/// Where a line goes (`chat_line`'s `channel`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Route {
    Say,
    Group,
    Guild,
    Shout,
    Tell,
    Reply,
}

impl Route {
    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "say" => Self::Say,
            "group" => Self::Group,
            "guild" => Self::Guild,
            "shout" => Self::Shout,
            "tell" => Self::Tell,
            "reply" => Self::Reply,
            _ => return None,
        })
    }

    /// The command word the game's own chat line takes for this route (`Play::chat_line`).
    pub(crate) const fn command(self) -> &'static str {
        match self {
            Self::Say => "/say",
            Self::Group => "/g",
            Self::Guild => "/gu",
            Self::Shout => "/s",
            Self::Tell => "/t",
            Self::Reply => "/r",
        }
    }
}

/// An item link in a `chat_line`: its place in the text (UTF-16 units from 0) and the item (list 0 the pack, 1 the bank).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LinkSpec {
    pub start: usize,
    pub len: usize,
    pub list: u8,
    pub key: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum A6Action {
    /// `eqoa.actions.menu_command(session.ref, command)`: the Start menu entry `command` of the mod's menu, run the
    /// way the game's own menu runs it.
    StartMenuCommand { vm: u64, session: String, command: String },
    /// `eqoa.actions.set_channel(channel)`: where the game's plain text goes (what Community > Chat Modes sets).
    SetChannel { channel: u8 },
    ChatLine { route: Route, to: Option<String>, text: String, links: Vec<LinkSpec> },
    /// `eqoa.actions.open_link(id, name)`: the game's own item card for a link a chat line carried.
    OpenLink { id: u32, name: String },
}

impl A6Action {
    pub(crate) const fn name(&self) -> &'static str {
        match self {
            Self::StartMenuCommand { .. } => "menu_command",
            Self::SetChannel { .. } => "set_channel",
            Self::ChatLine { .. } => "chat_line",
            Self::OpenLink { .. } => "open_link",
        }
    }
}

/// What the client says about chat each step (`a6_play::tick`): the tabs and their kinds as the player set them, who
/// last sent a tell, the channel plain text goes to.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ChatInfo {
    /// `(index, name, kinds)`: tab 0 is Main (always there); another is there while a kind goes to it.
    pub tabs: Vec<(usize, String, Vec<&'static str>)>,
    pub reply_to: String,
}

/// The links the server sent for the next chat line: `(start, len, id)` in UTF-16 units of the line. `fresh` while
/// the event that carries them has not yet been followed by the line's own event.
#[derive(Default)]
struct StashedLinks {
    links: Vec<(u16, u16, u32)>,
    fresh: bool,
}

/// Link ids seen in chat lines lately (the most `KNOWN_MOST`): `open_link` opens only what a line carried.
const KNOWN_MOST: usize = 2048;

thread_local! {
    static CHAT: RefCell<Option<ChatInfo>> = const { RefCell::new(None) };
    static STASH: RefCell<StashedLinks> = RefCell::new(StashedLinks::default());
    static KNOWN: RefCell<std::collections::VecDeque<u32>> = const { RefCell::new(std::collections::VecDeque::new()) };
}

/// A game event carrying item links arrived (`Event::ChatLinks`): they belong to the line the next event makes.
pub(crate) fn stash_links(links: &[(u16, u16, u32)]) {
    STASH.with(|s| *s.borrow_mut() = StashedLinks { links: links.to_vec(), fresh: true });
}

/// Any other game event arrived: links stashed for an earlier event that made no line are dropped.
pub(crate) fn event_boundary() {
    STASH.with(|s| {
        let mut s = s.borrow_mut();
        if s.links.is_empty() {
            return;
        }
        if s.fresh {
            s.fresh = false;
        } else {
            s.links.clear();
        }
    });
}

/// The links of the chat line `text` just added, as record data (places in characters from 1, the item's name and
/// the server's id for it), and remembered as ids `open_link` may open.
pub(crate) fn take_line_links(text: &str) -> Vec<Json> {
    let links = STASH.with(|s| std::mem::take(&mut s.borrow_mut().links));
    if links.is_empty() {
        return Vec::new();
    }
    let wide: Vec<u16> = text.encode_utf16().collect();
    let mut out = Vec::new();
    for (start, len, id) in links {
        let a = (start as usize).min(wide.len());
        let b = (a + len as usize).min(wide.len());
        if b <= a {
            continue;
        }
        let name = String::from_utf16_lossy(&wide[a..b]);
        let before = String::from_utf16_lossy(&wide[..a]).chars().count();
        out.push(json!({"start": before + 1, "len": name.chars().count(), "name": name, "id": id}));
        KNOWN.with(|k| {
            let mut k = k.borrow_mut();
            if !k.contains(&id) {
                if k.len() >= KNOWN_MOST {
                    k.pop_front();
                }
                k.push_back(id);
            }
        });
    }
    out
}

/// Whether a chat line carried a link with this id lately.
pub(crate) fn link_id_known(id: u32) -> bool {
    KNOWN.with(|k| k.borrow().contains(&id))
}

pub(crate) fn set_chat_info(info: ChatInfo) {
    CHAT.with(|c| {
        let mut c = c.borrow_mut();
        if c.as_ref() != Some(&info) {
            *c = Some(info);
        }
    });
}

#[cfg(test)]
pub(crate) fn reset_for_test() {
    CHAT.with(|c| *c.borrow_mut() = None);
    STASH.with(|s| *s.borrow_mut() = StashedLinks::default());
    KNOWN.with(|k| k.borrow_mut().clear());
}

/// The game's channel number for a name (`play.channel`: 0 say, 2 group, 3 shout, 4 guild).
pub(crate) fn channel_code(name: &str) -> Option<u8> {
    Some(match name {
        "say" => 0,
        "group" => 2,
        "shout" => 3,
        "guild" => 4,
        _ => return None,
    })
}

/// The name of the game's channel number; none for a number the client does not use.
pub(crate) fn channel_name(code: u8) -> Option<&'static str> {
    Some(match code {
        0 => "say",
        2 => "group",
        3 => "shout",
        4 => "guild",
        _ => return None,
    })
}

fn str_arg(v: &Value) -> Option<String> {
    v.as_string().and_then(|s| s.to_str().ok().map(|s| s.to_string()))
}

/// A link of a `chat_line`: `{start, len, item}` (an inventory or bank item handle) or `{start, len, list, key}`
/// (what a session's `offer` carries). `start` counts characters of the text from 1, `len` characters.
fn parse_link(text: &str, v: &Value) -> std::result::Result<LinkSpec, (&'static str, String)> {
    let Value::Table(t) = v else {
        return Err(("invalid_argument", "each link must be a table {start, len, item}".into()));
    };
    let whole = |name: &str| -> std::result::Result<i64, (&'static str, String)> {
        match t.get::<Value>(name) {
            Ok(Value::Integer(n)) => Ok(n),
            Ok(Value::Number(n)) if n.is_finite() && n.fract() == 0.0 && n.abs() < 1e12 => Ok(n as i64),
            _ => Err(("invalid_argument", format!("a link's {name} must be a whole number"))),
        }
    };
    let (start, len) = (whole("start")?, whole("len")?);
    let chars = text.chars().count() as i64;
    if start < 1 || len < 1 || start - 1 + len > chars {
        return Err(("out_of_range", format!("a link must lie inside the text (1..{chars})")));
    }
    // UTF-16 units, as the wire counts them
    let before: usize = text.chars().take(start as usize - 1).map(char::len_utf16).sum();
    let inside: usize = text.chars().skip(start as usize - 1).take(len as usize).map(char::len_utf16).sum();
    let (list, key) = match t.get::<Value>("item") {
        Ok(Value::Nil) | Err(_) => {
            let list = match t.get::<Value>("list") {
                Ok(Value::String(s)) => match s.to_str().map(|s| s.to_string()).as_deref() {
                    Ok("inventory") => 0u8,
                    Ok("bank") => 1u8,
                    _ => return Err(("invalid_argument", "a link's list must be 'inventory' or 'bank'".into())),
                },
                _ => return Err(("invalid_argument", "a link needs an item handle, or list and key".into())),
            };
            let key = whole("key")?;
            if !(0..=i64::from(u32::MAX)).contains(&key) {
                return Err(("out_of_range", "a link's key is not an item key".into()));
            }
            (list, key as u32)
        }
        Ok(item) => {
            let spec = resolve_item_handle(&item)?;
            let list = match spec.list {
                "inventory" => 0u8,
                "bank" => 1u8,
                other => return Err(("invalid_argument", format!("a link needs an inventory or bank item, got '{other}'"))),
            };
            (list, spec.inv_index)
        }
    };
    Ok(LinkSpec { start: before, len: inside, list, key })
}

pub(crate) fn install(host: &Host) -> Result<()> {
    let lua = &host.lua;
    for (path, fields) in [
        ("eqoa.chat.ChatTabs", "tabs kinds"),
        ("eqoa.chat.ChatTab", "index name kinds"),
        (
            "eqoa.windows.SessionData",
            "entries text links channel reply_to offer",
        ),
        ("eqoa.windows.StartEntry", "text enabled command native entries"),
        ("eqoa.windows.ChatLink", "start len list key name"),
        ("eqoa.windows.LinkOffer", "seq name list key"),
        ("eqoa.chat.ChatLink", "start len name id"),
    ] {
        describe(lua, path, "supported", Some(ROUND), "");
        for field in fields.split_whitespace() {
            describe(lua, &format!("{path}.{field}"), "supported", Some(ROUND), "");
        }
    }

    bind(
        lua,
        "chat",
        "tabs",
        lua.create_function(|lua, ()| {
            let info = CHAT.with(|c| c.borrow().clone());
            let Some(info) = info else {
                return error(lua, "eqoa.chat.tabs", "not_in_world", "enter a loaded world first");
            };
            let tabs: Vec<Json> = info
                .tabs
                .iter()
                .map(|(index, name, kinds)| json!({"index": index, "name": name, "kinds": kinds}))
                .collect();
            let kinds: Vec<&str> = crate::chat::Kind::ALL.iter().map(|k| k.key()).collect();
            lua.pack_multi((record(lua, "eqoa.chat.ChatTabs", &json!({"tabs": tabs, "kinds": kinds}))?, Value::Nil))
        })?,
    )?;
    bind(
        lua,
        "chat",
        "reply_to",
        lua.create_function(|lua, ()| {
            let info = CHAT.with(|c| c.borrow().clone());
            let Some(info) = info else {
                return error(lua, "eqoa.chat.reply_to", "not_in_world", "enter a loaded world first");
            };
            if info.reply_to.is_empty() {
                return lua.pack_multi((Value::Nil, Value::Nil));
            }
            lua.pack_multi((info.reply_to, Value::Nil))
        })?,
    )?;

    bind(
        lua,
        "actions",
        "set_channel",
        lua.create_function(|lua, channel: Value| {
            enqueue_lua_with_grant(lua, "eqoa.actions.set_channel", "eqoa.actions.set_channel", || {
                let Some(name) = str_arg(&channel) else {
                    return Err(("invalid_argument", "channel must be 'say', 'group', 'shout' or 'guild'".into()));
                };
                let Some(code) = channel_code(&name) else {
                    return Err(("invalid_argument", "channel must be 'say', 'group', 'shout' or 'guild'".into()));
                };
                Ok(ActionKind::A6(A6Action::SetChannel { channel: code }))
            })
        })?,
    )?;
    bind(
        lua,
        "actions",
        "open_link",
        lua.create_function(|lua, (id, name): (Value, Value)| {
            enqueue_lua_with_grant(lua, "eqoa.actions.open_link", "eqoa.actions.open_link", || {
                let id = match id {
                    Value::Integer(n) if (0..=i64::from(u32::MAX)).contains(&n) => n as u32,
                    Value::Number(n) if n.fract() == 0.0 && (0.0..=f64::from(u32::MAX)).contains(&n) => n as u32,
                    _ => return Err(("invalid_argument", "id must be the id of a link in a chat line".into())),
                };
                let name = str_arg(&name).unwrap_or_default();
                if name.is_empty() || name.chars().count() > 64 {
                    return Err(("invalid_argument", "name must be the linked item's name".into()));
                }
                if !link_id_known(id) {
                    return Err(("not_found", "no recent chat line carried a link with this id".into()));
                }
                Ok(ActionKind::A6(A6Action::OpenLink { id, name }))
            })
        })?,
    )?;
    bind(
        lua,
        "actions",
        "chat_line",
        lua.create_function(|lua, (channel, text, opts): (Value, Value, Option<Value>)| {
            const SYMBOL: &str = "eqoa.actions.chat_line";
            // judged by the line the player could have typed: an ordinary line (spec section 8, rule 4)
            enqueue_lua_with_grant(lua, SYMBOL, "eqoa.actions.chat", || {
                let route = str_arg(&channel).as_deref().and_then(Route::parse).ok_or((
                    "invalid_argument",
                    "channel must be 'say', 'group', 'guild', 'shout', 'tell' or 'reply'".to_string(),
                ))?;
                let text = str_arg(&text).ok_or(("invalid_argument", "text must be a string".to_string()))?;
                let text = text.trim().to_string();
                if text.is_empty() {
                    return Err(("invalid_argument", "message text is empty".into()));
                }
                if text.contains(['\0', '\r', '\n']) {
                    return Err(("invalid_argument", "message cannot contain control newlines or NUL".into()));
                }
                if text.starts_with('/') || text.starts_with('#') {
                    return Err((
                        "invalid_argument",
                        "a line that starts with / or # is a command: send it with eqoa.actions.command".into(),
                    ));
                }
                let (mut to, mut links) = (None, Vec::new());
                match opts {
                    None | Some(Value::Nil) => {}
                    Some(Value::Table(t)) => {
                        match t.get::<Value>("to").map_err(|_| ("invalid_argument", "options.to".to_string()))? {
                            Value::Nil => {}
                            v => {
                                to = Some(
                                    str_arg(&v)
                                        .ok_or(("invalid_argument", "options.to must be a name".to_string()))?,
                                )
                            }
                        }
                        match t.get::<Value>("links").map_err(|_| ("invalid_argument", "options.links".to_string()))? {
                            Value::Nil => {}
                            Value::Table(list) => {
                                for v in list.sequence_values::<Value>() {
                                    let v = v.map_err(|_| ("invalid_argument", "options.links must be a list".to_string()))?;
                                    links.push(parse_link(&text, &v)?);
                                }
                            }
                            _ => return Err(("invalid_argument", "options.links must be a list".into())),
                        }
                    }
                    Some(_) => return Err(("invalid_argument", "options must be a table or nil".into())),
                }
                if links.len() > LINKS_MOST {
                    return Err(("out_of_range", format!("a line carries at most {LINKS_MOST} item links")));
                }
                match (route, &to) {
                    (Route::Tell, None) => return Err(("invalid_argument", "a tell needs options.to, the player's name".into())),
                    (Route::Tell, Some(name)) => {
                        if name.is_empty()
                            || name.chars().any(|c| c.is_whitespace() || c.is_control() || c == '/')
                        {
                            return Err(("invalid_argument", "options.to must be one player's name".into()));
                        }
                    }
                    (_, Some(_)) => return Err(("invalid_argument", "options.to is only for a tell".into())),
                    _ => {}
                }
                let prefix = match (&route, &to) {
                    (Route::Tell, Some(name)) => format!("{} {name} ", route.command()),
                    _ => format!("{} ", route.command()),
                };
                let whole = format!("{prefix}{text}");
                if super::gesture::is_forbidden_report_text(&text) || super::gesture::is_forbidden_report_text(&whole) {
                    return Err((
                        "permission_denied",
                        "synthetic report routes (/bug, #bug, /feature, #feature) are forbidden".into(),
                    ));
                }
                if whole.encode_utf16().count() >= LINE_LIMIT {
                    return Err((
                        "out_of_range",
                        format!("the line, with its command, must be shorter than {LINE_LIMIT} characters"),
                    ));
                }
                Ok(ActionKind::A6(A6Action::ChatLine { route, to, text, links }))
            })
        })?,
    )?;
    Ok(())
}
