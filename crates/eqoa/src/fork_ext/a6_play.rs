// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A6 on `Play`: the Start menu and the chat line as windows a mod holds (`addons::native_a6`,
//! `addons::window_sessions`). A child of the Play adapter, so it reads `Play`'s private state; it adds no line to
//! any upstream file.
//!
//! **Start menu** (`TakeoverKind::StartMenu`, scope `all`). The Start key (the keyboard's, the pad's, a window
//! returning to it) calls `Play::open_start_menu`, which sets `mode = 13` and `start_open` and waits for the next
//! draw. Each step (`tick`, from `begin_iteration`, before any draw) a held menu undoes that open at once, as if
//! the key had not been pressed, and gives the holder a session whose data is the menu's entries: the game's own
//! tables for this server, filtered the way the game filters them when it opens the menu (`run_start_menu`'s
//! opening: the bank's and expansion's table, the TEST entry, the live client's group menu; the client's own
//! entries - duel, combat record, raid, dismiss mercenary - are already in the tables). A command runs through the
//! game's own `start_actions`, as the entry's key press does. A second press of the key while the mod's menu is up
//! closes it, as the game's own Start key closes the game's.
//!
//! **Chat line** (`TakeoverKind::ChatEntry`, scope `all`). Only the edit box's chat purpose (`0x0B0F_0000`) is
//! diverted: the passwords, report text, buddy, guild, ignore and alarm boxes, a Who list's Tell, never reach a mod.
//! What the game would have put in the box (the text of an R or T key's `/reply `, `/tell `, a line kept with
//! its item links) is the session's data. The line is sent with `eqoa.actions.chat_line`.
//!
//! Nothing here is replayed and nothing here confirms: an action completes when the client has done what the
//! game's own control does.
use super::Play;
use crate::fork_ext::addons::{
    action_broker::DispatchOutcome,
    native_a6::{self, A6Action, ChatInfo, LinkSpec, Route},
    window_sessions,
    window_takeover::{self, TakeoverKind},
};
use serde_json::{Value, json};
use std::cell::Cell;

/// The Start menu's Items entry: the game shows its item page over the frame, which a mod has no copy of.
const MSG_ITEMS: u32 = 0x0200_0000;
/// The edit box's purpose for a line of chat (`Play::open_chat_line`).
const CHAT_PURPOSE: u32 = 0x0B0F_0000;

thread_local! {
    /// A chat line of the game's was open at the last step (one open before a mod claimed the kind stays the game's).
    static CHAT_WAS_OPEN: Cell<bool> = const { Cell::new(false) };
    /// The game's own menu is wanted for the next open (a mod's menu entry for a page it cannot show).
    static PASS_ONCE: Cell<bool> = const { Cell::new(false) };
    /// Link offers made so far (a session's `offer.seq`).
    static OFFER_SEQ: Cell<u64> = const { Cell::new(0) };
}

/// A mod holds the chat line open (its `chat_entry` session): the character keeps running while it types, as it does in
/// the game's own chat box.
pub(crate) fn chat_line_open() -> bool {
    window_sessions::of_kind("chat_entry").is_some()
}

fn refuse(code: &'static str, reason: impl Into<String>) -> DispatchOutcome {
    DispatchOutcome::Refused { code, reason: reason.into() }
}

fn wide_str(w: &[u16]) -> String {
    let end = w.iter().position(|&c| c == 0).unwrap_or(w.len());
    String::from_utf16_lossy(&w[..end])
}

/// The holder of `kind`: its `(vm, addon)`.
fn holder(kind: TakeoverKind) -> Option<(u64, String)> {
    Some((window_takeover::holder_vm(kind)?, window_takeover::holder_of(kind)?))
}

/// What the game would show for an entry: its label (set at run time, or its word), and whether it is dimmed.
fn label_of(play: &Play, start: &crate::startmenu::StartData, e: &crate::client_image::MenuEntry) -> (String, bool) {
    match start.menus.labels.get(&(e.msg, e.id)) {
        Some(l) => (wide_str(&l.text), !l.dim),
        None => {
            let s = play.words.as_ref().map(|w| crate::duel::word(w, e.word, 0)).unwrap_or_default();
            (if s == "(null)" { String::new() } else { s }, true)
        }
    }
}

/// The entries of menu `menu`, each with its sub-menu's entries inside it.
fn entries_of(play: &Play, menu: usize, seen: &mut Vec<usize>) -> Vec<Value> {
    let Some(start) = play.start.as_ref() else { return Vec::new() };
    let Some(m) = start.menus.list.get(menu) else { return Vec::new() };
    m.entries
        .iter()
        .map(|e| {
            let (text, enabled) = label_of(play, start, e);
            let mut row = json!({"text": text, "enabled": enabled});
            if e.msg != 0 {
                row["command"] = json!(format!("{:#x}:{}", e.msg, e.id));
                // the item page is the game's own: running it hands the Start key back to the game's menu
                if e.msg == MSG_ITEMS {
                    row["native"] = json!(true);
                }
            } else if let Some(sub) = e.sub
                && !seen.contains(&sub)
                && seen.len() < 6
            {
                seen.push(sub);
                row["entries"] = json!(entries_of(play, sub, seen));
                seen.pop();
            }
            row
        })
        .collect()
}

/// `run_start_menu`'s opening, without a window: the table for the bank and the expansion, the TEST entry while the
/// GM's channel is on, the live client's group menu. Returns the menu the game would show first.
fn root_menu(play: &mut Play) -> Option<usize> {
    let bank_tables = crate::live::features().auction(play.dump.bank.p78cc);
    let test = play.admin.part.test_entry();
    let expansion = play.session.expansion;
    let s = play.start.as_mut()?;
    s.pick_test(bank_tables, expansion, test);
    crate::play::liveplay::group_menu(&mut s.menus);
    Some(s.menu.main.menu)
}

/// The Start menu's session data: its entries as the game would list them now.
pub(crate) fn start_menu_data(play: &mut Play) -> Option<Value> {
    let root = root_menu(play)?;
    let entries = entries_of(play, root, &mut vec![root]);
    Some(json!({"entries": entries}))
}

/// The entry `command` of a session's data, searched through the sub-menus.
fn find_entry<'a>(entries: &'a Value, command: &str) -> Option<&'a Value> {
    for e in entries.as_array()? {
        if e["command"].as_str() == Some(command) {
            return Some(e);
        }
        if let Some(found) = find_entry(&e["entries"], command) {
            return Some(found);
        }
    }
    None
}

/// A link of the edit box as session data: places counted in characters from 1, the item's name.
fn link_json(text: &[u16], l: &crate::editbox::TextLink) -> Value {
    let end = (l.start + l.len).min(text.len());
    let start = l.start.min(end);
    let before = String::from_utf16_lossy(&text[..start]).chars().count();
    let name = String::from_utf16_lossy(&text[start..end]);
    json!({
        "start": before + 1,
        "len": name.chars().count(),
        "list": if l.list == 0 { "inventory" } else { "bank" },
        "key": l.key,
        "name": name,
    })
}

fn chat_data(play: &Play, offer: Option<Value>) -> Value {
    let text = &play.edit.text;
    let links: Vec<Value> = play.edit.links.iter().map(|l| link_json(text, l)).collect();
    let mut data = json!({
        "text": String::from_utf16_lossy(text),
        "links": links,
        "channel": native_a6::channel_name(play.channel).unwrap_or("say"),
    });
    if !play.reply_name.is_empty() {
        data["reply_to"] = json!(play.reply_name);
    }
    if let Some(offer) = offer {
        data["offer"] = offer;
    }
    data
}

/// The chat state a mod's `eqoa.chat.tabs()` and `reply_to()` read, as the player set it (`settings::client`).
fn chat_info(play: &Play) -> ChatInfo {
    let opts = crate::settings::client();
    let mut tabs = Vec::new();
    for t in 0..=crate::chat::TABS {
        let kinds: Vec<&'static str> = crate::chat::Kind::ALL
            .iter()
            .filter(|k| opts.chat_tabs[k.index()] as usize == t)
            .map(|k| k.key())
            .collect();
        if t != 0 && kinds.is_empty() {
            continue;
        }
        // as `chat::Tabs::name`: Main, the tab's own name, else its first kind's
        let name = if t == 0 {
            "Main".to_string()
        } else {
            let own = crate::chat::tab_name_text(&opts.chat_tab_names[t - 1]);
            if own.is_empty() {
                let first = crate::chat::Kind::ALL.iter().find(|k| opts.chat_tabs[k.index()] as usize == t);
                first.map_or("Tab", |k| k.label()).chars().take(crate::chat::TAB_NAME_MOST).collect()
            } else {
                own
            }
        };
        tabs.push((t, name, kinds));
    }
    ChatInfo { tabs, reply_to: play.reply_name.clone() }
}

/// Each step's work for the Start menu and the chat line (`begin_iteration`).
pub(crate) fn tick(play: &mut Play) {
    native_a6::next_frame();
    if crate::fork_ext::addons::native_core::capture_requested() {
        native_a6::set_chat_info(chat_info(play));
    }
    // seen every step, held or not, so a line open before a mod claimed the kind is known to have been open
    let open_now = play.mode == 2 && play.edit.purpose == CHAT_PURPOSE;
    let fresh = open_now && !CHAT_WAS_OPEN.with(|c| c.replace(open_now));
    if !open_now {
        CHAT_WAS_OPEN.with(|c| c.set(false));
    }
    if !window_takeover::has_activity() && !window_sessions::is_active() {
        return;
    }
    start_menu(play);
    chat_entry(play, fresh);
}

/// A mod's window of a client-opened kind ends when nobody holds the kind any more (released, disabled, the
/// player's switch): the game's own window is the player's again. Told to its owner when it is still there.
fn drop_if_not_held(kind: TakeoverKind) -> bool {
    match window_sessions::of_kind(kind.name()) {
        Some(s) if window_takeover::holder_vm(kind) != Some(s.vm) => {
            window_sessions::end_kind(kind.name(), "released");
            false
        }
        _ => true,
    }
}

fn may_divert() -> bool {
    let env = window_sessions::env();
    !window_takeover::use_game_windows() && !env.away && !env.dead
}

fn start_menu(play: &mut Play) {
    drop_if_not_held(TakeoverKind::StartMenu);
    let Some((vm, addon)) = holder(TakeoverKind::StartMenu) else {
        PASS_ONCE.with(|p| p.set(false));
        return;
    };
    // only an opening made since the last step (a key, a window returning to the menu): a game menu already
    // up when the mod claimed the kind stays the game's until it closes
    if play.mode != 13 || play.start_open.is_none() || !may_divert() {
        return;
    }
    if PASS_ONCE.with(|p| p.replace(false)) {
        return;
    }
    let toggle = window_sessions::get(vm, "start_menu").map(|s| s.id);
    let data = if toggle.is_none() { start_menu_data(play) } else { None };
    if toggle.is_none() && data.is_none() {
        // no menu tables yet: the game's own menu
        return;
    }
    // undo the open: the menu was never drawn, the camera never framed (`open_start_menu` pushed that frame)
    play.start_open = None;
    play.mode = 0;
    if play.race_frame_due.last() == Some(&true) {
        play.race_frame_due.pop();
    }
    if let Some(s) = play.start.as_mut() {
        s.menu.visible = false;
    }
    match (toggle, data) {
        // the Start key again: the game's menu closes on it, so does the mod's
        (Some(id), _) => {
            let _ = window_sessions::close(vm, &id, "key");
        }
        (None, Some(data)) => {
            window_sessions::open_client(vm, &addon, "start_menu", "key", data);
        }
        (None, None) => {}
    }
}

fn chat_entry(play: &mut Play, fresh: bool) {
    drop_if_not_held(TakeoverKind::ChatEntry);
    let Some((vm, addon)) = holder(TakeoverKind::ChatEntry) else { return };
    // an opening made since the last step (a line already open when the mod claimed the kind stays the game's until
    // it closes); the chat purpose only, and not a Who list's Tell (it goes back to the list)
    if !fresh || play.mode != 2 || play.edit.purpose != CHAT_PURPOSE {
        return;
    }
    // a line that stays the game's says why in the log (owner test 2026-11-23: Enter typed in the game's box)
    if play.who_tell.is_some() || !may_divert() {
        tlog!("chat line stays the game's: who_tell={} may_divert={}", play.who_tell.is_some(), may_divert());
        return;
    }
    // a pad press opened it: the game's line has the pad's soft keyboard and a cancel button; a mod's line takes the
    // whole pad away while it types (`capture_text`), so a pad player is never given one
    if native_a6::pad_recent() {
        tlog!("chat line stays the game's: a pad button went down in the last steps");
        return;
    }
    CHAT_WAS_OPEN.with(|c| c.set(false));
    let data = chat_data(play, None);
    play.edit.clear();
    play.edit.title.clear();
    play.edit.purpose = 0;
    play.edit_keys.clear();
    play.link_draft = None;
    play.mode = 0;
    window_sessions::open_client(vm, &addon, "chat_entry", "key", data);
}

/// `eqoa.actions.link_item` while a mod holds the chat line: the item goes to the mod's line, never into the game's.
/// None where nothing is held (the game's own Link Item then).
pub(crate) fn offer_link(play: &mut Play, idx: usize) -> Option<DispatchOutcome> {
    let (vm, addon) = holder(TakeoverKind::ChatEntry)?;
    if !may_divert() {
        return None;
    }
    if !play.links_made() {
        return Some(refuse(
            "not_ready",
            "the item could not be linked: this server offers no item links, or the chat line has no room",
        ));
    }
    let it = play.dump.inventory.items.get(idx)?;
    // the name as a console's keyboard could type it (the server writes its own in its place)
    let name: String = it.item.name.chars().filter(|&c| crate::editbox::typable(c)).collect();
    let name = name.trim().to_string();
    if name.is_empty() {
        return Some(refuse("not_ready", "the item has no name that can be put in a line"));
    }
    let seq = OFFER_SEQ.with(|s| {
        s.set(s.get() + 1);
        s.get()
    });
    let offer = json!({"seq": seq, "name": name, "list": "inventory", "key": it.inv_index});
    match window_sessions::of_kind("chat_entry") {
        Some(s) if s.vm == vm => {
            let mut data = s.data_json();
            data["offer"] = offer;
            window_sessions::set_data(&s.id, data);
        }
        _ => {
            let data = chat_data(play, Some(offer));
            window_sessions::open_client(vm, &addon, "chat_entry", "mod", data);
        }
    }
    Some(DispatchOutcome::Completed { detail: Some(json!({"sent": true, "offered": true})) })
}

pub(crate) fn dispatch(play: &mut Play, action: &A6Action) -> DispatchOutcome {
    match action {
        A6Action::StartMenuCommand { vm, session, command } => start_menu_command(play, *vm, session, command),
        A6Action::SetChannel { channel } => {
            if play.is_dead() {
                return refuse("not_ready", "cannot change the chat channel while dead");
            }
            // the game's own channel step offers only these (`chat_channel_step`)
            let ok = match channel {
                2 => play.group.in_group && play.group.count() >= 2,
                4 => crate::guild::Cmds::can_member(&play.guild) == 0,
                _ => true,
            };
            if !ok {
                return refuse(
                    "not_ready",
                    if *channel == 2 { "the player is not in a group of two or more" } else { "the player is not in a guild" },
                );
            }
            play.group_changed(*channel);
            DispatchOutcome::Completed { detail: Some(json!({"sent": true, "channel": native_a6::channel_name(*channel)})) }
        }
        A6Action::ChatLine { route, to, text, links } => chat_line(play, *route, to.as_deref(), text, links),
        A6Action::OpenLink { id, name } => {
            if !play.links_made() {
                return refuse("not_ready", "this server offers no item links, or the player has turned them off");
            }
            // the game's own click on a link: its card opens over play and the server is asked for the item
            play.open_link(*id, name);
            DispatchOutcome::Completed { detail: Some(json!({"sent": true})) }
        }
    }
}

fn start_menu_command(play: &mut Play, vm: u64, session: &str, command: &str) -> DispatchOutcome {
    let Some(s) = window_sessions::find(vm, session).filter(|s| s.kind == "start_menu") else {
        return refuse("stale_handle", "the Start menu session is no longer open");
    };
    if crate::fork_ext::addons::window_actions::report_guard(&json!({}), Some(command), None).is_some() {
        return refuse("permission_denied", "report routes are forbidden");
    }
    let entries = s.data["entries"].clone();
    let Some(entry) = find_entry(&entries, command) else {
        return refuse("unsupported", format!("'{command}' is not a command of this menu"));
    };
    if entry["enabled"] != json!(true) {
        return refuse("not_ready", "the entry is not enabled");
    }
    if play.is_dead() || play.mode != 0 {
        return refuse("not_ready", "another window or box of the game is up");
    }
    let Some((msg, id)) = command.split_once(':').and_then(|(m, i)| {
        Some((u32::from_str_radix(m.strip_prefix("0x")?, 16).ok()?, i.parse::<i32>().ok()?))
    }) else {
        return refuse("invalid_argument", "malformed command");
    };
    if msg == MSG_ITEMS {
        // the item page is the game's: its own Start menu opens (this once, past the mod)
        PASS_ONCE.with(|p| p.set(true));
        play.open_start_menu(true);
        let _ = window_sessions::close(vm, session, "passed");
        return DispatchOutcome::Completed { detail: Some(json!({"sent": true, "passed": true})) };
    }
    // `start_actions` closes the menu first (`close_start_menu(0)`), which asks for the play camera back: the
    // mod's menu never framed the character, so there is nothing to put back
    let before = play.race_frame_due.len();
    play.start_actions(vec![(msg, id)]);
    if play.race_frame_due.len() > before && !play.race_frame_due[before] {
        play.race_frame_due.remove(before);
    }
    let _ = window_sessions::close(vm, session, "command");
    DispatchOutcome::Completed { detail: Some(json!({"sent": true})) }
}

fn chat_line(play: &mut Play, route: Route, to: Option<&str>, text: &str, links: &[LinkSpec]) -> DispatchOutcome {
    if play.session.self_id.is_none() {
        return refuse("not_ready", "this connection has had no world entry, so the game sends no chat");
    }
    if route == Route::Reply && play.reply_name.is_empty() {
        return refuse("not_ready", "nobody has sent the player a tell to reply to");
    }
    // each link must be an item the player has
    for l in links {
        let items = if l.list == 0 { &play.dump.inventory.items } else { &play.dump.bank.items.items };
        if !items.iter().any(|it| it.inv_index == l.key) {
            return refuse("stale_handle", "a linked item is no longer where the link says");
        }
    }
    // on the channel plain text already goes to, the game's Enter sends the text alone (one message); another channel
    // goes as the game's command for it
    let current = match route {
        Route::Say => 0,
        Route::Group => 2,
        Route::Shout => 3,
        Route::Guild => 4,
        Route::Tell | Route::Reply => u8::MAX,
    };
    let prefix = if current == play.channel {
        String::new()
    } else {
        match (route, to) {
            (Route::Tell, Some(name)) => format!("{} {name} ", route.command()),
            _ => format!("{} ", route.command()),
        }
    };
    let shift = prefix.encode_utf16().count();
    let whole = format!("{prefix}{text}");
    let spots: Vec<crate::editbox::TextLink> = links
        .iter()
        .map(|l| crate::editbox::TextLink { start: l.start + shift, len: l.len, list: l.list, key: l.key })
        .collect();
    play.chat_line_linked(&whole, &spots);
    DispatchOutcome::Completed { detail: Some(json!({"sent": true, "links": spots.len()})) }
}

#[cfg(test)]
#[path = "a6_play_tests.rs"]
mod tests;
