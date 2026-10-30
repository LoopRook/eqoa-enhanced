//! The combat indication adapter; service code only receives plain combat data.
use super::Play;

#[cfg(not(target_arch = "wasm32"))]
impl Play {
    pub(crate) fn forge_snapshot(
        &self,
        walker: Option<&crate::walker::Walker>,
    ) -> crate::fork_ext::addons::host::Snapshot {
        let connected = self.session.self_id.is_some() && self.disconnected.is_none() && !self.quit;
        let hud_ready = self.hud_ready() && !self.loading_up();
        let in_game = connected && hud_ready;

        let player = if in_game {
            let mut name_buf = [0u8; 24];
            let name_bytes = self.player.name.as_bytes();
            let copy_len = name_bytes.len().min(24);
            name_buf[..copy_len].copy_from_slice(&name_bytes[..copy_len]);

            let (position, velocity, heading_rad) = if let Some(w) = walker {
                let pos = w.cam.walk.phys.position;
                let vel = w.cam.walk.phys.velocity;
                let h_rad = w.cam.walk.phys.angles.x.0;
                ([pos.x.0, pos.y.0, pos.z.0], [vel.x.0, vel.y.0, vel.z.0], h_rad)
            } else if let Some((at, _)) = self.self_at {
                (at, [0.0, 0.0, 0.0], 0.0)
            } else {
                ([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], 0.0)
            };

            let target_entity_id = if self.target == u32::MAX { 0 } else { self.target };
            let pet_entity_id = if self.pet_id == u32::MAX { None } else { Some(self.pet_id) };

            let cur_pwr = self.stats.stat.get(9).copied().unwrap_or(0).max(0) as u32;
            let max_pwr = self.stats.stat.get(10).copied().unwrap_or(0).max(0) as u32;
            let base_pwr = self.stats.base.get(10).copied().unwrap_or(0).max(0) as u32;

            let class_id = self.dump.player.class.clamp(0, 14) as u32;
            let race_id = self.dump.player.race.clamp(0, 9) as u32;

            let location = if let Some(w) = walker {
                let g = w.scene.grid.as_ref();
                let p = w.cam.walk.committed.position;
                let world = self.world_id();
                let cols = g.map_or(1, |g| g.cols);
                let rows = g.map_or(1, |g| g.rows);
                let cell = g.map_or(1.0, |g| g.cell_size.0);
                let origin = g.map_or([0.0; 3], |g| [g.origin.x.0, g.origin.y.0, g.origin.z.0]);
                let bounds = self.start.as_ref().and_then(|s| s.bounds.get(world as usize).copied().flatten());

                if cols == 1 && rows == 1 {
                    let mut loc = crate::fork_ext::addons::player::PlayerLocationSnapshot::default();
                    loc.place[..3].copy_from_slice(b"***");
                    loc.place_len = 3;
                    loc.name[..3].copy_from_slice(b"***");
                    loc.name_len = 3;
                    loc.valid = true;
                    loc
                } else if cell > 0.0 {
                    let col = if p.x.0 < 0.0 { (p.x.0 / cell).ceil() as i32 } else { (p.x.0 / cell).floor() as i32 };
                    let row = if p.z.0 < 0.0 { (p.z.0 / cell).ceil() as i32 } else { (p.z.0 / cell).floor() as i32 };
                    let in_cell_x = p.x.0 - col as f32 * cell - origin[0];
                    let in_cell_z = p.z.0 - row as f32 * cell - origin[2];
                    let r_idx = row - 1;
                    let c_idx = col - 1;

                    let zone_id = if let Some(b) = bounds {
                        if c_idx < b[0] || b[1] < c_idx || r_idx < b[2] || b[3] < r_idx {
                            641
                        } else {
                            crate::zones::zone(world, r_idx, c_idx).unwrap_or(641)
                        }
                    } else {
                        641
                    };

                    let place_str = self.words.as_ref().map(|w| w.get(zone_id as i32, 0)).unwrap_or("");
                    const DIRS: [[i32; 3]; 3] = [[1426, 1422, 1427], [1425, 1430, 1424], [1428, 1423, 1429]];
                    let third = |v: f32| if v < -333.34 { 0 } else if v <= 333.34 { 1 } else { 2 };
                    let dir_id = DIRS[third(in_cell_z)][third(in_cell_x)];
                    let compass_str = self.words.as_ref().map(|w| w.get(dir_id, 0)).unwrap_or("");
                    let name_str = if compass_str.is_empty() {
                        place_str.to_string()
                    } else {
                        format!("{place_str} ({compass_str})")
                    };

                    let mut loc = crate::fork_ext::addons::player::PlayerLocationSnapshot::default();
                    let p_bytes = place_str.as_bytes();
                    let p_len = p_bytes.len().min(64);
                    loc.place[..p_len].copy_from_slice(&p_bytes[..p_len]);
                    loc.place_len = p_len as u8;

                    let c_bytes = compass_str.as_bytes();
                    let c_len = c_bytes.len().min(16);
                    loc.compass[..c_len].copy_from_slice(&c_bytes[..c_len]);
                    loc.compass_len = c_len as u8;

                    let n_bytes = name_str.as_bytes();
                    let n_len = n_bytes.len().min(80);
                    loc.name[..n_len].copy_from_slice(&n_bytes[..n_len]);
                    loc.name_len = n_len as u8;

                    loc.cell_x = col;
                    loc.cell_z = row;
                    loc.valid = true;
                    loc
                } else {
                    crate::fork_ext::addons::player::PlayerLocationSnapshot::default()
                }
            } else {
                crate::fork_ext::addons::player::PlayerLocationSnapshot::default()
            };

            crate::fork_ext::addons::player::PlayerSnapshot {
                name: name_buf,
                name_len: copy_len as u8,
                level: self.dump.player.level.max(0) as u32,
                exp: self.dump.player.exp.max(0) as u32,
                exp_debt: self.dump.player.p2c.max(0) as u32,
                current_hp: self.forge_health.snapshot(connected, hud_ready).current_hp.max(0) as u32,
                max_hp: self.forge_health.snapshot(connected, hud_ready).max_hp.max(0) as u32,
                base_hp: self.stats.base.get(8).copied().unwrap_or(0).max(0) as u32,
                current_pwr: cur_pwr,
                max_pwr: max_pwr,
                base_pwr: base_pwr,
                total_str: self.stats.stat.get(0).copied().unwrap_or(0).max(0) as u32,
                total_sta: self.stats.stat.get(1).copied().unwrap_or(0).max(0) as u32,
                total_agi: self.stats.stat.get(2).copied().unwrap_or(0).max(0) as u32,
                total_dex: self.stats.stat.get(3).copied().unwrap_or(0).max(0) as u32,
                total_wis: self.stats.stat.get(4).copied().unwrap_or(0).max(0) as u32,
                total_int: self.stats.stat.get(5).copied().unwrap_or(0).max(0) as u32,
                total_cha: self.stats.stat.get(6).copied().unwrap_or(0).max(0) as u32,
                base_str: self.stats.base.get(0).copied().unwrap_or(0).max(0) as u32,
                base_sta: self.stats.base.get(1).copied().unwrap_or(0).max(0) as u32,
                base_agi: self.stats.base.get(2).copied().unwrap_or(0).max(0) as u32,
                base_dex: self.stats.base.get(3).copied().unwrap_or(0).max(0) as u32,
                base_wis: self.stats.base.get(4).copied().unwrap_or(0).max(0) as u32,
                base_int: self.stats.base.get(5).copied().unwrap_or(0).max(0) as u32,
                base_cha: self.stats.base.get(6).copied().unwrap_or(0).max(0) as u32,
                ac: self.stats.stat.get(14).copied().unwrap_or(0).max(0) as u32,
                poison_resist: self.stats.stat.get(22).copied().unwrap_or(0).max(0) as u32,
                disease_resist: self.stats.stat.get(23).copied().unwrap_or(0).max(0) as u32,
                fire_resist: self.stats.stat.get(24).copied().unwrap_or(0).max(0) as u32,
                cold_resist: self.stats.stat.get(25).copied().unwrap_or(0).max(0) as u32,
                lightning_resist: self.stats.stat.get(26).copied().unwrap_or(0).max(0) as u32,
                arcane_resist: self.stats.stat.get(27).copied().unwrap_or(0).max(0) as u32,
                cms: 0,
                cms_spent: 0,
                cm_pct: 0,
                position,
                velocity,
                heading_rad,
                entity_id: self.self_id32().unwrap_or(0),
                target_entity_id,
                pet_entity_id,
                is_auto_attacking: self.auto_attack,
                is_dead: self.death.is_some(),
                class_id,
                race_id,
                location,
                available: true,
            }
        } else {
            crate::fork_ext::addons::player::PlayerSnapshot::default()
        };

        crate::fork_ext::addons::host::Snapshot {
            in_game,
            start_menu: self.start.as_ref().is_some_and(|s| s.menu.visible),
            health: self.forge_health.snapshot(connected, hud_ready),
            player,
        }
    }
}
pub(super) fn health_hidden(play: &Play) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        play.forge_snapshot(None).health.is_available()
            && !play.loading_up()
            && crate::fork_ext::addons::visibility::published_health_hidden()
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = play;
        false
    }
}
pub(super) fn health_redraw(play: &mut Play, step: bool, loading_changed: bool) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let hidden = health_hidden(play);
        play.forge_health.hud_changed(hidden) || step || loading_changed
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = play;
        step || loading_changed
    }
}
use crate::fork_ext::telemetry::{CombatEvent, CombatKind};
struct CombatSnapshot {
    actor: String,
    target: String,
    amount: u32,
    kind: CombatKind,
}
impl CombatSnapshot {
    fn event(&self, ts: f64) -> CombatEvent<'_> {
        CombatEvent {
            ts,
            actor: &self.actor,
            target: &self.target,
            amount: self.amount,
            ability: "melee",
            kind: self.kind,
        }
    }
}
fn snapshot(play: &Play, attacker: u32, amount: i32, defender: u32) -> CombatSnapshot {
    let me = play.self_id32();
    let pet = (play.pet_id != u32::MAX).then_some(play.pet_id);
    let name = |id| {
        if Some(id) == me {
            "you".to_string()
        } else if Some(id) == pet {
            "pet".to_string()
        } else {
            play.entity_name(id)
                .unwrap_or_else(|| format!("{id:#010x}"))
        }
    };
    let kind = if amount < 0 {
        CombatKind::Heal
    } else if Some(attacker) == me || Some(attacker) == pet {
        CombatKind::Damage
    } else {
        CombatKind::DamageTaken
    };
    CombatSnapshot {
        actor: name(attacker),
        target: name(defender),
        amount: amount.unsigned_abs(),
        kind,
    }
}
impl Play {
    pub(super) fn fork_combat_event(&self, _attacker: u32, _amount: i32, _defender: u32) {
        // Unit tests never emit to a real companion. The browser has no native socket
        // or SystemTime clock; neither is touched by this hook there.
        #[cfg(all(not(target_arch = "wasm32"), not(test)))]
        {
            let event = snapshot(self, _attacker, _amount, _defender);
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0.0, |d| d.as_secs_f64());
            crate::fork_ext::telemetry::combat_event(&event.event(ts));
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn play() -> Play {
        struct Nowhere;
        impl net::session::Datagrams for Nowhere {
            fn send_to(&mut self, _: &[u8], _: [u8; 4], _: u16) {}
            fn recv_from(&mut self) -> Option<(Vec<u8>, [u8; 4], u16)> {
                None
            }
        }
        let mut session = net::session::Session::connect_with(
            Box::new(Nowhere),
            ([127, 0, 0, 1], 5000),
            ([127, 0, 0, 1], 10070),
            "test",
            [0; 32],
        )
        .unwrap();
        session.self_id = Some(0x100);
        Play::new(session, net::session::Player::default(), Box::default())
    }
    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn forge_health_play_snapshot_and_actual_hud_cache_predicate_follow_publication() {
        use crate::fork_ext::addons::visibility::{publish, published_health_hidden};
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                publish(false);
            }
        }
        let _reset = Reset;
        publish(false);
        let mut p = play();
        let initial_loading = p.loading.wait.take(); // explicit entered-world fixture
        p.hud = Some((
            crate::ui::Vui::new(Default::default(), Default::default(), vec![]),
            Default::default(),
            crate::hud::Hud::new(),
        ));
        let mut bytes = vec![0; 233];
        bytes[28..32].copy_from_slice(&0_i32.to_le_bytes());
        bytes[32..36].copy_from_slice(&100_i32.to_le_bytes());
        assert!(!p.forge_snapshot(None).managed().in_game);
        p.forge_health.receive(&bytes);
        assert!(
            p.forge_snapshot(None).managed().in_game,
            "zero HP remains an in-game death"
        );
        assert!(!health_redraw(&mut p, false, false));
        publish(true);
        assert!(published_health_hidden());
        assert!(health_hidden(&p));
        assert!(
            health_redraw(&mut p, false, false),
            "claim transition must invalidate cached quads without game step"
        );
        assert!(
            !health_redraw(&mut p, false, false),
            "unchanged claim can replay cache"
        );
        p.session.self_id = None;
        assert!(!p.forge_snapshot(None).managed().in_game);
        assert!(
            health_redraw(&mut p, false, false),
            "connect gap restores health despite previously published claim"
        );
        p.session.self_id = Some(0x100);
        p.forge_health.reset();
        assert!(
            !p.forge_snapshot(None).managed().in_game,
            "same id with reset has no fresh receipt"
        );
        p.forge_health.receive(&bytes);
        assert!(health_redraw(&mut p, false, false));
        publish(false);
        assert!(health_redraw(&mut p, false, false));
        assert!(health_redraw(&mut p, true, false));
        assert!(health_redraw(&mut p, false, true));
        p.disconnected = Some(0);
        assert!(!p.forge_snapshot(None).managed().in_game);
        p.disconnected = None;
        p.quit = true;
        assert!(!p.forge_snapshot(None).managed().in_game);
        p.quit = false;
        p.loading.wait = initial_loading;
        assert!(
            !p.forge_snapshot(None).managed().in_game,
            "loading must not display health overlay"
        );
    }

    #[test]
    fn self_pet_and_other_attribution_keep_the_existing_parser_contract() {
        let mut p = play();
        p.pet_id = 0x200;
        for (attacker, amount, defender, actor, target, kind) in [
            (0x100, 7, 0x300, "you", "0x00000300", CombatKind::Damage),
            (0x200, 8, 0x100, "pet", "you", CombatKind::Damage),
            (
                0x300,
                9,
                0x200,
                "0x00000300",
                "pet",
                CombatKind::DamageTaken,
            ),
            (0x300, -9, 0x100, "0x00000300", "you", CombatKind::Heal),
        ] {
            let event = snapshot(&p, attacker, amount, defender);
            assert_eq!(
                (&*event.actor, &*event.target, event.kind),
                (actor, target, kind)
            );
            assert_eq!(event.amount, amount.unsigned_abs());
        }
        p.pet_id = u32::MAX;
        assert_eq!(
            snapshot(&p, u32::MAX, 7, 0x100).kind,
            CombatKind::DamageTaken
        );
    }
    #[test]
    fn combat_snapshot_serializes_entity_names_and_minimum_signed_amount() {
        let mut p = play();
        p.entities.slots = vec![Some(net::entity::Entity {
            id: 0x300,
            creature: Some(net::entity::Creature {
                name: "A \"quoted\" name\\tail".to_string(),
                ..Default::default()
            }),
            ..Default::default()
        })];
        let event = snapshot(&p, 0x300, i32::MIN, 0x100);
        let wire = crate::fork_ext::telemetry::json(&event.event(1.25));
        let value: serde_json::Value = serde_json::from_str(&wire).unwrap();
        assert_eq!(value["actor"], "A \"quoted\" name\\tail");
        assert_eq!(value["amount"], 2147483648u64);
        assert_eq!(value["kind"], "heal");
        assert_eq!(value["ability"], "melee");
    }
}
