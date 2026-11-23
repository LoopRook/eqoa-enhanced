// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A3 capture: what `Play` holds that a frame mod needs, handed to `addons::native_a3` as plain data, and the
//! dispatch of the small actions (`addons::frame_actions`). A child of the Play adapter, so it reads `Play`'s private
//! state; it adds no line to any upstream file.
use super::Play;
use crate::fork_ext::addons::{
    action_broker::DispatchOutcome,
    frame_actions::{EMOTES, FrameAction},
    native_a3::{self as a3, BuffLine, Goal, PetLine, RaidRole, TrackedQuest},
};
use serde_json::{Value as Json, json};
use std::cell::Cell;
use std::hash::{Hash, Hasher};

/// A hash of what a build is made from, taken without allocating: a frame whose source is as it was builds nothing.
fn hashed(f: impl FnOnce(&mut std::collections::hash_map::DefaultHasher)) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    f(&mut h);
    h.finish()
}

thread_local! {
    /// A jump the broker accepted, for the walk to be asked at the next frame (`apply_jump`).
    static PENDING_JUMP: Cell<bool> = const { Cell::new(false) };
}

/// The target's description. The title and guild are the server's words and come with the target-info reply; the
/// consider colour is `TrackTarget`'s ring colour (`rings::CON_COLORS`) by the reply's second byte.
fn target_json(play: &Play) -> Json {
    let id = play.target;
    if matches!(id, 0 | u32::MAX) {
        return Json::Null;
    }
    let known = play.target_info.is_some();
    let text = |s: &str| (known && !s.is_empty()).then(|| s.to_string());
    let consider = known.then(|| {
        let code = play.rings.con.unwrap_or(2);
        const NAMES: [&str; 6] = ["red", "yellow", "grey", "blue", "light_blue", "green"];
        match crate::rings::CON_COLORS.get(code as usize) {
            Some(rgb) => json!({
                "code": code,
                "name": NAMES[code as usize],
                "color": {
                    "r": f64::from(rgb[0]) / 255.0,
                    "g": f64::from(rgb[1]) / 255.0,
                    "b": f64::from(rgb[2]) / 255.0,
                    "a": 1.0,
                },
            }),
            None => json!({"code": code}),
        }
    });
    json!({
        "id": id,
        "title": text(&play.target_title),
        "guild": text(&play.target_guild),
        "consider": consider,
    })
}

/// What the player's own ability list says an effect does: the description of the ability with the effect's name
/// (the effect's name is cut at 31 characters, `autocast::effect_is`). None where the player has no such ability
/// (an effect of someone else's spell) or it has no description.
fn ability_description(play: &Play, effect: &str) -> Option<String> {
    play.abilities
        .values()
        .find(|a| !a.desc.is_empty() && crate::autocast::effect_is(effect, &a.name))
        .map(|a| a.desc.clone())
}

fn raid_roles(play: &Play) -> Vec<RaidRole> {
    play.group
        .raid
        .iter()
        .map(|m| RaidRole {
            id: m.id,
            name: m.name.clone(),
            group: m.group,
            class: m.class,
            merc: m.merc,
            marks: play.raid_marks(m.id),
        })
        .collect()
}

/// Brings the A3 state up to this frame. Called once a frame from `updates::refresh` while a mod wants the state.
pub(super) fn refresh(play: &Play, walker: Option<&crate::walker::Walker>) {
    a3::enter_generation();
    if !play.in_world_now() {
        return;
    }
    let now = a3::now_tick();
    let session = &play.session;

    let ends = |left_ms: u32| a3::ticks_of_ms(u64::from(left_ms));
    let buffs_available = a3::set_arrived(false, play.buff_window_on(), session.buffs_at);
    a3::sync_buffs(buffs_available, session.buffs_at, || {
        // what is left of each line now, as the game's own window counts it (the set may have come frames ago)
        session
            .buffs_left(session.now())
            .iter()
            .map(|l| BuffLine {
                whose: l.whose,
                id: l.id,
                icon: l.icon,
                removable: l.removable,
                description: ability_description(play, &l.name),
                name: l.name.clone(),
                ends_tick: (l.left_ms != net::buffs::NO_END).then(|| now + ends(l.left_ms)),
            })
            .collect()
    });
    let pet_available = a3::set_arrived(
        true,
        session.pet_effects_offer && crate::settings::pet_effects(),
        session.pet_effects_at,
    );
    a3::sync_pet(pet_available, session.pet_effects_at, || {
        let gone = session.now().wrapping_sub(session.pet_effects_at);
        let gone = if (gone as i32) < 0 { 0 } else { gone };
        session
            .pet_effects
            .iter()
            .filter(|e| e.left_ms > gone)
            .map(|e| PetLine { icon: e.icon, name: e.name.clone(), ends_tick: now + ends(e.left_ms - gone) })
            .collect()
    });
    let goals_hash = hashed(|h| {
        for q in &session.quest_progress {
            (&q.name, q.ability).hash(h);
            for g in &q.goals {
                (&g.text, g.have, g.need).hash(h);
            }
        }
    });
    a3::sync_goals(session.quest_progress_offer && crate::settings::quest_tracker(), goals_hash, || {
        session
            .quest_progress
            .iter()
            .map(|q| TrackedQuest {
                name: q.name.clone(),
                ability: q.ability,
                goals: q.goals.iter().map(|g| Goal { text: g.text.clone(), have: g.have, need: g.need }).collect(),
            })
            .collect()
    });
    a3::set_training(Some((
        play.dump.player.p40,
        play.dump.player.p44,
        std::array::from_fn(|i| i64::from(crate::trainer::stat_cap(play.dump.player.p44 as i32, &play.cm.m.bonuses, i as i32))),
    )));
    a3::set_breath(Some((
        f64::from(ee::div(play.dump.player.p30 as f32, 255.0)),
        walker.map(|w| w.cam.walk.is_under_liquid()),
    )));
    let target_hash = hashed(|h| (play.target, play.target_info, play.rings.con, &play.target_title, &play.target_guild).hash(h));
    a3::set_target(target_hash, || target_json(play));
    let raid_hash = hashed(|h| {
        for m in &play.group.raid {
            (m.id, &m.name, m.group, m.class, m.merc, play.raid_marks(m.id)).hash(h);
        }
    });
    a3::set_raid(raid_hash, || raid_roles(play));
    a3::set_watched(play.target);
    a3::expire_casts(
        |id| !play.entities.slots.iter().flatten().any(|e| e.id == id),
        play.death.is_some(),
    );
}

/// Whether `effect` is one of the melee swings' effects (`Attack1`..`Attack4`, `Range1`..`Range4`: what
/// `vi::swing::swing_effect` names for each swing). The server announces a melee round with a 0x00B1 as it does a
/// spell; the client plays the swing's effect for it, so the effect id is what tells a swing from a cast.
pub(crate) fn is_melee_swing(effect: u32) -> bool {
    effect != 0 && (0..=11).any(|k| vi::swing::swing_effect(k) == effect)
}

/// A game event on its way into `Play`: a cast the server announces (0x00B1) is reported as `cast.started`. A melee
/// round comes in the same packet (its time is the swing's delay, 3 to 6 seconds) and is not a cast: it starts no cast
/// bar and no event. The target's (and the player's own) casts and ignored rounds are written to eqoa.log.
pub(super) fn note_event(play: &Play, event: &net::session::Event) {
    let net::session::Event::Opcode { op: 0x00B1, data } = event else { return };
    let Some(c) = crate::spells::cast_of(0x00B1, data) else { return };
    let own = Some(c.caster) == play.self_id32();
    if is_melee_swing(c.effect) {
        if own || c.caster == play.target {
            eprintln!(
                "frames: melee round, not a cast: caster={:#x} target={:#x} effect={:#010x} time={}ms own={own} src=0x00B1",
                c.caster, c.target, c.effect, c.time
            );
        }
        return;
    }
    a3::cast_started(c.caster, c.target, c.targets, c.effect, u32::try_from(c.time.max(0)).unwrap_or(0), own);
}

/// The walk is asked for the jump a mod queued (the game's own jump key does this, `Play::jump_key`). Called from the
/// camera hook, which has the walker.
pub(crate) fn apply_jump(play: &mut Play, walker: &mut crate::walker::Walker) {
    if PENDING_JUMP.with(|p| p.take()) && play.session.jumps {
        play.jump_key(walker);
    }
}

#[cfg(test)]
pub(crate) fn jump_pending() -> bool {
    PENDING_JUMP.with(|p| p.get())
}

fn refuse(code: &'static str, reason: impl Into<String>) -> DispatchOutcome {
    DispatchOutcome::Refused { code, reason: reason.into() }
}

fn sent() -> DispatchOutcome {
    DispatchOutcome::Completed { detail: Some(json!({"sent": true, "confirmed": false})) }
}

/// Why `InviteEntity` would not send: the game's own checks, read before it runs (it answers in the chat, not in a
/// result).
fn invite_refusal(play: &Play) -> Option<(&'static str, String)> {
    let id = play.target;
    if matches!(id, 0 | u32::MAX) {
        return Some(("not_ready", "no target to invite".into()));
    }
    let Some(e) = play.entities.slots.iter().flatten().find(|e| e.id == id) else {
        return Some(("not_found", "the target is not in view".into()));
    };
    let Some(c) = e.creature.as_ref() else {
        return Some(("not_ready", "nothing is known of the target yet".into()));
    };
    if c.p256 != 2 {
        return Some(("not_ready", "only a player can be invited".into()));
    }
    if c.p264 == 0 {
        return Some(("not_ready", "that player cannot be invited".into()));
    }
    if Some(id) == play.self_id32() {
        return Some(("invalid_argument", "cannot invite yourself".into()));
    }
    if !(!play.group.in_group || play.group.leader || play.members_invite()) {
        return Some(("not_ready", "only the group's leader can invite".into()));
    }
    if play.group.members.iter().take(play.group.count()).any(|m| m.id == id) {
        return Some(("not_ready", "that player is already in the group".into()));
    }
    None
}

pub(crate) fn dispatch(play: &mut Play, action: &FrameAction) -> DispatchOutcome {
    if play.is_dead() {
        return refuse("not_ready", "cannot do this while dead");
    }
    if !matches!(play.mode, 0 | 38) {
        return refuse("not_ready", format!("cannot do this while a window is up (mode {})", play.mode));
    }
    match action {
        FrameAction::GroupInvite => {
            if let Some((code, reason)) = invite_refusal(play) {
                return refuse(code, reason);
            }
            play.group_action(0x0701_0000);
            sent()
        }
        FrameAction::PetAttack | FrameAction::PetBackOff => {
            if play.pet_id == u32::MAX {
                return refuse("not_ready", "you have no pet");
            }
            if matches!(action, FrameAction::PetAttack) {
                if matches!(play.target, 0 | u32::MAX) {
                    return refuse("not_ready", "no target for the pet to attack");
                }
                if Some(play.target) == play.self_id32() || play.target == play.pet_id {
                    return refuse("invalid_argument", "the pet cannot attack that");
                }
                play.pet_command(PET_ATTACK);
            } else {
                play.pet_command(PET_BACK_OFF);
            }
            sent()
        }
        FrameAction::Emote { index } => {
            let Some(_) = EMOTES.get(*index) else {
                return refuse("invalid_argument", "unknown emote");
            };
            play.do_action(0x0601_0000 + ((*index as u32) << 16), 0);
            DispatchOutcome::Completed { detail: Some(json!({"emote": EMOTES[*index]})) }
        }
        FrameAction::Jump => {
            if !play.session.jumps {
                return refuse("unsupported", "this server does not offer jumping");
            }
            PENDING_JUMP.with(|p| p.set(true));
            DispatchOutcome::Completed {
                detail: Some(json!({"asked": true, "confirmed": false})),
            }
        }
        FrameAction::RemoveEffect { whose, id, icon, name } => {
            let now = play.session.now();
            let Some(line) = play
                .session
                .buffs_left(now)
                .into_iter()
                .find(|l| (l.whose, l.id, l.icon) == (*whose, *id, *icon))
            else {
                return refuse("stale_handle", format!("{name} is no longer on the list"));
            };
            if !line.removable {
                return refuse("not_ready", "the server does not let this effect be taken off");
            }
            if !play.buff_window_on() {
                return refuse("unavailable", "Buff Window is off");
            }
            eprintln!("play: buff {:?} (id {}) asked off by a mod", line.name, line.id);
            if !play.session.send_buff_remove(&line) {
                return refuse("unsupported", "this server has no buff window");
            }
            sent()
        }
    }
}

/// The pet commands the start menu's "Pet Attack" (op 0x41, command 8 with the target) and "Pet Backoff" (command 7)
/// run (`quickchat::COMMANDS`; their words are 1007 and 1006 on the disc).
const PET_ATTACK: u32 = 0x050F_1000;
const PET_BACK_OFF: u32 = 0x050F_0000;
