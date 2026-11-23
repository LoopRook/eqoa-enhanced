// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A2 on `Play`: NPC dialogs and the game's yes/no offers a mod holds (`addons::dialog_sessions`). A child of the
//! Play adapter, so it reads `Play`'s private state; it adds no line to any upstream file.
//!
//! The game handles each packet as it always does (so its sounds and its own queue rules run); right after, the client
//! (`end_game_event`) takes what that packet showed out of the game's window when a mod holds its kind: the window
//! mode is put back, a queued dialog is taken out of the game's queue, a parked invitation out of its list. Only what
//! this packet opened or queued is taken: a window already open before the packet is the player's.
//!
//! An answer is the game's own: the dialog is put in the game's window for that one call and the game's own keys are
//! pressed on it (`Play::dialog_key`), so what goes to the server is what the key press sends. A dialog that ends with
//! no answer goes back to the game's window (`give`), never dropped.
use super::Play;
use crate::dialog::{DialogBox, Key};
use crate::fork_ext::addons::{
    action_broker::DispatchOutcome,
    confirm::{self, Act},
    dialog_sessions::{self, Intake, Item, TalkedTo},
    window_sessions,
    window_takeover::{self, TakeoverKind},
};
use serde_json::json;
use std::cell::RefCell;

/// Players move 25 units from where a dialog opened and the dialog is let go (6.5's walk-away distance).
const WALK_AWAY_SQ: f32 = 625.0;

fn log(line: String) {
    window_takeover::log(line);
}

fn refuse(code: &'static str, reason: impl Into<String>) -> DispatchOutcome {
    DispatchOutcome::Refused { code, reason: reason.into() }
}

/// What the game looked like when the packet arrived.
#[derive(Default)]
struct Pre {
    mode: i32,
    dialogs: usize,
    box_waiting: bool,
    invites: usize,
    summon: Option<(String, u8)>,
    gone: Option<u32>,
    /// The id of the 0x0034 dialog this packet carries.
    box_id: u32,
}

thread_local! {
    static WAS_DEAD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static PRE: RefCell<Option<Pre>> = const { RefCell::new(None) };
}

#[cfg(test)]
pub(crate) fn reset_for_test() {
    WAS_DEAD.with(|d| d.set(false));
    PRE.with(|p| *p.borrow_mut() = None);
}

fn pos(play: &Play) -> Option<[f32; 3]> {
    play.self_at.map(|(p, _)| p)
}

fn talked_to(play: &Play) -> TalkedTo {
    if play.target == u32::MAX {
        return None;
    }
    play.entity_name(play.target).map(|name| (play.target, name))
}

/// Before the game handles `event`: what it looks like now, when a mod holds a dialog or offer kind.
pub(crate) fn pre_event(play: &Play, event: &net::session::Event) {
    if !dialog_sessions::any_holder() {
        PRE.with(|p| *p.borrow_mut() = None);
        return;
    }
    let mut pre = Pre {
        mode: play.mode,
        dialogs: play.dialogs.len(),
        box_waiting: play.pending_box.is_some(),
        invites: play.group.invites.len(),
        ..Default::default()
    };
    match event {
        net::session::Event::SummonPrompt { name, kind } => pre.summon = Some((name.clone(), *kind)),
        net::session::Event::GroupInviteGone(id) => pre.gone = Some(*id),
        net::session::Event::DialogBox { id, .. } => pre.box_id = *id,
        _ => {}
    }
    PRE.with(|p| *p.borrow_mut() = Some(pre));
}

fn held(kind: TakeoverKind) -> bool {
    window_takeover::is_taken(kind)
}

/// After the game handled the packet: what it showed or queued is taken for the mod.
pub(crate) fn post_event(play: &mut Play) {
    let pre = PRE.with(|p| p.borrow_mut().take());
    let Some(pre) = pre else {
        // nothing arrived for a dialog; the next waiting one may still start
        tick_start(play);
        return;
    };
    let at = pos(play);
    let talked = talked_to(play);
    // the game's own rules for a dead character stay the game's: nothing is held for a conversation the dead cannot
    // answer (an offer, a resurrection, still is)
    let alive = !play.is_dead();
    let mut taken: Vec<Item> = Vec::new();

    // plain text: opened now (mode 0 before), or queued behind a window (taken out of the game's queue)
    if alive && held(TakeoverKind::TextDialog) {
        if pre.mode == 0 && play.mode == 9 {
            let text = play.priv_chat.read_text().to_vec();
            play.mode = 0;
            taken.push(Item::Text { text, talked_to: talked.clone() });
        }
        while play.dialogs.len() > pre.dialogs {
            let Some(text) = play.dialogs.pop_back() else { break };
            taken.insert(0, Item::Text { text, talked_to: talked.clone() });
        }
    }
    // a dialog with answers: opened now (the game set it in its window: every option is on its first page), or
    // waiting in the game's one place
    if alive && held(TakeoverKind::ResponseDialog) {
        if pre.mode == 0 && play.mode == 7 {
            let (text, options, _) = play.response.read_state();
            let text = text.to_vec();
            let options: Vec<Vec<u16>> = options.iter().filter(|(_, kind, _)| *kind == 1).map(|(_, _, t)| t.to_vec()).collect();
            let id = pre.box_id;
            play.mode = 0;
            taken.push(Item::Box { id, text, options, talked_to: talked.clone() });
        } else if !pre.box_waiting
            && let Some(d) = play.pending_box.take()
        {
            taken.push(Item::Box { id: d.id, text: d.text, options: d.options, talked_to: talked.clone() });
        }
    }
    // a group invitation: parked out of the game's list so it does not ask again each step
    if alive && held(TakeoverKind::GroupInvite) && play.group.invites.len() > pre.invites {
        while play.group.invites.len() > pre.invites {
            let Some(i) = play.group.invites.pop() else { break };
            if pre.mode == 0 && play.mode == 14 && play.mbox.p2 == 0x702_0000 {
                play.mode = 0;
            }
            let text = invite_text(play, &i.name);
            taken.push(Item::Invite { id: i.id, name: i.name, text });
        }
    }
    if let Some((name, kind)) = pre.summon {
        let which = if kind == 0 { TakeoverKind::SummonOffer } else { TakeoverKind::ReviveOffer };
        // a prompt already up before this one stays the game's (it was replaced in the game's box; nothing is left)
        if held(which) && pre.mode != 14 && play.mode == 14 && play.mbox.p1 == 2 {
            let text = play.mbox.text.clone();
            play.mode = if pre.mode == 13 { 0 } else { pre.mode };
            taken.push(Item::Offer { summons: kind == 0, name, text });
        }
    }
    if let Some(id) = pre.gone {
        dialog_sessions::invite_gone(id);
    }
    for item in taken {
        let label = item.kind().name();
        let refused_id = match &item {
            Item::Box { id, .. } => Some(*id),
            _ => None,
        };
        match dialog_sessions::hold(item, at) {
            Intake::Held => log(format!("takeover: {label} HIDDEN for the mod (a dialog the server sent; scope all)")),
            Intake::Refused => {
                // the game's own rule: one answerable dialog waits, a further one is refused to the server
                if let Some(id) = refused_id {
                    play.session.send_msg(0x0035, id, 255);
                }
                log(format!("takeover: {label} refused to the server (one already waits, as the game does)"));
            }
            Intake::Dropped => log(format!("takeover: {label} dropped (eleven already wait, as the game does)")),
            Intake::NotHeld => {}
        }
    }
    tick_start(play);
}

fn invite_text(play: &Play, name: &str) -> String {
    let fmt = play.words.as_ref().map_or_else(|| "%s".to_string(), |w| w.get(636, 0).to_string());
    fmt.replacen("%s", name, 1).replacen("%S", name, 1)
}

/// The next waiting dialog starts when none is up and play is plain.
pub(crate) fn tick_start(play: &mut Play) {
    if !dialog_sessions::is_busy() {
        return;
    }
    let ready = play.mode == 0 && play.in_world_now() && play.world_change.is_none();
    if let Some(kind) = dialog_sessions::start_next(ready, pos(play)) {
        log(format!("takeover: {} session started for the mod", kind.name()));
    }
}

/// Puts a dialog or offer in the game's own window (or its queue when another window is up), as if no mod held it.
fn give(play: &mut Play, item: Item) {
    match item {
        Item::Text { text, .. } => {
            if play.mode == 0 {
                play.open_dialog(&text, true);
            } else if play.dialogs.len() < dialog_sessions::MAX_WAITING_TEXTS {
                play.dialogs.push_back(text);
            }
        }
        Item::Box { id, text, options, .. } => {
            let d = DialogBox { id, text, options };
            if play.mode == 0 {
                play.show_dialog_box(d);
            } else if play.pending_box.is_none() {
                play.pending_box = Some(d);
            } else {
                play.session.send_msg(0x0035, id, 255);
            }
        }
        Item::Invite { id, name, .. } => {
            if !play.group.invites.iter().any(|i| i.id == id) {
                play.group.invites.push(crate::group::Invite { id, name: name.clone() });
            }
            if play.mode == 0 {
                play.show_invite(&name);
            }
        }
        Item::Offer { summons, name, .. } => play.summon_prompt(&name, if summons { 0 } else { 1 }),
    }
}

/// Each step: what the mod passed, what a released claim, a disabled mod or the idle limit ends, the player's walk.
pub(crate) fn tick(play: &mut Play) {
    // the step the character dies: the game closed its dialogs and refused its invitations (`msg_005e`)
    let died = play.is_dead() && !WAS_DEAD.with(|d| d.replace(true));
    if !play.is_dead() {
        WAS_DEAD.with(|d| d.set(false));
    }
    if !dialog_sessions::is_busy() {
        return;
    }
    if died {
        for id in dialog_sessions::death() {
            play.session.send_msg(0x0624, id, 3);
        }
        log("takeover: held dialogs and invitations ended (the character died, as the game closes them)".into());
        return;
    }
    dialog_sessions::reconcile();
    for a in dialog_sessions::take_passes() {
        log(format!("takeover: {} passed back to the game's window", a.item.kind().name()));
        give(play, a.item);
        window_sessions::end_dialog_session(&a.id, "passed");
    }
    for item in dialog_sessions::take_give_back() {
        log(format!("takeover: {} given back to the game's window (its mod no longer shows it)", item.kind().name()));
        give(play, item);
    }
    // the player walked away from the NPC: the dialog is let go, a response dialog answered "no choice" as the game's
    // own Escape does; the game's dialog held the player in place, a hidden one does not
    for slot in dialog_sessions::walkers() {
        let (Some(from), Some(now)) = (slot.pos, pos(play)) else { continue };
        let d = [now[0] - from[0], now[1] - from[1], now[2] - from[2]];
        if ee::lt(WALK_AWAY_SQ, d[0] * d[0] + d[1] * d[1] + d[2] * d[2]) {
            if let Item::Box { id, .. } = &slot.item {
                play.session.send_msg(0x0035, *id, 255);
            }
            log(format!("takeover: {} ended (walked away more than 25 units)", slot.item.kind().name()));
            dialog_sessions::finish(&slot.id, "walked_away");
        }
    }
    tick_start(play);
}

/// What an answer to a session is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Choice {
    /// `dialog_answer(ref, option)`, 1-based; an offer's 1 is Yes and 2 No.
    Option(usize),
    /// `close_window(ref)`: Done, "no choice", No.
    Close,
}

/// `dialog_answer` / `close_window` on a session ref.
pub(crate) fn dispatch_answer(play: &mut Play, session: &str, choice: Choice) -> DispatchOutcome {
    let Some(vm) = crate::fork_ext::addons::action_broker::current_dispatch_vm() else {
        return refuse("not_ready", "no mod is asking");
    };
    let Some(active) = dialog_sessions::find(session).filter(|a| a.vm == vm) else {
        return refuse("stale_handle", "the dialog or offer is gone");
    };
    // a dead character has no conversation to answer; only an offer is made to the dead (a resurrection)
    if play.is_dead() && !matches!(active.item, Item::Offer { .. }) {
        return refuse("not_ready", "cannot answer a dialog while dead");
    }
    let option = match choice {
        Choice::Option(n) => n,
        Choice::Close => 0,
    };
    // an offer's Yes is not sent: the player's own Yes to the client's question sends it
    if let Item::Offer { summons, text, .. } = &active.item
        && choice == Choice::Option(1)
    {
        let act = Act::AcceptOffer { session: active.id.clone(), summons: *summons };
        let ask_text = format!("Accept? {}", plain(text));
        let addon = crate::fork_ext::addons::action_broker::dispatch_addon().unwrap_or_default();
        return match confirm::open_for_offer(vm, &addon, Some(active.id.clone()), act, ask_text) {
            Ok(id) => DispatchOutcome::Completed { detail: Some(json!({"asked": true, "confirm": id})) },
            Err((code, msg)) => refuse(code, msg),
        };
    }
    match press(play, &active.item, option) {
        Ok(sent) => {
            dialog_sessions::finish(&active.id, "answered");
            DispatchOutcome::Completed { detail: Some(json!({"sent": sent})) }
        }
        Err(e) => e,
    }
}

fn plain(text: &[u16]) -> String {
    let end = text.iter().position(|&c| c == 0).unwrap_or(text.len());
    String::from_utf16_lossy(&text[..end])
}

/// The player's own Yes to the client's question about a summons or resurrection: the game's own keys, Yes.
pub(crate) fn accept_offer(play: &mut Play, session: &str) -> Result<(), String> {
    let Some(active) = dialog_sessions::find(session) else {
        return Err("the offer is gone".into());
    };
    if !matches!(active.item, Item::Offer { .. }) {
        return Err("the offer is gone".into());
    }
    match press(play, &active.item, 1) {
        Ok(_) => {
            dialog_sessions::finish(&active.id, "answered");
            Ok(())
        }
        Err(DispatchOutcome::Refused { reason, .. }) => Err(reason),
        Err(_) => Err("the offer could not be answered".into()),
    }
}

/// The game's own keys on the dialog, put in the game's window for this one call. Returns whether the game sent
/// something to the server.
fn press(play: &mut Play, item: &Item, option: usize) -> Result<bool, DispatchOutcome> {
    let busy = || refuse("busy", "another game window is up: answer when it is closed");
    match item {
        Item::Text { text, .. } => {
            if play.mode != 0 {
                return Err(busy());
            }
            play.priv_chat.set_text(text);
            play.mode = 9;
            play.dialog_key(Key::Enter);
            play.mode = 0;
            // the game's Done sends nothing
            Ok(false)
        }
        Item::Box { id, text, options, .. } => {
            if play.mode != 0 {
                return Err(busy());
            }
            let target = if option == 0 { None } else { Some(option - 1) };
            if target.is_some_and(|t| t >= options.len()) {
                return Err(refuse("out_of_range", "no such option"));
            }
            if play.hud.is_none() {
                return Err(refuse("not_ready", "the dialog is not drawn yet"));
            }
            let d = DialogBox { id: *id, text: text.clone(), options: options.clone() };
            if let Some((vui, _, _)) = &play.hud {
                play.response.set_dialog(vui, d);
            }
            play.mode = 7;
            match target {
                None => {
                    play.dialog_key(Key::Escape);
                }
                Some(t) => walk_to_option(play, t),
            }
            if play.mode == 7 {
                play.mode = 0;
                return Err(refuse("not_ready", "the game's window could not reach that option"));
            }
            Ok(true)
        }
        Item::Invite { id, name, text } => {
            if play.mode != 0 {
                return Err(busy());
            }
            let yes = option == 1;
            // the game's own list holds the invitation while its prompt answers (`accept_invite`, `decline_invite`)
            if !play.group.invites.iter().any(|i| i.id == *id) {
                play.group.invites.insert(0, crate::group::Invite { id: *id, name: name.clone() });
            }
            let now = play.session.now() as i64;
            play.mbox.prompt(&crate::ui::wide(text), 1, 0, 0x702_0000, now);
            play.mode = 14;
            if yes {
                play.dialog_key(Key::Left);
                play.dialog_key(Key::Enter);
                // the others waiting are refused as the game does (`refuse_invites`)
                for other in dialog_sessions::take_parked_invites() {
                    play.session.send_msg(0x0624, other, 2);
                }
            } else {
                play.dialog_key(Key::Escape);
            }
            play.mode = 0;
            Ok(true)
        }
        Item::Offer { text, .. } => {
            // Yes arrives here only after the player's own Yes (`accept_offer`); No is always allowed
            let yes = option == 1;
            let saved = play.mode;
            let now = play.session.now() as i64;
            play.mbox.prompt(text, 1, 2, 0, now);
            play.mode = 14;
            if yes {
                play.dialog_key(Key::Left);
                play.dialog_key(Key::Enter);
            } else {
                play.dialog_key(Key::Escape);
            }
            if play.mode == 14 || play.mode == 0 {
                play.mode = saved;
            }
            Ok(true)
        }
    }
}

/// Moves the game's response window to option `target` (0-based, absolute) with its own keys and presses Enter,
/// paging with its More entry when the option is on a later page.
fn walk_to_option(play: &mut Play, target: usize) {
    for _ in 0..64 {
        if play.mode != 7 {
            return;
        }
        let (opts, sel): (Vec<(i32, i32)>, i32) = {
            let (_, o, sel) = play.response.read_state();
            (o.iter().map(|(i, k, _)| (*i, *k)).collect(), sel)
        };
        let to = opts
            .iter()
            .find(|(i, k)| *k == 1 && *i == target as i32)
            .or_else(|| opts.iter().find(|(_, k)| *k == 2))
            .map(|(i, _)| *i);
        let Some(to) = to else { return };
        let mut at = sel;
        while at != to {
            play.dialog_key(if to > at { Key::Down } else { Key::Up });
            at += if to > at { 1 } else { -1 };
        }
        play.dialog_key(Key::Enter);
    }
}
