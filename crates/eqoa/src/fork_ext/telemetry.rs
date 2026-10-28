//! Plain combat events and the native localhost broadcaster.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CombatKind {
    Heal,
    Damage,
    DamageTaken,
}
pub struct CombatEvent<'a> {
    pub ts: f64,
    pub actor: &'a str,
    pub target: &'a str,
    pub amount: u32,
    pub ability: &'a str,
    pub kind: CombatKind,
}
/// The existing parser wire contract, escaped by JSON rather than interpolation.
pub fn json(event: &CombatEvent<'_>) -> String {
    let kind = match event.kind {
        CombatKind::Heal => "heal",
        CombatKind::Damage => "damage",
        CombatKind::DamageTaken => "damage_taken",
    };
    serde_json::json!({"ts":event.ts, "actor":event.actor, "target":event.target,
        "amount":event.amount, "ability":event.ability, "kind":kind})
    .to_string()
}
pub fn combat_event(event: &CombatEvent<'_>) {
    broadcast(&json(event));
}
#[cfg(not(target_arch = "wasm32"))]
static SOCKET: std::sync::OnceLock<Option<std::net::UdpSocket>> = std::sync::OnceLock::new();

/// Native-only best-effort local delivery. The browser has no companion socket.
#[cfg(not(target_arch = "wasm32"))]
pub fn broadcast(json: &str) {
    let socket = SOCKET.get_or_init(|| {
        let socket = std::net::UdpSocket::bind("127.0.0.1:0").ok()?;
        socket.set_nonblocking(true).ok()?;
        Some(socket)
    });
    if let Some(socket) = socket {
        let _ = send_to(socket, json, "127.0.0.1:28015".parse().unwrap());
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn send_to(
    socket: &std::net::UdpSocket,
    json: &str,
    destination: std::net::SocketAddr,
) -> std::io::Result<usize> {
    socket.send_to(json.as_bytes(), destination)
}

#[cfg(target_arch = "wasm32")]
pub fn broadcast(_json: &str) {}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn serializes_names_and_preserves_parser_contract() {
        let event = CombatEvent {
            ts: 1234.125,
            actor: "A\"B\\C\n",
            target: "you",
            amount: 42,
            ability: "melee",
            kind: CombatKind::DamageTaken,
        };
        let v: serde_json::Value = serde_json::from_str(&json(&event)).unwrap();
        assert_eq!(
            v,
            serde_json::json!({"ts":1234.125,"actor":"A\"B\\C\n","target":"you","amount":42,"ability":"melee","kind":"damage_taken"})
        );
        let heal = CombatEvent {
            kind: CombatKind::Heal,
            ..event
        };
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&json(&heal)).unwrap()["kind"],
            "heal"
        );
        let damage = CombatEvent {
            kind: CombatKind::Damage,
            ..heal
        };
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&json(&damage)).unwrap()["kind"],
            "damage"
        );
    }
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_delivery_sends_one_json_datagram_only_to_requested_listener() {
        use std::net::UdpSocket;
        use std::time::Duration;
        let listener = UdpSocket::bind("127.0.0.1:0").unwrap();
        let other = UdpSocket::bind("127.0.0.1:0").unwrap();
        listener
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        other.set_nonblocking(true).unwrap();
        let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
        let event = CombatEvent {
            ts: 1.0,
            actor: "you",
            target: "rat",
            amount: 7,
            ability: "melee",
            kind: CombatKind::Damage,
        };
        send_to(&sender, &json(&event), listener.local_addr().unwrap()).unwrap();
        let mut bytes = [0; 1024];
        let (count, _) = listener.recv_from(&mut bytes).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes[..count]).unwrap();
        assert_eq!(value["amount"], 7);
        assert_eq!(value["kind"], "damage");
        assert!(other.recv_from(&mut bytes).is_err());
    }
}
