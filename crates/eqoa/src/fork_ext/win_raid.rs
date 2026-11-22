// Copyright LoopRook. SPDX-License-Identifier: AGPL-3.0-or-later
//! Raid ready-check and loot-roll strip takeover hooks (child of `raidplay.rs`, seams 7-10).
//!
//! When a mod claims `raid_ready` or `raid_roll` through the takeover registry, that strip is not
//! drawn natively and takes no native keys or clicks, while its expiration timer keeps running in
//! `Play::run_rolls`. When the holder releases or goes away (disabled, errored, reloaded, or the
//! player's "Use the game's windows" switch), the native strip draws and responds again at once.

use super::Play;

#[cfg(not(target_arch = "wasm32"))]
use crate::fork_ext::addons::window_takeover::{self, TakeoverKind};

/// Refreshes the mod-readable raid, ready-check and loot-roll snapshots from `play`.
pub(crate) fn snapshot_raid(_play: &Play) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use crate::fork_ext::addons::raid_strips::{
            LootRollEntry, RaidMemberSnap, RaidSnapshot, ReadyCheckSnap,
        };
        let now = _play.session.now() as i64;
        let st = &_play.raid.state;
        let in_raid = st.in_raid();
        let loot_rule = match if in_raid { st.rule } else { st.group_rule() } {
            net::raid::RULE_NEED_BEFORE_GREED => "need_before_greed",
            net::raid::RULE_MASTER_LOOTER => "master_looter",
            _ => "free_for_all",
        };
        let members = _play
            .group
            .raid
            .iter()
            .map(|m| RaidMemberSnap {
                id: m.id,
                name: m.name.clone(),
                hp_fraction: ee::div(m.hp as f32, 255.0),
                power_fraction: ee::div(m.power as f32, 255.0),
                group: m.group,
                merc: m.merc,
                leader: st.in_raid() && st.leader == m.id,
                helper: st.in_raid() && st.helper == m.id,
                assist: st.in_raid() && st.assist == m.id,
            })
            .collect();
        let ready = _play.raid.ready.open.as_ref().map(|o| ReadyCheckSnap {
            id: o.id,
            asker: o.asker.clone(),
            seconds_remaining: ee::div((o.ends - now).max(0) as f32, 1000.0),
            duration: ee::div(o.length.max(0) as f32, 1000.0),
            deadline_ms: o.ends,
        });
        let flags = net::session::EXPANSION_FLAGS;
        let rolls = _play
            .raid
            .rolls
            .open
            .iter()
            .map(|o| {
                let icon = crate::inventory::item_icon(&o.item, flags);
                LootRollEntry {
                    id: o.id,
                    item_name: o.item.item.name.clone(),
                    item_id: o.item.item.id.to_string(),
                    icon: (icon as i32 != 0 && icon as i32 != -1).then_some(icon as u32),
                    may_need: o.may_need,
                    seconds_remaining: ee::div((o.ends - now).max(0) as f32, 1000.0),
                    duration: ee::div(o.length.max(0) as f32, 1000.0),
                    deadline_ms: o.ends,
                }
            })
            .collect();
        crate::fork_ext::addons::raid_strips::update(RaidSnapshot {
            in_raid,
            leader_id: (in_raid && st.leader != 0).then_some(st.leader),
            helper_id: (in_raid && st.helper != 0).then_some(st.helper),
            assist_id: (in_raid && st.assist != 0).then_some(st.assist),
            loot_rule,
            in_combat: if in_raid {
                st.in_combat()
            } else {
                st.group_in_combat()
            },
            leads: st.leads(),
            helps: st.helps(),
            members,
            ready,
            rolls,
        });
    }
}

/// Seam 8 in `Play::run_rolls` (after both `expire` calls): suppresses drawing of taken strips
/// while allowing any unclaimed strip to draw normally.
pub(super) fn run_strips(
    _play: &mut Play,
    _vui: &vi::ui::Vui,
    _r: &mut dyn vi::ui::Raster,
    _now: i64,
) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if crate::fork_ext::addons::native_core::capture_requested()
            || window_takeover::has_activity()
        {
            snapshot_raid(_play);
        }
        if !window_takeover::has_activity() {
            return false;
        }
        let ready_taken = window_takeover::is_taken(TakeoverKind::RaidReady);
        let roll_taken = window_takeover::is_taken(TakeoverKind::RaidRoll);
        if !ready_taken && !roll_taken {
            return false;
        }
        if ready_taken {
            _play.raid.ready.let_go();
        }
        if roll_taken {
            _play.raid.rolls.let_go();
        }
        if !ready_taken && matches!(_play.mode, 0 | 33 | 38) {
            _play.raid.ready.draw(_vui, _r, _now);
        }
        if !roll_taken && _play.rolls_shown() {
            _play.raid.rolls.skip = (!ready_taken && _play.raid.ready.is_open()) as usize;
            let flags = net::session::EXPANSION_FLAGS;
            _play
                .raid
                .rolls
                .draw(_vui, _r, _now, &|it| crate::inventory::item_icon(it, flags) as i32);
            if let Some(it) = _play.raid.rolls.lit_item() {
                let attack_words = _play
                    .start
                    .as_ref()
                    .map(|s| s.attack_words.clone())
                    .unwrap_or_default();
                let pl = &_play.dump.player;
                let info = crate::infotext::Info {
                    words: &_vui.words,
                    inv: &_play.dump.inventory,
                    hp: [
                        _play.stats.stat[7],
                        _play.stats.stat[8],
                        _play.stats.stat[9],
                        _play.stats.stat[10],
                    ],
                    base: &_play.stats.base,
                    class: pl.class,
                    level: pl.level,
                    attack_words: &attack_words,
                };
                let text = crate::ui::wide(&info.inspect_item(it));
                let mut w = vi::ui::Window::default();
                let rows = w.count_lines(_vui, _r, &text, 210).clamp(1, 14);
                w.init(_vui, _r, [310, 240], 250, rows, 1);
                w.open(true);
                w.draw(_vui, _r);
                w.text_box(_vui, _r, &text, 20, 0, 210, rows, [1.0; 4], 3);
            }
        }
        true
    }
    #[cfg(target_arch = "wasm32")]
    {
        false
    }
}

/// Seam 9 in `Play::roll_key`: hidden strips take no keys.
pub(super) fn roll_key(_play: &mut Play, _key: i32, _down: bool) -> Option<bool> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if !window_takeover::has_activity() {
            return None;
        }
        let ready_taken = window_takeover::is_taken(TakeoverKind::RaidReady);
        let roll_taken = window_takeover::is_taken(TakeoverKind::RaidRoll);
        if !ready_taken && !roll_taken {
            return None;
        }
        if ready_taken {
            _play.raid.ready.let_go();
        }
        if roll_taken {
            _play.raid.rolls.let_go();
        }
        if ready_taken && roll_taken {
            return Some(false);
        }
        if !ready_taken {
            let ready = &mut _play.raid.ready;
            if ready.focused() || _down && matches!(_key, 137 | 18) && ready.is_open() {
                if !ready.focused() {
                    ready.take_focus();
                    _play.cues.push(crate::sound::Cue::Ui(11));
                    return Some(true);
                }
                if !matches!(_key, 136 | 137 | 17 | 18 | 134 | 13 | 132 | 27) {
                    return Some(matches!(_key, 138 | 139 | 19 | 20));
                }
                if !_down {
                    return Some(!matches!(_key, 134 | 13));
                }
                let mut cues = Vec::new();
                let made = ready.key(_key, &mut cues);
                _play.cues.extend(cues.into_iter().map(crate::sound::Cue::Ui));
                if let Some((id, yes)) = made {
                    _play.make_ready(id, yes);
                }
                return Some(true);
            }
            return Some(false);
        }
        // Only ready is taken; handle rolls natively.
        let rolls = &mut _play.raid.rolls;
        if !rolls.focused() {
            if _down && matches!(_key, 137 | 18) && rolls.take_focus() {
                _play.cues.push(crate::sound::Cue::Ui(11));
                return Some(true);
            }
            return Some(false);
        }
        if !matches!(
            _key,
            136 | 137 | 138 | 139 | 17 | 18 | 19 | 20 | 134 | 13 | 132 | 27
        ) {
            return Some(false);
        }
        if !_down {
            return Some(!matches!(_key, 134 | 13));
        }
        let mut cues = Vec::new();
        let made = rolls.key(_key, &mut cues);
        _play.cues.extend(cues.into_iter().map(crate::sound::Cue::Ui));
        if let Some((id, c)) = made {
            _play.make_roll(id, c);
        }
        Some(true)
    }
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
}

/// Seam 10 in `Play::roll_point`: hidden strips take no clicks.
pub(super) fn roll_point(_play: &mut Play, _f: [f32; 2]) -> Option<bool> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if !window_takeover::has_activity() {
            return None;
        }
        let ready_taken = window_takeover::is_taken(TakeoverKind::RaidReady);
        let roll_taken = window_takeover::is_taken(TakeoverKind::RaidRoll);
        if !ready_taken && !roll_taken {
            return None;
        }
        if ready_taken && roll_taken {
            return Some(false);
        }
        if !ready_taken && matches!(_play.mode, 0 | 33 | 38) && _play.raid.ready.covers(_f) {
            if let Some((id, yes)) = _play.raid.ready.point(_f) {
                _play.cues.push(crate::sound::Cue::Ui(13));
                _play.make_ready(id, yes);
            }
            return Some(true);
        }
        if !roll_taken && _play.rolls_shown() && _play.raid.rolls.covers(_f) {
            if let Some((id, c)) = _play.raid.rolls.point(_f) {
                _play.cues.push(crate::sound::Cue::Ui(13));
                _play.make_roll(id, c);
            }
            return Some(true);
        }
        Some(false)
    }
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
}

/// Optional guard in `Play::rolls_up`: taken strips do not count as native strips up.
pub(super) fn rolls_up(_play: &Play) -> Option<bool> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if !window_takeover::has_activity() {
            return None;
        }
        let ready_taken = window_takeover::is_taken(TakeoverKind::RaidReady);
        let roll_taken = window_takeover::is_taken(TakeoverKind::RaidRoll);
        if !ready_taken && !roll_taken {
            return None;
        }
        Some(
            (!roll_taken && _play.raid.rolls.is_open())
                || (!ready_taken && _play.raid.ready.is_open()),
        )
    }
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
}

/// Answers an open ready check through `Play`'s private `make_ready`.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn answer_ready(
    play: &mut Play,
    id: u32,
    ready: bool,
) -> std::result::Result<(), (&'static str, String)> {
    if !play.session.ready_offer {
        return Err((
            "not_ready",
            "this server does not offer ready checks".into(),
        ));
    }
    let Some(open) = play.raid.ready.open.as_ref() else {
        return Err(("not_found", "no ready check is open".into()));
    };
    if open.id != id {
        return Err((
            "stale_handle",
            format!("ready check {id} is not the active check ({})", open.id),
        ));
    }
    play.raid.ready.close();
    play.make_ready(id, ready);
    snapshot_raid(play);
    Ok(())
}

/// Answers an open loot roll through `Play`'s private `make_roll`.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn answer_roll(
    play: &mut Play,
    id: u32,
    choice: net::raid::Choice,
) -> std::result::Result<(), (&'static str, String)> {
    if !play.session.raid_offer {
        return Err(("not_ready", "this server does not offer raid rolls".into()));
    }
    if play.raid.rolls.open.is_empty() {
        return Err(("not_found", "no loot roll is open".into()));
    }
    let Some(open) = play.raid.rolls.open.iter().find(|o| o.id == id) else {
        return Err((
            "stale_handle",
            format!("loot roll {id} is no longer open"),
        ));
    };
    if choice == net::raid::Choice::Need && !open.may_need {
        return Err((
            "not_ready",
            "cannot choose Need on this loot roll".into(),
        ));
    }
    play.raid.rolls.close(id);
    play.make_roll(id, choice);
    snapshot_raid(play);
    Ok(())
}
