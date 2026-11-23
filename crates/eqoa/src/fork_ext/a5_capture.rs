// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A5 on `Play`: what the character window's mod needs from the running game (`addons::native_a5`). A child of
//! the Play adapter, so it reads `Play`'s private state.
//!
//! - **Reads** (`refresh`): the faction page and the combat record as the game holds them, handed to `native_a5` as
//!   plain data.
//! - **Class Mastery on a session** (`tick`, `mastery_open`, `node_kept`): a mod that holds a `"mastery"` window
//!   session gets the list the game's own window gets. The list is fetched when the session opens and cleared when it
//!   closes (as the game fetches it at open and clears it at close), and an inspected skill survives without the
//!   game's window (the one upstream line, `fork_hooks::mastery_node_kept`).
//! - **The steps that ask first** (`ask_mastery`, `dispatch`, `carry_out`): spending training points, buying a skill
//!   and switching one off. Each puts the client's own question to the player (`addons::confirm`); only their Yes
//!   reaches `carry_out`, which sends the request the game's own window sends, after checking again that nothing
//!   changed since the question was asked.
use super::Play;
use crate::fork_ext::addons::{
    action_broker::DispatchOutcome,
    confirm::{self, Act},
    native_a5::{self as a5, A5Action},
    window_sessions,
};
use serde_json::json;
use std::cell::Cell;
use std::hash::{Hash, Hasher};

fn refuse(code: &'static str, reason: impl Into<String>) -> DispatchOutcome {
    DispatchOutcome::Refused { code, reason: reason.into() }
}

fn hashed(f: impl FnOnce(&mut std::collections::hash_map::DefaultHasher)) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    f(&mut h);
    h.finish()
}

/// Brings the A5 reads up to this frame. Called once a frame from `updates::refresh` while a mod wants the state.
pub(super) fn refresh(play: &Play) {
    a5::enter_generation();
    if !play.in_world_now() {
        return;
    }
    let f = &play.factions;
    let key = hashed(|h| (f.ready, f.page, f.pages, &f.list).hash(h));
    a5::set_factions(key, || {
        f.ready.then(|| {
            let entries = f.list.iter().map(|(name, standing)| (String::from_utf16_lossy(name), *standing)).collect();
            a5::page_json(f.page, f.pages, entries)
        })
    });
    let record = play.combat_record();
    let key = hashed(|h| {
        if let Some(r) = record {
            r.name.hash(h);
            for row in &r.rows {
                (row.heading, &row.label, &row.value).hash(h);
            }
        }
        record.is_some().hash(h);
    });
    a5::set_record(key, play.session.combat_record, || {
        record.map(|r| {
            json!({
                "name": r.name,
                "rows": r.rows.iter().map(|row| json!({"heading": row.heading, "label": row.label, "value": row.value})).collect::<Vec<_>>(),
            })
        })
    });
}

/// The cap of trainable stat `i`: the level's cap and what the masteries raise it by (the trainer window's own rule).
fn stat_cap(play: &Play, i: usize) -> i32 {
    crate::trainer::stat_cap(play.dump.player.p44 as i32, &play.cm.m.bonuses, i as i32)
}

/// Why these points cannot be spent now, by the trainer window's own rules: no more than the points held, and no
/// stat past its cap.
fn train_refusal(play: &Play, points: &[i32; 7]) -> Option<(&'static str, String)> {
    let have = play.dump.player.p40 as i32;
    let total: i32 = points.iter().sum();
    if total <= 0 {
        return Some(("invalid_argument", "name at least one point to spend".into()));
    }
    if total > have {
        return Some(("out_of_range", format!("you have {have} training points, not {total}")));
    }
    for (i, p) in points.iter().enumerate() {
        if *p > 0 && play.stats.base[i] + p > stat_cap(play, i) {
            return Some((
                "out_of_range",
                format!("{} cannot go past {} (it is {} now)", a5::STAT_NAMES[i], stat_cap(play, i), play.stats.base[i]),
            ));
        }
    }
    None
}

pub(crate) fn dispatch(play: &mut Play, action: &A5Action) -> DispatchOutcome {
    if play.is_dead() {
        return refuse("not_ready", "cannot do this while dead");
    }
    match action {
        A5Action::TrainStats { points } => {
            if let Some((code, why)) = train_refusal(play, points) {
                return refuse(code, why);
            }
            // never at once: the question goes to the player; their own Yes sends it (`carry_out`)
            super::super::a1_play::ask(Act::TrainStats { points: *points }, confirm::train_text(points), "trainer")
        }
        A5Action::RequestFactions { page } => {
            let pages = play.factions.pages;
            if pages > 0 && *page >= pages {
                return refuse("out_of_range", format!("the server has {pages} pages of factions"));
            }
            let m = play.factions.request(*page);
            play.session.send(&m);
            DispatchOutcome::Completed { detail: Some(json!({"sent": true, "page": page + 1})) }
        }
        A5Action::RequestCombatRecord => {
            if !play.session.ask_combat_record() {
                return refuse("unsupported", "this server keeps no combat record");
            }
            DispatchOutcome::Completed { detail: Some(json!({"sent": true})) }
        }
    }
}

/// Whether a Class Mastery list is there to work on: the game's own window is open, or a mod holds a `"mastery"`
/// session (`tick` fetches its list).
pub(crate) fn mastery_open(play: &Play) -> bool {
    play.mode == 40 || window_sessions::any_open("mastery")
}

/// The one upstream seam: a skill the list's reply opened (`Part::recv` sub-op 9) is dropped when no window is up to
/// show it. A mod's session shows it, so it stays (the game's own window, mode 40, decides for itself as before).
pub(crate) fn node_kept(play: &Play) -> bool {
    play.mode != 40 && window_sessions::any_open("mastery")
}

/// The question for buying or switching off the inspected skill. The caller has checked the skill, the pending
/// lock and the Master Class rule.
pub(crate) fn ask_mastery(play: &Play, skill_id: i32, refund: bool, class: u8, parent: i32) -> DispatchOutcome {
    let Some(k) = play.cm.find(skill_id) else {
        return refuse("not_found", "no such skill in the loaded list");
    };
    let skill = &play.cm.skills[k];
    let name = String::from_utf16_lossy(&skill.name);
    let text = confirm::mastery_text(&name, skill.cost, refund, class == crate::classmastery::MASTER_CLASS);
    super::super::a1_play::ask(Act::Mastery { skill_id, name, refund, class, parent }, text, "mastery")
}

/// What a Yes does, as the game's own windows do: after checking again that what the question named still holds.
pub(crate) fn carry_out(play: &mut Play, act: &Act) -> Result<(), String> {
    match act {
        Act::TrainStats { points } => {
            if let Some((_, why)) = train_refusal(play, points) {
                return Err(why);
            }
            let Some(m) = crate::trainer::train_stats(points, play.dump.player.p40 as i32) else {
                return Err("the points changed since the question was asked".into());
            };
            play.session.send(&m);
            Ok(())
        }
        Act::Mastery { skill_id, name, refund, class, parent } => {
            if !mastery_open(play) || play.cm.skills.is_empty() {
                return Err("the Class Mastery list is not loaded".into());
            }
            if play.cm.pending != 0 {
                return Err("a class mastery request is still pending".into());
            }
            let Some(k) = play.cm.find(*skill_id) else {
                return Err("the skill is no longer in the list".into());
            };
            let skill = play.cm.skills[k].clone();
            if String::from_utf16_lossy(&skill.name) != *name {
                return Err("the list holds a different skill there now".into());
            }
            let msg = if *refund {
                if !skill.on {
                    return Err("the skill is not owned".into());
                }
                play.cm.send03(*skill_id)
            } else {
                if skill.on || skill.known == 0 {
                    return Err("the skill is owned or not yet buyable".into());
                }
                if *class == crate::classmastery::MASTER_CLASS && play.cm.refuses_master_class(*skill_id, *parent) {
                    play.warn(1840);
                    return Err("another Master Class is already owned".into());
                }
                play.cm.send02(*skill_id)
            };
            match msg {
                Some(m) => {
                    play.session.send(&m);
                    Ok(())
                }
                None => Err("a class mastery request is still pending".into()),
            }
        }
        Act::Destroy { .. }
        | Act::DropQuest { .. }
        | Act::AcceptOffer { .. }
        | Act::GuildKick { .. }
        | Act::FriendAccept { .. }
        | Act::GuildAccept { .. } => Err("not a character step".into()),
    }
}

/// What the mod's Class Mastery session has done on the game's list.
#[derive(Clone, Copy, Default)]
struct MasterySession {
    /// a session was open at the last step
    open: bool,
    /// the list has been asked for and is waited for (or has come)
    asked: bool,
    /// the list was seen held while the session was open
    had_list: bool,
}

thread_local! {
    static MASTERY: Cell<MasterySession> = const { Cell::new(MasterySession { open: false, asked: false, had_list: false }) };
}

/// Each game step: a mod's `"mastery"` session gets the list when it opens and loses it when it closes, as the game's
/// window fetches the list at open and clears it at close. A list the game's own window closed under an open session
/// is asked for again (once per loss).
pub(crate) fn tick(play: &mut Play) {
    let open = window_sessions::any_open("mastery");
    let mut st = MASTERY.with(Cell::get);
    if !open && !st.open {
        return;
    }
    if open && !st.open {
        st = MasterySession { open: true, asked: play.mode == 40, had_list: false };
    }
    if open {
        if st.asked && play.mode != 40 && st.had_list && play.cm.skills.is_empty() {
            // the game's window closed (clearing the list) while the mod's session was open: ask again
            st.asked = false;
            st.had_list = false;
        }
        if !st.asked
            && let Some(m) = play.cm.open()
        {
            // `Part::open` clears the list first and asks for it (sub-op 4), as the window's open does
            play.session.send(&m);
            st.asked = true;
        }
        if !play.cm.skills.is_empty() {
            st.had_list = true;
        }
    } else {
        // the session ended: the list goes, as the window's close clears it (the game's own window keeps its list)
        if play.mode != 40 {
            play.cm.close();
        }
        st = MasterySession::default();
    }
    st.open = open;
    MASTERY.with(|c| c.set(st));
}

#[cfg(test)]
pub(crate) fn reset_for_test() {
    MASTERY.with(|c| c.set(MasterySession::default()));
}
