//! The combat indication adapter; service code only receives plain combat data.
use super::Play;
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
