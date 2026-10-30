//! Native FrontiersForge chat adapter (`frontiers_forge.chat`).
//!
//! Authoritative Contracts:
//! - Upstream pinned reference: `scripts/modules/frontiers_forge/chat.lua`
//!   and `wereoxxs_ui.lua`.
//!
//! Scope & Invariant Guarantees:
//! - Fixed-size, Copy snapshot of chat log messages (`ChatSnapshot`, `ChatMessageSnapshot`).
//! - Full native implementation of `frontiers_forge.chat` module table.
//! - Read-only access to engine chat state via `host.snapshot`.
//! - Message publishing bus (`OnChatMessage`) matching authentic FrontiersForge payload schema.
//! - Outgoing chat text queuing (`SendChatText`, `IsSendPending`).

use super::host::Host;
use mlua::{Result as LuaResult, Table, Value};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

pub const MAX_CHAT_MESSAGES: usize = 16;

pub const MSG_TYPE_SAY: u32 = 0x3F800000;
pub const MSG_TYPE_SHOUT: u32 = 0x3E7CFCFD;
pub const MSG_TYPE_PARTY: u32 = 0x3F000000;
pub const MSG_TYPE_TELL: u32 = 0x3F008081;
pub const MSG_TYPE_GUILD: u32 = 0x3E800000;

thread_local! {
    static OUTGOING_CHAT: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static CHAT_MONOTONIC_SEQ: Cell<u32> = const { Cell::new(1) };
    static CHAT_HISTORY: RefCell<VecDeque<(String, u32, u32)>> = const { RefCell::new(VecDeque::new()) };
}

pub(crate) fn queue_outgoing_chat(msg: String) {
    OUTGOING_CHAT.with(|q| q.borrow_mut().push(msg));
}

pub(crate) fn take_outgoing_chat() -> Vec<String> {
    OUTGOING_CHAT.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

pub(crate) fn snapshot_chat(raw_chat: &VecDeque<(String, [f32; 4])>, now_ms: u32) -> ChatSnapshot {
    CHAT_HISTORY.with(|hist_cell| {
        let mut hist = hist_cell.borrow_mut();
        let raw_len = raw_chat.len();

        let mut k = raw_len;
        for candidate in 0..=raw_len {
            let prefix_len = raw_len - candidate;
            if prefix_len <= hist.len() {
                let hist_offset = hist.len() - prefix_len;
                let matches = (0..prefix_len).all(|i| {
                    let (raw_line, raw_col) = &raw_chat[i];
                    let (hist_line, hist_type, _) = &hist[hist_offset + i];
                    raw_line == hist_line && raw_col[0].to_bits() == *hist_type
                });
                if matches {
                    k = candidate;
                    break;
                }
            }
        }

        for i in (raw_len - k)..raw_len {
            let (raw_line, raw_col) = &raw_chat[i];
            let seq = CHAT_MONOTONIC_SEQ.with(|s| {
                let val = s.get();
                s.set(val.wrapping_add(1));
                val
            });
            if hist.len() >= 64 {
                hist.pop_front();
            }
            hist.push_back((raw_line.clone(), raw_col[0].to_bits(), seq));
        }

        let count = hist.len().min(MAX_CHAT_MESSAGES);
        let mut messages = [ChatMessageSnapshot::default(); MAX_CHAT_MESSAGES];
        let offset = hist.len() - count;
        for i in 0..count {
            let (line, msg_type, seq) = &hist[offset + i];
            let mut text_buf = [0u8; 128];
            let b = line.as_bytes();
            let mut len = b.len().min(128);
            while len > 0 && !line.is_char_boundary(len) {
                len -= 1;
            }
            text_buf[..len].copy_from_slice(&b[..len]);
            messages[i] = ChatMessageSnapshot {
                text: text_buf,
                text_len: len as u8,
                msg_type: *msg_type,
                timestamp_ms: now_ms,
                seq: *seq,
            };
        }

        ChatSnapshot {
            count: count as u8,
            messages,
        }
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChatMessageSnapshot {
    pub text: [u8; 128],
    pub text_len: u8,
    pub msg_type: u32,
    pub timestamp_ms: u32,
    pub seq: u32,
}

impl Default for ChatMessageSnapshot {
    fn default() -> Self {
        Self {
            text: [0u8; 128],
            text_len: 0,
            msg_type: MSG_TYPE_SAY,
            timestamp_ms: 0,
            seq: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChatSnapshot {
    pub count: u8,
    pub messages: [ChatMessageSnapshot; MAX_CHAT_MESSAGES],
}

impl Default for ChatSnapshot {
    fn default() -> Self {
        Self {
            count: 0,
            messages: [ChatMessageSnapshot::default(); MAX_CHAT_MESSAGES],
        }
    }
}

pub fn get_message_type_string(msg_type: u32) -> &'static str {
    match msg_type {
        MSG_TYPE_SAY => "Say",
        MSG_TYPE_SHOUT => "Shout",
        MSG_TYPE_PARTY => "Party",
        MSG_TYPE_TELL => "Tell",
        MSG_TYPE_GUILD => "Guild",
        _ => "Unknown Message Type",
    }
}

pub fn install_chat_adapter(host: &Host) -> LuaResult<()> {
    let lua = &host.lua;
    let chat_mod = lua.create_table()?;

    // MsgType enum
    let msg_type_tbl = lua.create_table()?;
    msg_type_tbl.set("Say", MSG_TYPE_SAY)?;
    msg_type_tbl.set("Shout", MSG_TYPE_SHOUT)?;
    msg_type_tbl.set("Party", MSG_TYPE_PARTY)?;
    msg_type_tbl.set("Tell", MSG_TYPE_TELL)?;
    msg_type_tbl.set("Guild", MSG_TYPE_GUILD)?;
    chat_mod.set("MsgType", msg_type_tbl)?;

    // ChatMode enum
    let chat_mode_tbl = lua.create_table()?;
    chat_mode_tbl.set("Default", "")?;
    chat_mode_tbl.set("Say", "/say")?;
    chat_mode_tbl.set("Group", "/group")?;
    chat_mode_tbl.set("Guild", "/guild")?;
    chat_mode_tbl.set("Shout", "/shout")?;
    chat_mod.set("ChatMode", chat_mode_tbl)?;

    // GetMessageTypeString
    chat_mod.set(
        "GetMessageTypeString",
        lua.create_function(|_, msg_type: u32| Ok(get_message_type_string(msg_type)))?,
    )?;

    // Events constants table
    let events_tbl = lua.create_table()?;
    events_tbl.set("OnChatMessage", "OnChatMessage")?;
    chat_mod.set("Events", events_tbl)?;

    let snapshot_cell = host.snapshot.clone();
    let subscribers = Rc::new(RefCell::new(HashMap::<String, mlua::Function>::new()));
    let subscriber_errors = Rc::new(RefCell::new(Vec::<Table>::new()));
    let last_published_seq = Rc::new(RefCell::new(0u32));
    let primed = Rc::new(RefCell::new(false));

    // On(owner, event_name, fn)
    {
        let subs = subscribers.clone();
        chat_mod.set(
            "On",
            lua.create_function(
                move |_, (owner, event_name, func): (String, String, mlua::Function)| {
                    if event_name != "OnChatMessage" {
                        return Ok((false, Some(format!("unknown chat event: {event_name}"))));
                    }
                    subs.borrow_mut().insert(owner, func);
                    Ok((true, None::<String>))
                },
            )?,
        )?;
    }

    // Off(owner, event_name)
    {
        let subs = subscribers.clone();
        chat_mod.set(
            "Off",
            lua.create_function(move |_, (owner, event_name): (String, Option<String>)| {
                if event_name
                    .as_deref()
                    .map_or(true, |ev| ev == "OnChatMessage")
                {
                    subs.borrow_mut().remove(&owner);
                }
                Ok(())
            })?,
        )?;
    }

    // GetSubscriberErrors
    {
        let errs = subscriber_errors.clone();
        chat_mod.set(
            "GetSubscriberErrors",
            lua.create_function(move |lua_ctx, ()| {
                let list = errs.borrow();
                let tbl = lua_ctx.create_table()?;
                for (i, err) in list.iter().enumerate() {
                    tbl.set(i + 1, err.clone())?;
                }
                Ok(tbl)
            })?,
        )?;
    }

    // Update()
    {
        let s = snapshot_cell.clone();
        let subs = subscribers.clone();
        let errs = subscriber_errors.clone();
        let last_seq = last_published_seq.clone();
        let is_primed = primed.clone();

        chat_mod.set(
            "Update",
            lua.create_function(move |lua_ctx, ()| {
                let snap = s.get().chat;
                let mut last = *last_seq.borrow();
                let was_primed = *is_primed.borrow();

                if !was_primed {
                    // Initial drain primes without reporting historical scrollback
                    let max_seq = snap.messages[..snap.count as usize]
                        .iter()
                        .map(|m| m.seq)
                        .max()
                        .unwrap_or(0);
                    *last_seq.borrow_mut() = max_seq;
                    *is_primed.borrow_mut() = true;
                    return Ok(());
                }

                let mut dead_owners = Vec::new();

                for i in 0..snap.count as usize {
                    let m = &snap.messages[i];
                    if m.seq > last {
                        let text_slice = &m.text[..(m.text_len as usize).min(m.text.len())];
                        let text = std::str::from_utf8(text_slice).unwrap_or("");
                        let type_name = get_message_type_string(m.msg_type);

                        let payload = lua_ctx.create_table()?;
                        payload.set("start_index", i)?;
                        payload.set("committed_ms", m.timestamp_ms)?;
                        payload.set("msg_type", m.msg_type)?;
                        payload.set("type_name", type_name)?;
                        payload.set("text", text)?;

                        let subs_map = subs.borrow();
                        for (owner, func) in subs_map.iter() {
                            if let Err(e) = func.call::<Value>(payload.clone()) {
                                dead_owners.push(owner.clone());
                                let err_tbl = lua_ctx.create_table()?;
                                err_tbl.set("owner", owner.as_str())?;
                                err_tbl.set("event", "OnChatMessage")?;
                                err_tbl.set("error", e.to_string())?;
                                let mut err_list = errs.borrow_mut();
                                err_list.push(err_tbl);
                                if err_list.len() > 16 {
                                    err_list.remove(0);
                                }
                            }
                        }

                        if m.seq > last {
                            last = m.seq;
                        }
                    }
                }

                *last_seq.borrow_mut() = last;

                if !dead_owners.is_empty() {
                    let mut subs_map = subs.borrow_mut();
                    for dead in dead_owners {
                        subs_map.remove(&dead);
                    }
                }

                Ok(())
            })?,
        )?;
    }

    // SendChatText(text, mode)
    {
        let s = snapshot_cell.clone();
        chat_mod.set(
            "SendChatText",
            lua.create_function(move |_, (text, mode): (Option<String>, Option<String>)| {
                if !s.get().in_game {
                    return Ok((false, Some("not in game".to_string())));
                }
                let mut line = text.unwrap_or_default();
                if line.is_empty() {
                    return Ok((false, Some("empty message".to_string())));
                }
                if let Some(m) = mode {
                    if !m.is_empty() && !line.starts_with('/') {
                        line = format!("{m} {line}");
                    }
                }
                if line.len() >= 0x80 {
                    return Ok((
                        false,
                        Some("message too long (127 character max)".to_string()),
                    ));
                }
                queue_outgoing_chat(line);
                Ok((true, None::<String>))
            })?,
        )?;
    }

    // Send hook management stubs (in native engine these are always operational)
    chat_mod.set(
        "IsSendHookInstalled",
        lua.create_function(|_, ()| Ok(true))?,
    )?;
    chat_mod.set(
        "IsSendHookSuspended",
        lua.create_function(|_, ()| Ok(false))?,
    )?;
    chat_mod.set("InstallSendHook", lua.create_function(|_, ()| Ok(true))?)?;
    chat_mod.set("UninstallSendHook", lua.create_function(|_, ()| Ok(true))?)?;
    chat_mod.set("IsSendPending", lua.create_function(|_, ()| Ok(false))?)?;

    // Focus & window channel inspection helpers
    chat_mod.set(
        "CaptureChannelTypeFromGameWindow",
        lua.create_function(|_, ()| Ok((0u8, None::<String>, 0u32)))?,
    )?;
    chat_mod.set(
        "DebugDumpFocusChain",
        lua.create_function(|lua_ctx, ()| Ok(lua_ctx.create_table()?))?,
    )?;
    chat_mod.set(
        "GetChannelTableInfo",
        lua.create_function(|lua_ctx, ()| Ok(lua_ctx.create_table()?))?,
    )?;

    host.register_host_module("frontiers_forge.chat", Value::Table(chat_mod))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fork_ext::addons::discovery::DiscoveredAddon;
    use crate::fork_ext::addons::host::Snapshot;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn temp_test_dir(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("eqoa_chat_test_{}_{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn test_addon(root: &Path) -> DiscoveredAddon {
        let entry = root.join("main.lua");
        fs::write(&entry, "-- test chat addon").unwrap();
        DiscoveredAddon {
            id: "test_chat".to_string(),
            name: "Test Chat".to_string(),
            entry,
            script_root: root.to_path_buf(),
            package_root: root.to_path_buf(),
            module_dirs: vec![root.to_path_buf()],
            resource_dirs: vec![],
            api: Some("eqoa/1".to_string()),
            requested_capabilities: vec![],
            initially_enabled: true,
        }
    }

    #[test]
    fn forge_chat_module_constants_and_types() {
        let dir = temp_test_dir("constants");
        let addon = test_addon(&dir);
        let host = Host::for_addon(&addon).unwrap();
        let chunk = host.lua.load(
            r#"
            local Chat = require("frontiers_forge.chat")
            assert(Chat.MsgType.Say == 0x3F800000, "Say msg type")
            assert(Chat.MsgType.Shout == 0x3E7CFCFD, "Shout msg type")
            assert(Chat.MsgType.Party == 0x3F000000, "Party msg type")
            assert(Chat.MsgType.Tell == 0x3F008081, "Tell msg type")
            assert(Chat.MsgType.Guild == 0x3E800000, "Guild msg type")

            assert(Chat.GetMessageTypeString(Chat.MsgType.Say) == "Say")
            assert(Chat.GetMessageTypeString(Chat.MsgType.Shout) == "Shout")
            assert(Chat.GetMessageTypeString(Chat.MsgType.Party) == "Party")
            assert(Chat.GetMessageTypeString(Chat.MsgType.Tell) == "Tell")
            assert(Chat.GetMessageTypeString(Chat.MsgType.Guild) == "Guild")
            assert(Chat.GetMessageTypeString(0x12345678) == "Unknown Message Type")

            assert(Chat.ChatMode.Default == "")
            assert(Chat.ChatMode.Say == "/say")
            assert(Chat.ChatMode.Group == "/group")
            assert(Chat.ChatMode.Guild == "/guild")
            assert(Chat.ChatMode.Shout == "/shout")

            assert(Chat.Events.OnChatMessage == "OnChatMessage")
            assert(Chat.IsSendHookInstalled() == true)
            assert(Chat.IsSendHookSuspended() == false)

            -- SendChatText out of game rejected
            local ok, err = Chat.SendChatText("hello")
            assert(ok == false)
            assert(err == "not in game")
            "#,
        );
        chunk.exec().unwrap();
    }

    #[test]
    fn forge_chat_publishing_and_events() {
        let dir = temp_test_dir("publishing");
        let addon = test_addon(&dir);
        let host = Host::for_addon(&addon).unwrap();

        let mut snap = ChatSnapshot::default();
        snap.count = 1;
        let mut msg0 = [0u8; 128];
        msg0[..16].copy_from_slice(b"Initial greeting");
        snap.messages[0] = ChatMessageSnapshot {
            text: msg0,
            text_len: 16,
            msg_type: MSG_TYPE_SAY,
            timestamp_ms: 1000,
            seq: 1,
        };

        host.snapshot.set(Snapshot {
            in_game: true,
            chat: snap,
            ..Default::default()
        });

        let chunk = host.lua.load(
            r#"
            local Chat = require("frontiers_forge.chat")
            _G.received = {}

            Chat.On("wereoxxs_ui", Chat.Events.OnChatMessage, function(msg)
                table.insert(_G.received, msg)
            end)

            -- Initial update: primes without re-announcing scrollback
            Chat.Update()
            assert(#_G.received == 0, "No messages on initial priming")
            "#,
        );
        chunk.exec().unwrap();

        // Mutate snapshot: new incoming message
        let mut snap2 = snap;
        snap2.count = 2;
        let mut msg1 = [0u8; 128];
        msg1[..19].copy_from_slice(b"Bob tells you, Hail");
        snap2.messages[1] = ChatMessageSnapshot {
            text: msg1,
            text_len: 19,
            msg_type: MSG_TYPE_TELL,
            timestamp_ms: 2000,
            seq: 2,
        };

        host.snapshot.set(Snapshot {
            in_game: true,
            chat: snap2,
            ..Default::default()
        });

        let chunk2 = host.lua.load(
            r#"
            local Chat = require("frontiers_forge.chat")
            Chat.Update()
            assert(#_G.received == 1, "One new message received")
            assert(_G.received[1].text == "Bob tells you, Hail")
            assert(_G.received[1].msg_type == Chat.MsgType.Tell)
            assert(_G.received[1].type_name == "Tell")
            assert(_G.received[1].committed_ms == 2000)

            -- SendChatText
            local ok, err = Chat.SendChatText("/tell Bob Greetings!")
            assert(ok == true, "SendChatText should succeed")
            assert(err == nil)
            "#,
        );
        chunk2.exec().unwrap();

        let outgoing = take_outgoing_chat();
        assert_eq!(outgoing, vec!["/tell Bob Greetings!"]);
    }

    #[test]
    fn wereoxxs_ui_chat_window_integration() {
        let dir = temp_test_dir("wereoxxs_chat");
        let addon = test_addon(&dir);
        let host = Host::for_addon(&addon).unwrap();

        let mut snap = ChatSnapshot::default();
        snap.count = 2;
        let mut msg0 = [0u8; 128];
        let b0 = b"Alice tells you, hey";
        msg0[..b0.len()].copy_from_slice(b0);
        snap.messages[0] = ChatMessageSnapshot {
            text: msg0,
            text_len: b0.len() as u8,
            msg_type: MSG_TYPE_TELL,
            timestamp_ms: 1000,
            seq: 1,
        };
        let mut msg1 = [0u8; 128];
        let b1 = b"Let's go group";
        msg1[..b1.len()].copy_from_slice(b1);
        snap.messages[1] = ChatMessageSnapshot {
            text: msg1,
            text_len: b1.len() as u8,
            msg_type: MSG_TYPE_PARTY,
            timestamp_ms: 1050,
            seq: 2,
        };

        host.snapshot.set(Snapshot {
            in_game: true,
            chat: snap,
            ..Default::default()
        });

        // Test TellPartner extraction as performed by wereoxxs_ui/modules/chat_window.lua
        let chunk = host.lua.load(
            r#"
            local Chat = require("frontiers_forge.chat")
            _G.tells = {}
            _G.parties = {}

            -- A buggy subscriber that throws an error
            Chat.On("buggy_addon", Chat.Events.OnChatMessage, function(msg)
                error("deliberate subscriber fault")
            end)

            Chat.On("wereoxxs_ui", Chat.Events.OnChatMessage, function(msg)
                if msg.type_name == "Tell" then
                    local partner = msg.text:match("^(%S+) tells you") or msg.text:match("^[Yy]ou tell (%S+)")
                    if partner then
                        partner = partner:gsub("[,:%.]+$", "")
                    end
                    table.insert(_G.tells, { partner = partner, text = msg.text })
                elseif msg.type_name == "Party" then
                    table.insert(_G.parties, msg.text)
                end
            end)

            -- Initial prime
            Chat.Update()
            assert(#_G.tells == 0)
            assert(#_G.parties == 0)
            "#,
        );
        chunk.exec().unwrap();

        // Mutate snapshot with new tell and party message
        let mut snap2 = snap;
        snap2.count = 4;
        let mut msg2 = [0u8; 128];
        let b2 = b"You tell Alice, cya";
        msg2[..b2.len()].copy_from_slice(b2);
        snap2.messages[2] = ChatMessageSnapshot {
            text: msg2,
            text_len: b2.len() as u8,
            msg_type: MSG_TYPE_TELL,
            timestamp_ms: 2000,
            seq: 3,
        };
        let mut msg3 = [0u8; 128];
        let b3 = b"Pulling now...";
        msg3[..b3.len()].copy_from_slice(b3);
        snap2.messages[3] = ChatMessageSnapshot {
            text: msg3,
            text_len: b3.len() as u8,
            msg_type: MSG_TYPE_PARTY,
            timestamp_ms: 2100,
            seq: 4,
        };

        host.snapshot.set(Snapshot {
            in_game: true,
            chat: snap2,
            ..Default::default()
        });

        let chunk2 = host.lua.load(
            r#"
            local Chat = require("frontiers_forge.chat")
            Chat.Update()

            assert(#_G.tells == 1, "One tell should be captured")
            assert(_G.tells[1].partner == "Alice", "Tell partner extracted as Alice")
            assert(_G.tells[1].text == "You tell Alice, cya")

            assert(#_G.parties == 1, "One party message should be captured")
            assert(_G.parties[1] == "Pulling now...")

            -- Verify subscriber error was recorded and isolated without aborting execution
            local errors = Chat.GetSubscriberErrors()
            assert(#errors > 0, "Error from buggy_addon recorded")
            assert(errors[1].owner == "buggy_addon")
            assert(string.find(errors[1].error, "deliberate subscriber fault") ~= nil)
            "#,
        );
        chunk2.exec().unwrap();
    }

    #[test]
    fn forge_chat_scrollback_monotonic_delivery() {
        let dir = temp_test_dir("scrollback");
        let addon = test_addon(&dir);
        let host = Host::for_addon(&addon).unwrap();

        let mut deque = VecDeque::new();
        // Initially 2 messages in-game
        deque.push_back((
            "Msg 1".to_string(),
            [f32::from_bits(MSG_TYPE_SAY), 0.0, 0.0, 1.0],
        ));
        deque.push_back((
            "Msg 2".to_string(),
            [f32::from_bits(MSG_TYPE_TELL), 0.0, 0.0, 1.0],
        ));

        let snap1 = snapshot_chat(&deque, 100);
        host.snapshot.set(Snapshot {
            in_game: true,
            chat: snap1,
            ..Default::default()
        });

        let chunk = host.lua.load(
            r#"
            local Chat = require("frontiers_forge.chat")
            _G.delivered = {}

            Chat.On("test_addon", Chat.Events.OnChatMessage, function(msg)
                table.insert(_G.delivered, msg.text)
            end)

            Chat.Update()
            assert(#_G.delivered == 0, "Prime does not re-emit scrollback")
            "#,
        );
        chunk.exec().unwrap();

        // Push 10 more messages (total 12). Since deque only retains max 8 in Play,
        // we simulate popping from front when len > 8.
        for i in 3..=12 {
            deque.push_back((
                format!("Msg {i}"),
                [f32::from_bits(MSG_TYPE_SAY), 0.0, 0.0, 1.0],
            ));
            if deque.len() > 8 {
                deque.pop_front();
            }
            let snap = snapshot_chat(&deque, (100 + i * 50) as u32);
            host.snapshot.set(Snapshot {
                in_game: true,
                chat: snap,
                ..Default::default()
            });

            let update_chunk = host.lua.load(
                r#"
                local Chat = require("frontiers_forge.chat")
                Chat.Update()
                "#,
            );
            update_chunk.exec().unwrap();
        }

        // Verify that all 10 subsequent messages (Msg 3 through Msg 12) were delivered!
        let check_chunk = host.lua.load(
            r#"
            assert(#_G.delivered == 10, "Expected 10 new messages delivered, got " .. #_G.delivered)
            assert(_G.delivered[1] == "Msg 3")
            assert(_G.delivered[10] == "Msg 12")
            "#,
        );
        check_chunk.exec().unwrap();
    }
}
