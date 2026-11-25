// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A8 on `Play`: crafting for a mod window (`addons::native_a8`). A child of the Play adapter, so it reads
//! `Play`'s private state (`craft`, `words`, `dump`); it adds one line to `play.rs` (the match arm that offers the
//! server's crafting reply to `reply` first).
//!
//! The game reads the server's crafting replies (0x1102) only while its own recipe book (mode 6) or experiment
//! (mode 37) is up: the book's list and a recipe's detail are dropped otherwise, and so are the notices and the
//! "keep this recipe?" question. A mod's own window hides nothing of the game's, so the game's mode stays 0. While a
//! mod holds a crafting window `reply` reads the reply the way the game's window would, hands the words to the mod
//! (`notice`, `question` in `eqoa.craft.get()`) and tells the game's own chat what it would have told it.
//!
//! The actions are the game's own requests: `Craft`'s builders (`send6`, `send7`, `send_combine`, `send5`) and its
//! tray functions, one request at a time as the game does (`Craft::pending`). Nothing is ever resent: a request
//! that finds another still out is refused `busy`, and an uncertain combine is never replayed.
use super::Play;
use crate::craft::{Craft, CraftOut, Modes};
use crate::fork_ext::addons::{
    action_broker::{self, DispatchOutcome},
    native_a8::{self, A8Action},
    window_sessions,
};
use serde_json::{Value as Json, json};
use std::cell::RefCell;

/// The `Ask` the experiment puts to the player after it finds a recipe: Yes scribes it (`craftwnd.rs`).
const SCRIBE: i32 = 0xA703_0004u32 as i32;

/// The two windows a mod may hold for crafting (`window_sessions::OWN_KINDS`).
const KINDS: [&str; 2] = ["recipe_book", "experiment"];

/// The skills' name words, by skill (`craftwnd.rs` `skill_name`).
const SKILL_WORDS: [i32; 6] = [1584, 1583, 1586, 1585, 1587, 179];
/// Each skill's component words, copied from `craftwnd.rs` (a test keeps them equal).
const COMPONENTS01: [i32; 11] = [1492, 1482, 1483, 1493, 1484, 1495, 1490, 1491, 1496, 1497, 1481];
const COMPONENTS2: [i32; 19] = [1508, 1509, 1510, 1511, 1512, 1513, 1514, 1515, 1516, 1517, 1518, 1519, 1520, 1521, 1522, 1523, 1524, 1494, 1481];
const COMPONENTS3: [i32; 11] = [1482, 1483, 1484, 1485, 1486, 1487, 1488, 1489, 1490, 1491, 1481];
const COMPONENTS4: [i32; 11] = [1498, 1499, 1500, 1501, 1502, 1503, 1504, 1505, 1506, 1507, 1481];
const COMPONENTS5: [i32; 13] = [1526, 1527, 1528, 1529, 1530, 1531, 1532, 1533, 1534, 1535, 1536, 1537, 1481];

#[derive(Default)]
struct Local {
    /// A mod crafting window was open at the last step (its opening copies the bags, as the game's own does).
    was_open: bool,
    /// The skill whose recipe list was asked for last.
    listed: Option<u8>,
    /// The recipe whose detail was asked for last.
    inspected: Option<i32>,
    /// The server's "keep this recipe?": its words and the recipe it would scribe.
    question: Option<(String, i32)>,
    /// What the server last told the window, with a count so a mod sees a repeat of the same words.
    notice: Option<(String, u64)>,
    seq: u64,
}

thread_local! {
    static LOCAL: RefCell<Local> = RefCell::new(Local::default());
}

fn refuse(code: &'static str, reason: impl Into<String>) -> DispatchOutcome {
    DispatchOutcome::Refused { code, reason: reason.into() }
}

/// Some mod holds its own recipe book or experiment window.
pub(crate) fn mod_window_open() -> bool {
    window_sessions::any_open_of(&KINDS)
}

/// The game's own crafting window is up (the mod's actions wait for it to go).
fn game_window_up(play: &Play) -> bool {
    play.mode == 6 || play.mode == 37
}

fn words_of(play: &Play) -> vi::strtab::StringTable {
    play.words.clone().unwrap_or_default()
}

fn text_of(words: &vi::strtab::StringTable, id: i32, fallback: &str) -> String {
    let t = words.get(id, 0);
    if t.is_empty() || t == "(null)" { fallback.to_string() } else { t.to_string() }
}

/// What opening the game's experiment does with the bags (`Craft::use_id`, 0xA702_0000), without its window: the
/// bags copied, the tray emptied, no request waited on. Run once when a mod's crafting window appears.
fn on_open(play: &mut Play) {
    play.craft.pending = 0;
    play.craft.items = play.dump.inventory.clone();
    play.craft.combine.clear();
    LOCAL.with(|l| *l.borrow_mut() = Local { was_open: true, ..Local::default() });
}

/// The transition to open (and, when it is closed, back): the same step the window appears in, whichever of the
/// step or an action sees it first.
fn sync_open(play: &mut Play) -> bool {
    let open = mod_window_open();
    // the game's own recipe book or experiment owns the crafting state while it is up (its tray, its waiting request):
    // nothing here touches it until it closes, and the transition then runs once
    if game_window_up(play) {
        return open;
    }
    let was = LOCAL.with(|l| l.borrow().was_open);
    if open && !was {
        on_open(play);
    } else if !open && was {
        play.craft.combine.clear();
        LOCAL.with(|l| *l.borrow_mut() = Local::default());
        native_a8::clear();
    }
    open
}

/// A row the game's Experiment lists: a component (`p54 > 0`) that is not of type 8 (`craftwnd.rs` `refresh`).
fn offered(it: &net::dump::NetItem) -> bool {
    it.item.p54 > 0 && it.item.p4c != 8
}

fn bag_key(list: &net::dump::ItemList) -> Vec<(u32, i64, i64)> {
    list.items.iter().map(|i| (i.inv_index, i.item.id, i.quantity)).collect()
}

/// The game's Experiment cannot go stale (it holds the player in place); a mod's window can (the character walks,
/// loots, sells, destroys). While nothing is pending, the tray's copy of the bags follows the real bags, and a tray
/// entry the player no longer holds is dropped. A request out keeps the copy as the game's replies have updated it.
fn refresh_items(play: &mut Play) {
    if play.craft.pending != 0 || bag_key(&play.craft.items) == bag_key(&play.dump.inventory) {
        return;
    }
    play.craft.items = play.dump.inventory.clone();
    let mut kept: Vec<net::dump::NetItem> = Vec::new();
    for t in std::mem::take(&mut play.craft.combine) {
        let have = play.craft.items.items.iter().find(|i| i.item.id == t.item.id && offered(i)).map_or(0, |i| i.quantity);
        if (kept.iter().filter(|k| k.item.id == t.item.id).count() as i64) < have {
            kept.push(t);
        }
    }
    play.craft.combine = kept;
}

/// Each step's work: the window's opening and closing, and the snapshot a mod reads.
pub(crate) fn tick(play: &mut Play, game_step: bool) {
    if !sync_open(play) || game_window_up(play) {
        return;
    }
    refresh_items(play);
    if game_step || native_a8::current().is_none() {
        native_a8::publish(snapshot(play));
    }
}

/// The world is let go: nothing of a crafting window is left to read.
pub(crate) fn dropped() {
    LOCAL.with(|l| *l.borrow_mut() = Local::default());
    native_a8::clear();
}

/// The component word of recipe component `comp` of `skill` (`craftwnd.rs` `component_name`).
pub(crate) fn component_word(skill: i32, comp: i32) -> Option<i32> {
    let tab: &[i32] = match skill {
        0 | 1 => &COMPONENTS01,
        2 => &COMPONENTS2,
        3 => &COMPONENTS3,
        4 => &COMPONENTS4,
        5 => &COMPONENTS5,
        _ => return None,
    };
    usize::try_from(comp).ok().and_then(|i| tab.get(i).copied())
}

fn pending_name(pending: i32) -> &'static str {
    match pending {
        0 => "none",
        1 => "remove",
        2 => "create",
        3 => "move",
        4 => "combine",
        5 => "list",
        6 => "detail",
        _ => "other",
    }
}

fn wide(s: &[u16]) -> String {
    String::from_utf16_lossy(&s[..s.iter().position(|&c| c == 0).unwrap_or(s.len())])
}

/// What a mod reads (`eqoa.craft.State`).
fn snapshot(play: &Play) -> Json {
    let craft = &play.craft;
    let words = words_of(play);
    let skills: Vec<Json> = (0..6)
        .map(|i| {
            json!({
                "id": i,
                "name": text_of(&words, SKILL_WORDS[i as usize], &format!("Skill {i}")),
                "level": craft.slot04(i),
                "rank": craft.slot08(i),
            })
        })
        .collect();
    let mut book: Vec<&crate::craft::Recipe> = craft.recipes.iter().collect();
    book.sort_by_key(|r| (r.comp, r.order));
    let recipes: Vec<Json> = book
        .iter()
        .map(|r| {
            json!({
                "id": r.id,
                "name": wide(&r.name),
                "level": r.level,
                "skill": r.skill,
                "component": r.comp,
                "component_name": component_word(r.skill as i32, r.comp as i32).map_or(String::new(), |w| text_of(&words, w, "")),
                "difficulty": r.difficulty,
                "difficulty_name": if r.difficulty <= 3 { text_of(&words, 1648 + r.difficulty as i32, ["C", "UC", "R", "UR"][r.difficulty as usize]) } else { String::new() },
                "order": r.order,
            })
        })
        .collect();
    let (listed, inspected, question, notice) = LOCAL.with(|l| {
        let l = l.borrow();
        (l.listed, l.inspected, l.question.clone(), l.notice.clone())
    });
    let detail = if craft.detail == crate::craft::Detail::default() || craft.pending == 6 {
        Json::Null
    } else {
        let d = &craft.detail;
        let held = |name: &[u16]| -> i64 {
            let name = wide(name);
            play.dump
                .inventory
                .items
                .iter()
                .filter(|it| it.equip_slot == -1 && it.item.name.eq_ignore_ascii_case(&name))
                .map(|it| it.quantity.max(1))
                .sum()
        };
        json!({
            "recipe": inspected,
            "name": wide(&d.name),
            "description": wide(&d.desc),
            "rarity": d.rarity,
            "count": d.count,
            "level": d.level,
            "ingredients": d.ingredients.iter().map(|(n, c)| json!({"name": wide(n), "need": c, "have": held(n)})).collect::<Vec<_>>(),
            "tools": d.tools.iter().map(|(n, have)| json!({"name": wide(n), "have": *have != 0})).collect::<Vec<_>>(),
        })
    };
    let row_of = |id: i64| craft.items.items.iter().position(|it| it.item.id == id).map_or(0, |i| i + 1);
    let tray: Vec<Json> = craft
        .combine
        .iter()
        .map(|it| json!({"slot": row_of(it.item.id), "id": it.item.id.to_string(), "name": it.item.name}))
        .collect();
    let items: Vec<Json> = craft
        .items
        .items
        .iter()
        .enumerate()
        .filter(|(_, it)| offered(it))
        .map(|(i, it)| {
            json!({
                "slot": i + 1,
                "id": it.item.id.to_string(),
                "name": it.item.name,
                "quantity": it.quantity.max(1),
                "in_tray": craft.combine.iter().filter(|c| c.item.id == it.item.id).count(),
            })
        })
        .collect();
    json!({
        "revision": 0,
        "pending": pending_name(craft.pending),
        "listed_skill": listed,
        "skills": skills,
        "recipes": recipes,
        "detail": detail,
        "tray": tray,
        "items": items,
        "question": question.map(|(text, recipe)| json!({"text": text, "recipe": recipe})),
        "notice": notice.map(|(text, seq)| json!({"text": text, "seq": seq})),
    })
}

/// The server's crafting reply (0x1102), offered here before the game's own reading (the one line in `play.rs`).
/// True when a mod's window took it; false leaves it to the game exactly as it was. The game's own window up
/// always wins: that one reads its own replies.
pub(crate) fn reply(play: &mut Play, data: &[u8]) -> bool {
    if game_window_up(play) || !mod_window_open() {
        return false;
    }
    sync_open(play);
    let words = words_of(play);
    let modes = Modes { recipe: true, experiment: true };
    let outs = play.craft.recv(data, modes, &words, play.session.expansion & net::session::EXPANSION_FLAGS);
    for out in outs {
        match out {
            // a notice the window would have boxed: the mod shows it
            CraftOut::Notify(text, _) | CraftOut::RecipeMessage(text, _) => LOCAL.with(|l| {
                let mut l = l.borrow_mut();
                l.seq += 1;
                let seq = l.seq;
                l.notice = Some((wide(&text), seq));
            }),
            // the experiment's question after it finds a recipe
            CraftOut::Ask(text, p1, p2, _) if p1 == SCRIBE => LOCAL.with(|l| l.borrow_mut().question = Some((wide(&text), p2))),
            CraftOut::Ask(..) | CraftOut::RecipeInspect | CraftOut::OpenRecipe(_) | CraftOut::OpenExperiment | CraftOut::Send(_) => {}
            // what the game says in its chat and word line, whichever window is up (`run_craft`)
            CraftOut::ShowWord(w) => play.show_word(w),
            CraftOut::Chat(t, c) => play.add_chat(t, c),
        }
    }
    native_a8::publish(snapshot(play));
    true
}

#[cfg(test)]
thread_local! {
    /// Every crafting request the actions sent, as the bytes the session was handed (the tests compare them).
    pub(super) static SENT: RefCell<Vec<Vec<u8>>> = const { RefCell::new(Vec::new()) };
}

fn send(play: &mut Play, m: Vec<u8>) {
    #[cfg(test)]
    SENT.with(|s| s.borrow_mut().push(m.clone()));
    play.session.send(&m);
}

/// The caller holds a crafting window of its own.
fn caller_holds_window() -> bool {
    action_broker::current_dispatch_vm()
        .is_some_and(|vm| KINDS.iter().any(|k| window_sessions::get(vm, k).is_some()))
}

pub(crate) fn dispatch(play: &mut Play, action: &A8Action) -> DispatchOutcome {
    let out = dispatch_inner(play, action);
    // what the action changed (the tray, a question answered, a request out) is what a mod reads next
    if mod_window_open() {
        native_a8::publish(snapshot(play));
    }
    out
}

fn dispatch_inner(play: &mut Play, action: &A8Action) -> DispatchOutcome {
    if play.is_dead() {
        return refuse("not_ready", "cannot craft while dead");
    }
    if !caller_holds_window() {
        return refuse(
            "window_closed",
            "open a crafting window first (eqoa.windows.open_session 'recipe_book' or 'experiment'): the game's server replies are read only while one is open",
        );
    }
    if game_window_up(play) {
        return refuse("not_ready", "the game's own crafting window is up");
    }
    sync_open(play);
    refresh_items(play);
    let words = words_of(play);
    match action {
        A8Action::List { .. } | A8Action::Inspect { .. } if LOCAL.with(|l| l.borrow().question.is_some()) => {
            refuse("busy", "answer the keep-this-recipe question first, as the game's window asks")
        }
        A8Action::List { skill } => {
            let Some(m) = play.craft.send6(*skill as i32) else {
                return refuse("busy", "the last crafting request has not been answered yet");
            };
            send(play, m);
            LOCAL.with(|l| {
                let mut l = l.borrow_mut();
                l.listed = Some(*skill);
                l.inspected = None;
            });
            DispatchOutcome::Completed { detail: Some(json!({"sent": true})) }
        }
        A8Action::Inspect { recipe } => {
            if play.craft.find(*recipe).is_none() {
                return refuse("stale_handle", "that recipe is not in the book now");
            }
            let Some(m) = play.craft.send7(*recipe) else {
                return refuse("busy", "the last crafting request has not been answered yet");
            };
            send(play, m);
            LOCAL.with(|l| l.borrow_mut().inspected = Some(*recipe));
            DispatchOutcome::Completed { detail: Some(json!({"sent": true})) }
        }
        A8Action::TrayAdd { slot, id } => match tray_add(&mut play.craft, *slot, *id, &words) {
            Ok(()) => tray_done(&play.craft),
            Err(e) => e,
        },
        A8Action::TrayRemove { slot, id } => {
            let Some(it) = play.craft.items.items.get(*slot) else {
                return refuse("stale_handle", "that row is not in the bags now");
            };
            if it.item.id != *id {
                return refuse("stale_handle", "that row holds a different item now");
            }
            let before = play.craft.combine.len();
            play.craft.remove_from_combine(*slot as i32);
            if play.craft.combine.len() == before {
                return refuse("not_ready", "that item is not in the tray");
            }
            tray_done(&play.craft)
        }
        A8Action::TrayClear => {
            play.craft.combine.clear();
            tray_done(&play.craft)
        }
        A8Action::Fill { picks } => {
            // on a copy, so a refused pick leaves the tray as the player had it
            let mut c = play.craft.clone();
            c.combine.clear();
            for (slot, id) in picks {
                if let Err(e) = tray_add(&mut c, *slot, *id, &words) {
                    return e;
                }
            }
            play.craft.combine = c.combine;
            tray_done(&play.craft)
        }
        A8Action::Combine => {
            if LOCAL.with(|l| l.borrow().question.is_some()) {
                return refuse("busy", "answer the keep-this-recipe question first, as the game's window asks");
            }
            if play.craft.combine.is_empty() {
                return refuse("not_ready", "the tray is empty");
            }
            let inventory = play.dump.inventory.clone();
            let Some(m) = play.craft.send_combine(&inventory) else {
                return refuse("busy", "the last crafting request has not been answered yet");
            };
            send(play, m);
            LOCAL.with(|l| l.borrow_mut().notice = None);
            DispatchOutcome::Completed { detail: Some(json!({"sent": true})) }
        }
        A8Action::Keep { yes } => {
            let Some((_, recipe)) = LOCAL.with(|l| l.borrow().question.clone()) else {
                return refuse("not_ready", "the server is not asking about a recipe");
            };
            if *yes {
                let Some(m) = play.craft.send5(recipe) else {
                    return refuse("busy", "the last crafting request has not been answered yet");
                };
                send(play, m);
            }
            LOCAL.with(|l| l.borrow_mut().question = None);
            DispatchOutcome::Completed { detail: Some(json!({"sent": *yes, "answer": if *yes { "yes" } else { "no" }})) }
        }
    }
}

fn tray_done(craft: &Craft) -> DispatchOutcome {
    DispatchOutcome::Completed { detail: Some(json!({"sent": false, "tray": craft.combine.len()})) }
}

/// One of a bag row into the tray, by the game's own `add_to_combine`: a component only, no more of one item than
/// the stack holds, four in all.
fn tray_add(craft: &mut Craft, slot: usize, id: i64, words: &vi::strtab::StringTable) -> Result<(), DispatchOutcome> {
    let Some(it) = craft.items.items.get(slot) else {
        return Err(refuse("stale_handle", "that row is not in the bags now"));
    };
    if it.item.id != id {
        return Err(refuse("stale_handle", "that row holds a different item now"));
    }
    if !offered(it) {
        return Err(refuse("invalid_argument", "that item is not a crafting component"));
    }
    let before = craft.combine.len();
    let outs = craft.add_to_combine(slot as i32, words);
    if craft.combine.len() == before {
        // the game's red line, 1601 or 1602
        let why = outs
            .into_iter()
            .find_map(|o| if let CraftOut::Chat(t, _) = o { Some(t) } else { None })
            .unwrap_or_else(|| "the item could not be added to the tray".into());
        return Err(refuse("not_ready", why));
    }
    Ok(())
}

#[cfg(test)]
#[path = "a8_play_tests.rs"]
mod tests;
