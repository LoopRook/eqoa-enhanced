//! Small diagnostic snapshot; a child of Play's fork adapter, never game logic.
use super::Play;
use crate::fork_ext::{addons::visibility, report};
use crate::walker::Walker;
use std::fmt::Write;

pub(super) fn describe(play: &Play, walker: Option<&Walker>) -> String {
    let me = play.self_id32();
    let own = play
        .entities
        .slots
        .iter()
        .flatten()
        .find(|e| Some(e.id) == me);
    let position = walker
        .map(|w| w.cam.walk.phys.position.to_array())
        .or_else(|| play.self_at.map(|p| p.0))
        .or_else(|| own.map(|e| e.pos));
    let heading = walker
        .map(|w| w.cam.walk.phys.angles.x.0)
        .or_else(|| own.map(|e| e.hpr[0]));
    let world = play.world_id();
    let mut text = format!(
        "{}\nCharacter: {}\nWorld: {world}\n",
        report::enhanced_label(),
        play.player.name
    );
    if let Some(pos) = position {
        let _ = writeln!(
            text,
            "Position: {:.2}, {:.2}, {:.2}",
            pos[0], pos[1], pos[2]
        );
    } else {
        text.push_str("Position: unavailable\n");
    }
    let _ = writeln!(
        text,
        "Heading (radians): {}",
        heading.map_or_else(|| "unavailable".into(), |h| format!("{h:.3}"))
    );
    if let Some(w) = walker {
        let (_, loc) = play.start_view(w);
        if loc.cell > 0.0 && (loc.cols > 1 || loc.rows > 1) {
            let col = (loc.pos[0] / loc.cell) as i32 - 1;
            let row = (loc.pos[2] / loc.cell) as i32 - 1;
            let zone = crate::zones::zone(world, row, col);
            let name = zone.and_then(|id| play.words.as_ref().map(|words| words.get(id as i32, 0)));
            let _ = writeln!(
                text,
                "Zone: {} (id {}, cell {col}, {row})",
                name.unwrap_or("unavailable"),
                zone.map_or_else(|| "unknown".into(), |id| id.to_string())
            );
        } else {
            text.push_str("Zone: unavailable (no outdoor grid)\n");
        }
    } else {
        text.push_str("Zone: unavailable\n");
    }
    let _ = writeln!(text, "Mode: {} ({})", play.mode, mode_name(play.mode));
    // Host floats here only describe distances; they never feed gameplay or EE state.
    let distance = |e: &net::entity::Entity| {
        position.map(|p| {
            p.iter()
                .zip(e.pos)
                .map(|(&a, b)| (a as f64 - b as f64).powi(2))
                .sum::<f64>()
                .sqrt()
        })
    };
    let entity_line = |e: &net::entity::Entity| {
        let kind = if e.id == play.pet_id || e.creature.as_ref().is_some_and(|c| c.p256 == 3) {
            "pet"
        } else if e
            .creature
            .as_ref()
            .is_some_and(|c| c.p256 == 2 || c.p255 != 0)
        {
            "player"
        } else {
            "NPC"
        };
        let name = e.creature.as_ref().map_or("unnamed", |c| c.name.as_str());
        let level = e.creature.as_ref().map_or(0, |c| c.level);
        format!(
            "{name}, level {level}, distance {}, id {:#010x}, {kind}",
            distance(e).map_or_else(|| "unavailable".into(), |d| format!("{d:.2}")),
            e.id
        )
    };
    let target = play
        .entities
        .slots
        .iter()
        .flatten()
        .find(|e| e.id == play.target);
    let _ = writeln!(
        text,
        "Target: {}",
        target.map_or_else(
            || if play.target == u32::MAX {
                "none".into()
            } else {
                format!("unavailable, id {:#010x}", play.target)
            },
            entity_line
        )
    );
    let mut near: Vec<_> = play
        .entities
        .slots
        .iter()
        .flatten()
        .filter(|e| Some(e.id) != me && e.world as i32 == world && e.creature.is_some())
        .collect();
    near.sort_by(|a, b| {
        distance(a)
            .unwrap_or(f64::INFINITY)
            .total_cmp(&distance(b).unwrap_or(f64::INFINITY))
            .then(a.id.cmp(&b.id))
    });
    text.push_str("Nearest entities (up to 8):\n");
    for entity in near.into_iter().take(8) {
        let _ = writeln!(text, "  {}", entity_line(entity));
    }
    let _ = writeln!(text, "Addon HUD claims: {}", visibility::report_claims());
    let _ = writeln!(
        text,
        "Enabled mods/addons: {}",
        report::enabled_mods_addons_string()
    );
    let _ = writeln!(
        text,
        "Map open: {}; search open: {}; map search open: {}",
        play.map.open, play.search.open, play.map.search.open
    );
    let _ = writeln!(
        text,
        "Autorun: {}; following: {}; follow id: {:#010x}",
        play.autorun, play.following, play.follow_id
    );
    text
}

fn mode_name(mode: i32) -> &'static str {
    match mode {
        0 => "play",
        1 => "menu / loot",
        2 => "chat line",
        13 => "start menu",
        30 => "quick chat",
        33 => "chat history",
        34 => "options",
        38 => "manual camera",
        40 => "class mastery",
        43 => "duel invite",
        44 => "duel window",
        _ => "other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Play {
        let mut p = crate::play::commands::play();
        p.player.name = "CharacterKept".into();
        p.mode = 33;
        p.self_at = Some(([0.0; 3], None));
        p.target = 2;
        p.pet_id = 3;
        for id in 1..=11 {
            p.entities.slots[id as usize] = Some(net::entity::Entity {
                id,
                world: p.world_id() as i8,
                pos: [id as f32, 0.0, 0.0],
                creature: Some(net::entity::Creature {
                    name: format!("Neighbor{id}"),
                    level: 20 + id as u8,
                    p256: if id == 1 {
                        2
                    } else if id == 4 {
                        3
                    } else {
                        0
                    },
                    ..Default::default()
                }),
                ..Default::default()
            });
        }
        p
    }

    #[test]
    fn report_state_describes_play_mode_target_and_nearest_entities() {
        let p = fixture();
        let text = describe(&p, None);
        for expected in [
            "EQOA Enhanced",
            "Mode: 33 (chat history)",
            "Target: Neighbor2, level 22, distance 2.00, id 0x00000002, NPC",
            "Neighbor1, level 21, distance 1.00, id 0x00000001, player",
            "Neighbor3, level 23, distance 3.00, id 0x00000003, pet",
            "CharacterKept",
            "World:",
            "Heading (radians):",
            "Map open: false",
            "Autorun: false",
        ] {
            assert!(text.contains(expected), "missing {expected}: {text}");
        }
        assert_eq!(
            text.lines()
                .filter(|line| line.starts_with("  Neighbor"))
                .count(),
            8
        );
        assert!(!text.contains("Neighbor9"));
        assert!(text.find("  Neighbor1").unwrap() < text.find("  Neighbor8").unwrap());
        assert!(
            text.contains("id 0x00000004, pet"),
            "other players' pets are pets too"
        );
    }

    #[test]
    fn report_state_includes_claimed_hud_elements_and_movement_flags() {
        use crate::fork_ext::addons::health::InstanceToken;
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                visibility::publish(false);
            }
        }
        let _reset = Reset;
        let mut claims = visibility::HudClaimsRegistry::new();
        let token = InstanceToken::next().unwrap();
        claims.disable("hotbar_draw_compact", token, "Wereoxxs");
        claims.disable("compass_back", token, "Wereoxxs");
        visibility::publish_claims(&claims);
        let mut p = fixture();
        p.autorun = true;
        p.following = true;
        p.follow_id = 1;
        p.map.open = true;
        p.search.open = true;
        let text = describe(&p, None);
        for item in [
            "Addon HUD claims: compass_back, hotbar_draw_compact",
            "Map open: true; search open: true",
            "Autorun: true; following: true; follow id: 0x00000001",
        ] {
            assert!(text.contains(item), "missing {item}");
        }
        claims.release_instance(token);
        visibility::publish_claims(&claims);
        assert!(describe(&p, None).contains("Addon HUD claims: none"));
    }

    #[test]
    fn report_state_frame_hook_populates_saved_zip_with_redaction() {
        let dir = std::env::temp_dir().join(format!("eqoa-state-hook-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(dir.clone());
        let mut p = fixture();
        let home = std::env::var("USERPROFILE").unwrap_or_else(|_| std::env::var("HOME").unwrap());
        p.player.name =
            format!("CharacterKept account_canary_snapshot password_canary_snapshot {home}");
        crate::report::secret("account_canary_snapshot", "password_canary_snapshot");
        crate::report::with_state_test(&dir, || {
            super::super::frame_library(&p, &esf::Library::new(), None);
            report::save_report_to(
                &crate::report::Kind::Problem,
                "",
                "",
                None,
                Some(&dir),
                Some(&dir),
                false,
            )
            .unwrap();
        });
        let zip = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .find(|e| e.path().extension().is_some_and(|e| e == "zip"))
            .unwrap()
            .path();
        let entries = report::tests::extract_zip_entries(&std::fs::read(zip).unwrap());
        let state = entries
            .iter()
            .find(|(name, _)| name == "client_state.txt")
            .map(|(_, data)| String::from_utf8_lossy(data).into_owned())
            .unwrap_or_default();
        assert!(
            state.contains("Mode: 33 (chat history)"),
            "client_state.txt is empty without the per-frame report hook"
        );
        assert!(state.contains("CharacterKept") && state.contains("Neighbor1"));
        for secret in [
            "account_canary_snapshot",
            "password_canary_snapshot",
            home.as_str(),
        ] {
            assert!(!state.contains(secret), "snapshot leaked {secret}");
            assert!(
                !std::fs::read_to_string(dir.join("state.txt"))
                    .unwrap()
                    .contains(secret)
            );
        }
    }
}
