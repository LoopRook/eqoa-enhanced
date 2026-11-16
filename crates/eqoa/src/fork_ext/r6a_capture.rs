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

#[derive(Clone, Copy, PartialEq, Eq)]
struct CameraFingerprint {
    world_gen: u64,
    eye_pos: [u32; 3],
    eye_dir: [u32; 3],
    orbit_x: u32,
    orbit_y: u32,
    dist: u32,
    mode: i32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct WindowsFingerprint {
    world_gen: u64,
    mode: i32,
    start_visible: bool,
    items_visible: bool,
    map_open: bool,
    search_open: bool,
    typing_hash: u64,
    edit_hash: u64,
    mbox_hash: u64,
    words_present: bool,
    merchant_mode: i32,
    merchant_visible: bool,
}

thread_local! {
    static LAST_QUEST_HASH: Cell<Option<u64>> = const { Cell::new(None) };
    static LAST_CM: RefCell<Option<CmFingerprint>> = const { RefCell::new(None) };
    static LAST_CAMERA: Cell<Option<CameraFingerprint>> = const { Cell::new(None) };
    static LAST_WINDOWS: Cell<Option<WindowsFingerprint>> = const { Cell::new(None) };
}

pub(super) fn forget() {
    LAST_QUEST_HASH.with(|h| h.set(None));
    LAST_CM.with(|c| *c.borrow_mut() = None);
    LAST_CAMERA.with(|c| c.set(None));
    LAST_WINDOWS.with(|w| w.set(None));
}

pub(super) fn mode_name(mode: i32) -> &'static str {
    match mode {
        0 => "normal",
        1 => "loot",
        2 => "input",
        4 => "gm_goto",
        5 => "spellbook",
        6 => "recipe_book",
        7 => "response_dialog",
        9 => "text_dialog",
        10 => "merchant",
        11 => "bank",
        12 => "trainer",
        13 => "start_menu",
        14 => "confirm",
        15 => "trade",
        16 => "trade_ask",
        17 => "buddy_request",
        18 => "buddy_list",
        19 => "blacksmith",
        20 => "guild_invite",
        21 => "guild",
        22 => "quest_log",
        23 => "mail_compose",
        24 => "mail_inbox",
        26 => "ignore_list",
        27 => "gm_buff_pc",
        28 => "mail_tell",
        29 => "mail_reply",
        30 => "quick_chat",
        31 => "support_request",
        32 => "support_inbox",
        33 => "chat_history",
        34 => "options",
        35 => "who",
        36 => "fishing",
        37 => "experiment",
        38 => "manual_camera",
        39 => "auction_search",
        40 => "mastery",
        41 => "factions",
        42 => "group_search",
        43 => "duel_invite",
        44 => "duel",
        60 => "combat_record",
        _ => "other",
    }
}

fn mode_title(mode: i32, edit_title: &str) -> Option<&str> {
    match mode {
        1 => Some("Loot"),
        2 => Some(if edit_title.is_empty() { "Input" } else { edit_title }),
        4 => Some("Go To"),
        5 => Some("Abilities"),
        6 => Some("Recipe Book"),
        7 | 9 => Some("Dialog"),
        10 => Some("Merchant"),
        11 => Some("Bank"),
        12 => Some("Character Summary"),
        13 => Some("Start Menu"),
        14 => Some("Confirm"),
        15 | 16 => Some("Trade"),
        17 => Some("Buddy Request"),
        18 => Some("Buddy List"),
        19 => Some("Blacksmith"),
        20 => Some("Guild Invite"),
        21 => Some("Guild"),
        22 => Some("Quest Journal"),
        23 | 24 | 28 | 29 => Some("Mail"),
        26 => Some("Ignore List"),
        27 => Some("Buff PC"),
        30 => Some("Quick Chat"),
        31 | 32 => Some("Support"),
        33 => Some("Chat History"),
        34 => Some("Options"),
        35 => Some("Who"),
        36 => Some("Fishing"),
        37 => Some("Experiment"),
        39 => Some("Auction Search"),
        40 => Some("Class Mastery"),
        41 => Some("Factions"),
        42 => Some("Group Search"),
        43 | 44 => Some("Duel"),
        60 => Some("Combat Record"),
        _ => None,
    }
}

fn windows_fingerprint(play: &Play, generation: u64) -> WindowsFingerprint {
    let mut th = std::collections::hash_map::DefaultHasher::new();
    play.typing.hash(&mut th);
    let mut eh = std::collections::hash_map::DefaultHasher::new();
    if play.mode == 2 {
        play.edit.purpose.hash(&mut eh);
        play.edit.password.hash(&mut eh);
        play.edit.title.hash(&mut eh);
        play.edit.text.hash(&mut eh);
    }
    let mut mh = std::collections::hash_map::DefaultHasher::new();
    if play.mode == 14 {
        play.mbox.kind.hash(&mut mh);
        play.mbox.text.hash(&mut mh);
    }
    WindowsFingerprint {
        world_gen: generation,
        mode: play.mode,
        start_visible: play.start.as_ref().is_some_and(|s| s.menu.visible),
        items_visible: play.items.visible,
        map_open: play.map.open,
        search_open: play.search.open,
        typing_hash: th.finish(),
        edit_hash: eh.finish(),
        mbox_hash: mh.finish(),
        words_present: play.words.is_some(),
        merchant_mode: play.npcw.merchant.mode,
        merchant_visible: play.npcw.merchant.visible,
    }
}

fn hint_entry(button: &str, label: &str) -> Json {
    json!({"button": button, "label": label})
}

fn mode_hints(play: &Play, mode: i32) -> Vec<Json> {
    let Some(words) = play.words.as_ref() else {
        return Vec::new();
    };
    let word = |id: i32| -> Option<&str> {
        let s = words.get(id, 0);
        (!s.is_empty() && s != "(null)").then_some(s)
    };
    match mode {
        12 | 13 => {
            let mut out = Vec::new();
            if let Some(s) = word(212) {
                out.push(hint_entry("cross", s));
            }
            if let Some(e) = word(161) {
                out.push(hint_entry("triangle", e));
            }
            out
        }
        10 if play.npcw.merchant.visible => word(161)
            .map(|e| vec![hint_entry("triangle", e)])
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn windows_json(play: &Play) -> Json {
    let start_menu_open =
        play.mode == 13 || play.start.as_ref().is_some_and(|s| s.menu.visible);
    let mut windows: Vec<Json> = Vec::new();
    let push_wnd = |windows: &mut Vec<Json>,
                    key: String,
                    kind: &str,
                    title: Option<&str>,
                    hints: Vec<Json>,
                    dialog: Json| {
        windows.push(json!({
            "_key": key,
            "kind": kind,
            "title": title,
            "visible": true,
            "focused": false,
            "rows": Json::Null,
            "selected_row": Json::Null,
            "scroll_top": Json::Null,
            "hints": hints,
            "dialog": dialog,
        }));
    };

    if play.mode != 0 && play.mode != 38 {
        if play.mode == 13 {
            push_wnd(
                &mut windows,
                "mode:13".into(),
                "start_menu",
                Some("Start Menu"),
                mode_hints(play, 13),
                Json::Null,
            );
            if play.items.visible {
                push_wnd(
                    &mut windows,
                    "sub:inventory".into(),
                    "inventory",
                    Some("Inventory"),
                    Vec::new(),
                    Json::Null,
                );
            }
        } else {
            let kind = mode_name(play.mode);
            let edit_title = if play.mode == 2 {
                utf16_str(&play.edit.title)
            } else {
                String::new()
            };
            let title = mode_title(play.mode, &edit_title);
            let dialog = match play.mode {
                2 => json!({
                    "kind": "input",
                    "text": (!play.edit.password).then(|| utf16_str(&play.edit.text)),
                    "tag": play.edit.purpose,
                    "title": edit_title,
                    "chat": play.edit.purpose == 0xB0F_0000,
                    "password": play.edit.password,
                }),
                14 => {
                    let style = match play.mbox.kind {
                        0 => "ok",
                        1 => "yes_no",
                        2 => "cancel",
                        _ => "other",
                    };
                    json!({
                        "kind": "confirm",
                        "text": utf16_str(&play.mbox.text),
                        "style": style,
                        "style_id": play.mbox.kind,
                        "selection": (play.mbox.kind == 0 || play.mbox.kind == 2).then_some(1u64),
                    })
                }
                // `PrivChat` (mode 9) and `Response` (mode 7) dialog contents remain nil
                // until the read-only accessors in `round-6-seams.md` are approved.
                _ => Json::Null,
            };
            let key = if kind == "other" {
                format!("mode:other:{}", play.mode)
            } else {
                format!("mode:{}", play.mode)
            };
            push_wnd(
                &mut windows,
                key,
                kind,
                title,
                mode_hints(play, play.mode),
                dialog,
            );
        }
    } else if start_menu_open {
        push_wnd(
            &mut windows,
            "overlay:start_menu".into(),
            "start_menu",
            Some("Start Menu"),
            mode_hints(play, 13),
            Json::Null,
        );
    }

    if play.map.open {
        push_wnd(
            &mut windows,
            "overlay:map".into(),
            "map",
            Some("Map"),
            Vec::new(),
            Json::Null,
        );
    }
    if play.search.open {
        push_wnd(
            &mut windows,
            "overlay:search".into(),
            "search",
            Some("Player Search"),
            Vec::new(),
            Json::Null,
        );
    }
    if let Some(text) = &play.typing {
        push_wnd(
            &mut windows,
            "overlay:typing".into(),
            "typing",
            Some("Chat"),
            Vec::new(),
            json!({
                "kind": "input",
                "text": text,
                "tag": 0xB0F_0000u32,
                "title": "",
                "chat": true,
                "password": false,
            }),
        );
    }

    if let Some(last) = windows.last_mut() {
        last["focused"] = json!(true);
    }
    let any_window_open = !windows.is_empty();
    let busy = (play.mode != 0 && play.mode != 38) || start_menu_open || any_window_open;
    json!({
        "mode": mode_name(play.mode),
        "native_mode": play.mode,
        "busy": busy,
        "start_menu_open": start_menu_open,
        "any_window_open": any_window_open,
        "active": Json::Null,
        "windows": windows,
    })
}

fn camera_fingerprint(w: &crate::walker::Walker, generation: u64) -> CameraFingerprint {
    let ep = w.eye.position;
    let ed = w.eye.direction;
    CameraFingerprint {
        world_gen: generation,
        eye_pos: [ep.x.0.to_bits(), ep.y.0.to_bits(), ep.z.0.to_bits()],
        eye_dir: [ed.x.0.to_bits(), ed.y.0.to_bits(), ed.z.0.to_bits()],
        orbit_x: w.cam.cur.orbit.x.0.to_bits(),
        orbit_y: w.cam.cur.orbit.y.0.to_bits(),
        dist: w.cam.cur.dist.0.to_bits(),
        mode: w.cam.camera_mode,
    }
}

fn camera_values(w: &crate::walker::Walker) -> (Json, f32) {
    let p = w.eye.position;
    let d = w.eye.direction;
    let angle = (-d.x).atan2f(-d.z).0;
    let wrapped = if ee::lt(angle, 0.0) {
        ee::add(angle, std::f32::consts::TAU)
    } else {
        angle
    };
    let compass = if wrapped == 0.0 { 0.0 } else { wrapped };
    let heading = w.cam.cur.orbit.x.0.rem_euclid(std::f32::consts::TAU);
    let cam = json!({
        "position": {"x": p.x.0, "y": p.y.0, "z": p.z.0},
        "heading": heading,
        "pitch": w.cam.cur.orbit.y.0,
        "distance": w.cam.cur.dist.0,
        "mode": w.cam.camera_mode,
    });
    (cam, compass)
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
pub(super) fn populate_view(
    play: &Play,
    walker: Option<&crate::walker::Walker>,
    view: &mut View,
) {
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
        if let Some(w) = walker {
            let cfp = camera_fingerprint(w, c.generation);
            LAST_CAMERA.with(|lc| lc.set(Some(cfp)));
            let (cam, compass) = camera_values(w);
            view.camera = cam;
            view.camera_compass = Some(compass);
        } else {
            LAST_CAMERA.with(|lc| lc.set(None));
            view.camera = Json::Null;
            view.camera_compass = None;
        }
        let wfp = windows_fingerprint(play, c.generation);
        LAST_WINDOWS.with(|lw| lw.set(Some(wfp)));
        view.windows_state = windows_json(play);
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
pub(super) fn refresh(play: &Play, walker: Option<&crate::walker::Walker>) {
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

    // 3. Camera
    if let Some(w) = walker {
        let cfp = camera_fingerprint(w, generation);
        if LAST_CAMERA.with(|lc| lc.replace(Some(cfp))) != Some(cfp) {
            let (cam, compass) = camera_values(w);
            CORE.with(|c| {
                let mut c = c.borrow_mut();
                c.view.camera_compass = Some(compass);
                let before = std::mem::replace(&mut c.view.camera, cam);
                if before != c.view.camera {
                    c.camera_changed(before);
                }
            });
            super::updates::dirty();
        }
    }

    // 4. Windows
    let wfp = windows_fingerprint(play, generation);
    if LAST_WINDOWS.with(|lw| lw.replace(Some(wfp))) != Some(wfp) {
        let mut after = windows_json(play);
        CORE.with(|c| {
            let mut c = c.borrow_mut();
            c.assign_window_refs(&mut after);
            let before = std::mem::replace(&mut c.view.windows_state, after);
            if before != c.view.windows_state {
                let changed = changed_fields(&before, &c.view.windows_state);
                let payload = json!({
                    "before": before,
                    "after": c.view.windows_state,
                    "changed": changed,
                });
                c.emit("windows.changed", payload);
            }
        });
        super::updates::dirty();
    }
}
