//! The combat indication adapter; service code only receives plain combat data.
use super::Play;

#[cfg(not(target_arch = "wasm32"))]
impl Play {
    pub(crate) fn forge_snapshot(&self) -> crate::fork_ext::addons::host::Snapshot {
        let connected = self.session.self_id.is_some() && self.disconnected.is_none() && !self.quit;
        let hud_ready = self.hud_ready() && !self.loading_up();
        crate::fork_ext::addons::host::Snapshot {
            in_game: connected && hud_ready,
            start_menu: self.start.as_ref().is_some_and(|s| s.menu.visible),
            health: self.forge_health.snapshot(connected, hud_ready),
        }
    }
}
pub(super) fn health_hidden(play: &Play) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        play.forge_snapshot().health.is_available()
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
        assert!(!p.forge_snapshot().managed().in_game);
        p.forge_health.receive(&bytes);
        assert!(
            p.forge_snapshot().managed().in_game,
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
        assert!(!p.forge_snapshot().managed().in_game);
        assert!(
            health_redraw(&mut p, false, false),
            "connect gap restores health despite previously published claim"
        );
        p.session.self_id = Some(0x100);
        p.forge_health.reset();
        assert!(
            !p.forge_snapshot().managed().in_game,
            "same id with reset has no fresh receipt"
        );
        p.forge_health.receive(&bytes);
        assert!(health_redraw(&mut p, false, false));
        publish(false);
        assert!(health_redraw(&mut p, false, false));
        assert!(health_redraw(&mut p, true, false));
        assert!(health_redraw(&mut p, false, true));
        p.disconnected = Some(0);
        assert!(!p.forge_snapshot().managed().in_game);
        p.disconnected = None;
        p.quit = true;
        assert!(!p.forge_snapshot().managed().in_game);
        p.quit = false;
        p.loading.wait = initial_loading;
        assert!(
            !p.forge_snapshot().managed().in_game,
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
