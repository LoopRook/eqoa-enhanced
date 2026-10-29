//! Receipt validity belongs to a Play lifetime, not the HUD atlas lifetime.
use super::health::HealthSnapshot;

#[derive(Default)]
pub(crate) struct GameHealth {
    received: Option<(i32, i32)>,
    last_hud_hidden: bool,
}
impl GameHealth {
    pub fn receive(&mut self, bytes: &[u8]) {
        let mut reader = net::stream::Reader::new(bytes);
        let stats = net::dump::read_stats(&mut reader);
        self.received = (!reader.error).then_some((stats.stat[7], stats.stat[8]));
    }
    pub fn reset(&mut self) {
        self.received = None;
    }
    pub fn snapshot(&self, connected: bool, hud_ready: bool) -> HealthSnapshot {
        match self.received {
            Some((hp, max)) if connected && hud_ready => HealthSnapshot::new(true, hp, max),
            _ => HealthSnapshot::unavailable(),
        }
    }
    pub fn hud_changed(&mut self, hidden: bool) -> bool {
        let changed = self.last_hud_hidden != hidden;
        self.last_hud_hidden = hidden;
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn packet(hp: i32, max: i32) -> Vec<u8> {
        let mut bytes = vec![0; 233];
        bytes[28..32].copy_from_slice(&hp.to_le_bytes());
        bytes[32..36].copy_from_slice(&max.to_le_bytes());
        bytes
    }
    #[test]
    fn forge_game_health_ordered_receipts_reentry_short_packet_and_connect_gap() {
        let mut state = GameHealth::default();
        assert!(!state.snapshot(true, true).is_available());
        state.receive(&packet(70, 100));
        assert_eq!(state.snapshot(true, true).current_hp(), 70);
        assert!(
            !state.snapshot(false, true).is_available(),
            "connect-none gap"
        );
        assert!(
            !state.snapshot(true, false).is_available(),
            "atlas not ready"
        );
        state.reset(); // WorldEntry even with SAME id/world, after an early receipt
        assert!(!state.snapshot(true, true).is_available());
        state.receive(&packet(60, 100)); // ordered WorldEntry -> full stats
        assert_eq!(state.snapshot(true, true).current_hp(), 60);
        state.receive(&packet(50, 100)[..232]);
        assert!(
            !state.snapshot(true, true).is_available(),
            "short update invalidates old HP"
        );
        state.receive(&packet(40, 100));
        state.reset(); // disconnected; new Play also defaults unavailable
        assert!(!state.snapshot(true, true).is_available());
        assert!(!GameHealth::default().snapshot(true, true).is_available());
    }
    #[test]
    fn forge_game_health_zero_hp_is_valid_death_and_cache_tracks_both_transitions() {
        let mut state = GameHealth::default();
        state.receive(&packet(0, 100));
        assert!(state.snapshot(true, true).is_available());
        assert_eq!(state.snapshot(true, true).current_hp(), 0);
        state.receive(&packet(0, 0));
        assert!(!state.snapshot(true, true).is_available());
        assert!(!state.hud_changed(false));
        assert!(state.hud_changed(true));
        assert!(!state.hud_changed(true));
        state.reset();
        assert!(state.hud_changed(false));
    }
}
