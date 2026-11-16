// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round 6 Part A native state capture: quests and class mastery (plus camera and windows in subsequent steps).
use super::Play;
use crate::fork_ext::addons::native_core::{self, CORE, Core, MasteryFlags, View, changed_fields};
use serde_json::{Value as Json, json};
use std::cell::{Cell, RefCell};
use std::hash::{Hash, Hasher};

fn utf16_str(slice: &[u16]) -> String {
    let end = slice.iter().position(|&c| c == 0).unwrap_or(slice.len());
    String::from_utf16_lossy(&slice[..end])
}

#[derive(Clone, PartialEq, Eq)]
struct CmFingerprint {
    world_gen: u64,
    x0c: i64,
    x10: i64,
    x08: i64,
    x14: i64,
    pending: i32,
    stats: [i64; 29],
    bonuses: [i64; 7],
    skills_len: usize,
    skills_hash: u64,
    node_hash: u64,
}

impl Default for CmFingerprint {
    fn default() -> Self {
        Self {
            world_gen: 0,
            x0c: 0,
            x10: 0,
            x08: 0,
            x14: 0,
            pending: 0,
            stats: [0; 29],
            bonuses: [0; 7],
            skills_len: 0,
            skills_hash: 0,
            node_hash: 0,
        }
    }
}

thread_local! {
    static LAST_QUEST_HASH: Cell<Option<u64>> = const { Cell::new(None) };
    static LAST_CM: RefCell<Option<CmFingerprint>> = const { RefCell::new(None) };
}

pub(super) fn forget() {
    LAST_QUEST_HASH.with(|h| h.set(None));
    LAST_CM.with(|c| *c.borrow_mut() = None);
}

fn quests_hash(play: &Play) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    play.quests.titles.len().min(8).hash(&mut h);
    for t in play.quests.titles.iter().take(8) {
        t.hash(&mut h);
    }
    h.finish()
}

fn quests_json(play: &Play) -> Vec<Json> {
    play.quests
        .titles
        .iter()
        .take(8)
        .enumerate()
        .map(|(i, title)| {
            json!({
                "index": i + 1,
                "title": utf16_str(title),
            })
        })
        .collect()
}

fn cm_fingerprint(play: &Play, generation: u64) -> CmFingerprint {
    let mut sh = std::collections::hash_map::DefaultHasher::new();
    play.cm.skills.len().hash(&mut sh);
    for s in &play.cm.skills {
        s.id.hash(&mut sh);
        s.x00.hash(&mut sh);
        s.parent.hash(&mut sh);
        s.category.hash(&mut sh);
        s.cost.hash(&mut sh);
        s.known.hash(&mut sh);
        s.on.hash(&mut sh);
        s.name.hash(&mut sh);
    }
    let mut nh = std::collections::hash_map::DefaultHasher::new();
    if let Some(n) = &play.cm.node {
        true.hash(&mut nh);
        n.id.hash(&mut nh);
        n.x04.hash(&mut nh);
        n.x05.hash(&mut nh);
        n.x08.hash(&mut nh);
        n.x0c.hash(&mut nh);
        n.x10.hash(&mut nh);
        n.x11.hash(&mut nh);
        n.x18.hash(&mut nh);
        n.mins.hash(&mut nh);
        n.name.hash(&mut nh);
        n.title.hash(&mut nh);
        n.text.hash(&mut nh);
        // Selected node cost joins from skills
        play.cm
            .skills
            .iter()
            .find(|s| s.id == n.x08)
            .map(|s| s.cost)
            .hash(&mut nh);
    } else {
        false.hash(&mut nh);
    }
    let m = &play.cm.m;
    CmFingerprint {
        world_gen: generation,
        x0c: m.x0c,
        x10: m.x10,
        x08: m.x08,
        x14: m.x14,
        pending: play.cm.pending,
        stats: m.stats,
        bonuses: m.bonuses,
        skills_len: play.cm.skills.len(),
        skills_hash: sh.finish(),
        node_hash: nh.finish(),
    }
}

fn update_mastery_flags(play: &Play, prev: Option<&CmFingerprint>, now: &CmFingerprint, c: &mut Core) {
    if play.cm_msgs.is_empty() && c.cm_pending_ops != 0 {
        let bits = std::mem::take(&mut c.cm_pending_ops);
        for op in 1u8..=10 {
            if (bits & (1u16 << op)) != 0 {
                c.mastery_flags.apply_subop(op);
            }
        }
    }
    let prev = prev.filter(|p| p.world_gen == now.world_gen);
    match prev {
        Some(p) => {
            if now.x08 != p.x08 {
                c.mastery_flags.share = true;
            }
            if now.x0c != p.x0c || now.x10 != p.x10 {
                c.mastery_flags.available_spent = true;
            }
            if now.x14 != p.x14 {
                c.mastery_flags.progress = true;
            }
            if now.stats != p.stats || now.bonuses != p.bonuses {
                c.mastery_flags.stats = true;
            }
            if now.skills_len > 0 {
                c.mastery_flags.skills = true;
            } else if p.skills_len > 0 && now.skills_len == 0 && (c.cm_pending_ops & (1 << 8)) == 0 {
                // `cm.close()` cleared the skill list
                c.mastery_flags.skills = false;
            }
        }
        None => {
            if now.x08 != 0 {
                c.mastery_flags.share = true;
            }
            if now.x0c != 0 || now.x10 != 0 {
                c.mastery_flags.available_spent = true;
            }
            if now.x14 != 0 {
                c.mastery_flags.progress = true;
            }
            if now.stats != [0; 29] || now.bonuses != [0; 7] {
                c.mastery_flags.stats = true;
            }
            if now.skills_len > 0 {
                c.mastery_flags.skills = true;
            }
        }
    }
}

fn mastery_json(play: &Play, flags: MasteryFlags) -> Json {
    let m = &play.cm.m;
    json!({
        "available": flags.available_spent.then(|| m.x0c.max(0)),
        "spent": flags.available_spent.then(|| m.x10.max(0)),
        "experience_share": flags.share.then(|| ee::div(m.x08 as f32, 100.0)),
        "progress_raw": flags.progress.then_some(m.x14),
        "pending": play.cm.pending != 0,
        "stats": m.stats.as_slice(),
        "bonuses": m.bonuses.as_slice(),
    })
}

fn skills_json(play: &Play, flags: MasteryFlags) -> Option<Vec<Json>> {
    flags.skills.then(|| {
        play.cm
            .skills
            .iter()
            .map(|s| {
                json!({
                    "id": s.id,
                    "parent_key": s.parent,
                    "rank_key": s.x00,
                    "name": utf16_str(&s.name),
                    "category": s.category,
                    "cost": s.cost,
                    "known": s.known != 0,
                    "owned": s.on,
                    "master_class": s.master_class(),
                })
            })
            .collect()
    })
}

fn node_json(play: &Play) -> Option<Json> {
    play.cm.node.as_ref().map(|n| {
        let cost = play.cm.skills.iter().find(|s| s.id == n.x08).map(|s| s.cost);
        json!({
            "id": n.id,
            "skill_id": n.x08,
            "parent_key": n.x0c,
            "name": utf16_str(&n.name),
            "title": utf16_str(&n.title),
            "category": n.x10,
            "cost": cost,
            "text": utf16_str(&n.text),
            "known": n.x05 != 0,
            "owned": n.x04 != 0,
            "level_required": n.x11,
            "points_spent_required": n.x18,
            "minimum_stats": n.mins.as_slice(),
        })
    })
}

/// Populates Round 6 Part A fields into `view` during initial / full `native_view` build.
pub(super) fn populate_view(play: &Play, view: &mut View) {
    let qh = quests_hash(play);
    LAST_QUEST_HASH.with(|h| h.set(Some(qh)));
    view.quests = quests_json(play);

    let flags = CORE.with(|c| {
        let mut c = c.borrow_mut();
        let fp = cm_fingerprint(play, c.generation);
        LAST_CM.with(|prev| {
            let mut prev = prev.borrow_mut();
            update_mastery_flags(play, prev.as_ref(), &fp, &mut c);
            *prev = Some(fp);
        });
        c.mastery_flags
    });
    let m = mastery_json(play, flags);
    if view.player.is_object() {
        view.player["mastery"] = m.clone();
    }
    view.mastery = Some(m);
    view.mastery_skills = skills_json(play, flags);
    view.mastery_node = node_json(play);
}

/// Captures quest log changes after 0x007C / 0x007D packet events.
pub(super) fn capture_quests(play: &Play) {
    let qh = quests_hash(play);
    if LAST_QUEST_HASH.with(|h| h.get()) == Some(qh) {
        return;
    }
    LAST_QUEST_HASH.with(|h| h.set(Some(qh)));
    let mut after = quests_json(play);
    native_core::defer(2048, move |c| {
        c.quest_revision += 1;
        for q in &mut after {
            let key = q["index"].as_u64().unwrap_or(0).to_string();
            q["ref"] = json!({
                "$ref": "quest",
                "key": key,
                "generation": c.generation,
                "incarnation": c.quest_revision,
            });
        }
        let mut before = std::mem::replace(&mut c.view.quests, after);
        if before != c.view.quests {
            let payload = json!({
                "before": before,
                "after": c.view.quests,
                "changed": ["records"],
            });
            c.emit("quests.changed", payload);
        } else {
            let _ = &mut before;
        }
    });
}

/// Incremental per-frame refresh for Round 6 Part A state (zero allocations when unchanged).
pub(super) fn refresh(play: &Play) {
    // 1. Quests (also catches direct mutations to play.quests outside 0x007C/0x007D)
    let qh = quests_hash(play);
    if LAST_QUEST_HASH.with(|h| h.replace(Some(qh))) != Some(qh) {
        let mut after = quests_json(play);
        CORE.with(|c| {
            let mut c = c.borrow_mut();
            c.quest_revision += 1;
            for q in &mut after {
                let key = q["index"].as_u64().unwrap_or(0).to_string();
                q["ref"] = json!({
                    "$ref": "quest",
                    "key": key,
                    "generation": c.generation,
                    "incarnation": c.quest_revision,
                });
            }
            let before = std::mem::replace(&mut c.view.quests, after);
            if before != c.view.quests {
                let payload = json!({
                    "before": before,
                    "after": c.view.quests,
                    "changed": ["records"],
                });
                c.emit("quests.changed", payload);
            }
        });
        super::updates::dirty();
    }

    // 2. Class Mastery (processes after cm_msgs drains in run_class_mastery)
    let (generation, has_pending_ops) = CORE.with(|c| {
        let c = c.borrow();
        (c.generation, play.cm_msgs.is_empty() && c.cm_pending_ops != 0)
    });
    let fp = cm_fingerprint(play, generation);
    let cm_changed = has_pending_ops
        || LAST_CM.with(|prev| prev.borrow().as_ref() != Some(&fp));
    if cm_changed {
        let flags = CORE.with(|c| {
            let mut c = c.borrow_mut();
            LAST_CM.with(|prev| {
                let mut prev = prev.borrow_mut();
                update_mastery_flags(play, prev.as_ref(), &fp, &mut c);
                *prev = Some(fp);
            });
            c.mastery_flags
        });
        let m = mastery_json(play, flags);
        let skills = skills_json(play, flags);
        let node = node_json(play);
        CORE.with(|c| {
            let mut c = c.borrow_mut();
            let before = std::mem::replace(&mut c.view.mastery, Some(m.clone()));
            if c.view.player.is_object() {
                c.view.player["mastery"] = m;
            }
            c.view.mastery_skills = skills;
            c.view.mastery_node = node;
            if before != c.view.mastery {
                let b = before.as_ref().unwrap_or(&Json::Null);
                let a = c.view.mastery.as_ref().unwrap_or(&Json::Null);
                let changed = changed_fields(b, a);
                let payload = json!({
                    "before": before,
                    "after": c.view.mastery,
                    "changed": changed,
                });
                c.emit("mastery.changed", payload);
            }
        });
        super::updates::dirty();
    }
}
