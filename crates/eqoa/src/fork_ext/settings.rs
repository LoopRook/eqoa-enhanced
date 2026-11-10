// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
// Upstream portions up to 1e23e80 remain MIT (NOTICE).
//! Enhanced default migration and HUD-number keys in their existing format.
use super::ClientOptions;
const FILE: &str = "enhanced.opt";
const DEFAULTS_APPLIED: &str = "first_run_defaults_applied=on";

/// Migrate the old Enhanced defaults once. Missing values used to mean
/// 64/650; explicit 24/100 was saved and must remain explicit until the
/// marker is written. Later saves use upstream's serializer and defaults.
pub(super) fn migrate_defaults() -> Option<ClientOptions> {
    let marker = super::store::read(FILE).unwrap_or_default();
    let mut marker = String::from_utf8_lossy(&marker).into_owned();
    if marker.lines().any(|line| line == DEFAULTS_APPLIED) {
        return None;
    }
    let mut main = None;
    let mut persisted = true;
    for place in 0..crate::multibox::PLACES {
        let file = super::own_file("client", place);
        let bytes = super::store::read(&file);
        if place > 0 && bytes.is_none() {
            continue;
        }
        let original = bytes.as_deref().unwrap_or_default();
        let mut text = String::from_utf8_lossy(original).into_owned();
        let has = |key: &str, low: u16, high: u16| {
            text.lines()
                .filter_map(|line| line.split_once('='))
                .any(|(k, value)| {
                    k == key
                        && value
                            .parse::<u16>()
                            .ok()
                            .is_some_and(|v| (low..=high).contains(&v))
                })
        };
        let (slots, view) = (
            has(
                "entity_slots",
                super::ENTITY_SLOTS_GAME as u16,
                super::ENTITY_SLOTS_MAX as u16,
            ),
            has(
                "entity_view",
                super::ENTITY_VIEW_GAME,
                super::ENTITY_VIEW_MAX,
            ),
        );
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        if !slots {
            text.push_str("entity_slots=64\n");
        }
        if !view {
            text.push_str("entity_view=650\n");
        }
        if place == 0 {
            main = Some(ClientOptions::from_text(&text));
        }
        if text.as_bytes() != original {
            if let Err(error) = super::store::write(&file, text.as_bytes()) {
                eprintln!("settings: enhanced defaults migration: {error}");
                persisted = false;
            }
        }
    }
    // A failed write retries next time; never mark an incomplete migration.
    if persisted {
        if !marker.is_empty() && !marker.ends_with('\n') {
            marker.push('\n');
        }
        marker.push_str(DEFAULTS_APPLIED);
        marker.push('\n');
        if let Err(error) = super::store::write(FILE, marker.as_bytes()) {
            static WARNING: std::sync::Once = std::sync::Once::new();
            WARNING.call_once(|| eprintln!("settings: enhanced defaults marker: {error}"));
        }
    }
    main
}

/// Upstream fixtures represent an already-migrated installation. Migration
/// tests remove this marker to exercise the real first-load path.
#[cfg(test)]
pub(super) fn reset_test_client() {
    super::testing::put(FILE, DEFAULTS_APPLIED.as_bytes());
    // An already-loaded client remains at upstream defaults when this
    // fixture subsequently refuses every read, including the marker.
    if super::store::REFUSE.with(|refuse| refuse.get()) {
        super::set_client(ClientOptions::default());
    }
}
#[cfg(test)]
thread_local! { static UPSTREAM_ROWS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
#[cfg(test)]
pub(super) struct UpstreamRows;
#[cfg(test)]
pub(super) fn upstream_rows() -> UpstreamRows {
    UPSTREAM_ROWS.with(|upstream| upstream.set(true));
    UpstreamRows
}
#[cfg(test)]
impl Drop for UpstreamRows {
    fn drop(&mut self) {
        UPSTREAM_ROWS.with(|upstream| upstream.set(false));
    }
}
fn enhanced_rows() -> bool {
    #[cfg(test)]
    if UPSTREAM_ROWS.with(|upstream| upstream.get()) {
        return false;
    }
    true
}
pub(super) fn write(c: &ClientOptions, text: &mut String) {
    if enhanced_rows() {
        if c.target_of_target {
            text.push_str("target_of_target=on\n");
        }
        if !c.mana_bars {
            text.push_str("mana_bars=off\n");
        }
        if c.stats_box {
            text.push_str("stats_box=on\n");
        }
    }
    if !c.target_hp_percent {
        text.push_str("target_hp_percent=off\n");
    }
    if !c.xp_details {
        text.push_str("xp_details=off\n");
    }
    if !c.hud_numbers {
        text.push_str("hud_numbers=off\n");
    }
}
pub(super) fn parse(c: &mut ClientOptions, key: &str, value: &str) {
    let on = match value {
        "on" => true,
        "off" => false,
        _ => return,
    };
    if enhanced_rows() {
        match key {
            "target_of_target" => c.target_of_target = on,
            "mana_bars" => c.mana_bars = on,
            "stats_box" => c.stats_box = on,
            _ => {}
        }
    }
    match key {
        "target_hp_percent" => c.target_hp_percent = on,
        "xp_details" => c.xp_details = on,
        "hud_numbers" => c.hud_numbers = on,
        _ => {}
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quest_markers_kept_only_when_off() {
        assert!(ClientOptions::default().quest_markers, "on by default");
        assert_eq!(
            ClientOptions {
                quest_markers: false,
                ..Default::default()
            }
            .to_text(),
            "lod=on\nbrightness=5\ncamera_distance=11\nquest_markers=off\n"
        );
        assert!(!ClientOptions::from_text("quest_markers=off").quest_markers);
        assert!(ClientOptions::from_text("quest_markers=on") == ClientOptions::default());
        for bad in ["quest_markers=0", "quest_markers=no", "quest_markers="] {
            assert!(
                ClientOptions::from_text(bad).quest_markers,
                "{bad}: on, the default"
            );
        }
    }

    #[test]
    fn quest_tracker_kept_only_when_off() {
        assert!(ClientOptions::default().quest_tracker, "on by default");
        assert_eq!(
            ClientOptions {
                quest_tracker: false,
                ..Default::default()
            }
            .to_text(),
            "lod=on\nbrightness=5\ncamera_distance=11\nquest_tracker=off\n"
        );
        let off = ClientOptions::from_text("quest_tracker=off");
        assert!(
            !off.quest_tracker && off.quest_markers,
            "the tracker off, the marks kept"
        );
        for bad in ["quest_tracker=0", "quest_tracker="] {
            assert!(
                ClientOptions::from_text(bad).quest_tracker,
                "{bad}: on, the default"
            );
        }
    }

    #[test]
    fn minimap_kept_only_when_off() {
        assert!(ClientOptions::default().minimap, "on by default");
        assert_eq!(
            ClientOptions {
                minimap: false,
                ..Default::default()
            }
            .to_text(),
            "lod=on\nbrightness=5\ncamera_distance=11\nminimap=off\n"
        );
        assert!(!ClientOptions::from_text("minimap=off").minimap);
        assert!(ClientOptions::from_text("minimap=on") == ClientOptions::default());
        for bad in ["minimap=0", "minimap=no", "minimap="] {
            assert!(
                ClientOptions::from_text(bad).minimap,
                "{bad}: on, the default"
            );
        }
    }

    #[test]
    fn stats_box_kept_only_when_on() {
        assert!(!ClientOptions::default().stats_box, "off by default");
        assert_eq!(
            ClientOptions {
                stats_box: true,
                ..Default::default()
            }
            .to_text(),
            "lod=on\nbrightness=5\ncamera_distance=11\nstats_box=on\n"
        );
        let on = ClientOptions::from_text("stats_box=on");
        assert!(on.stats_box && on.minimap, "the box on, the minimap kept");
        assert!(ClientOptions::from_text("stats_box=off") == ClientOptions::default());
        for bad in ["stats_box=1", "stats_box=yes", "stats_box=", "minimap=on"] {
            assert!(
                !ClientOptions::from_text(bad).stats_box,
                "{bad}: off, the default"
            );
        }
    }

    #[test]
    fn mana_bars_kept_only_when_off() {
        assert!(ClientOptions::default().mana_bars, "on by default");
        assert_eq!(
            ClientOptions {
                mana_bars: false,
                ..Default::default()
            }
            .to_text(),
            "lod=on\nbrightness=5\ncamera_distance=11\nmana_bars=off\n"
        );
        assert!(!ClientOptions::from_text("mana_bars=off").mana_bars);
        assert!(ClientOptions::from_text("mana_bars=on") == ClientOptions::default());
        for bad in ["mana_bars=0", "mana_bars=no", "mana_bars="] {
            assert!(
                ClientOptions::from_text(bad).mana_bars,
                "{bad}: on, the default"
            );
        }
    }

    #[test]
    fn map_markers_kept_only_when_off() {
        assert!(ClientOptions::default().map_markers, "on");
        let off = ClientOptions {
            map_markers: false,
            ..Default::default()
        };
        assert_eq!(
            off.to_text(),
            "lod=on\nbrightness=5\ncamera_distance=11\nmap_markers=off\n"
        );
        assert!(
            !ClientOptions::from_text(&off.to_text()).map_markers,
            "the control: off read back off"
        );
        assert!(ClientOptions::from_text("map_markers=on").map_markers);
        for bad in ["map_markers=0", "map_markers="] {
            assert!(
                ClientOptions::from_text(bad).map_markers,
                "{bad}: on, the default"
            );
        }
        // the town names the same way, apart from the markers
        let towns = ClientOptions {
            map_towns: false,
            ..Default::default()
        };
        assert_eq!(
            towns.to_text(),
            "lod=on\nbrightness=5\ncamera_distance=11\nmap_towns=off\n"
        );
        let back = ClientOptions::from_text(&towns.to_text());
        assert!(
            !back.map_towns && back.map_markers,
            "off read back off, the markers kept on"
        );
    }

    fn unmigrated(saved: Option<&str>) {
        super::super::testing::reset(false);
        super::super::store::FILES.with(|files| {
            files.borrow_mut().remove(FILE);
        });
        if let Some(text) = saved {
            super::super::testing::put("client.opt", text.as_bytes());
        }
    }
    #[test]
    fn invalid_legacy_defaults_migrate_but_valid_explicit_choices_stay() {
        for invalid in [
            "entity_slots=16\nentity_view=50\n",
            "entity_slots=nope\nentity_view=65536\n",
        ] {
            unmigrated(Some(invalid));
            let c = super::super::client();
            assert_eq!((c.entity_slots, c.entity_view), (64, 650));
        }
        unmigrated(Some("entity_slots=24\nentity_view=100\n"));
        let c = super::super::client();
        assert_eq!(
            (c.entity_slots, c.entity_view),
            (24, 100),
            "valid retail values are not treated as invalid"
        );
        super::super::testing::reset(false);
    }
    #[test]
    fn target_of_target_keeps_its_saved_key_and_rejects_invalid_values() {
        assert_eq!(
            ClientOptions {
                target_of_target: true,
                ..Default::default()
            }
            .to_text(),
            "lod=on\nbrightness=5\ncamera_distance=11\ntarget_of_target=on\n"
        );
        assert!(ClientOptions::from_text("target_of_target=on").target_of_target);
        for value in ["off", "yes", "1"] {
            assert!(
                !ClientOptions::from_text(&format!("target_of_target={value}")).target_of_target
            );
        }
    }
    #[test]
    fn combined_old_save_keeps_fork_and_upstream_keys_and_drops_unknowns() {
        let old = "target_of_target=on\nmana_bars=off\nstats_box=on\nminimap=off\nquest_markers=off\nquest_tracker=off\nmap_markers=off\nmap_towns=off\nflora=off\nloading=instant\nunknown_setting=off\n";
        let c = ClientOptions::from_text(old);
        assert!(c.target_of_target && !c.mana_bars && c.stats_box);
        let text = c.to_text();
        for line in old
            .lines()
            .filter(|line| !line.starts_with("unknown_setting="))
        {
            assert!(
                text.lines().any(|kept| kept == line),
                "missing {line}: {text}"
            );
        }
        assert!(!text.contains("unknown_setting"));
        assert_eq!(ClientOptions::from_text(&text), c);
    }
    #[test]
    fn first_run_and_legacy_omitted_defaults_migrate_once() {
        for old in [
            None,
            Some("brightness=7\nunknown=kept\n"),
            Some("entity_slots=128\n"),
        ] {
            unmigrated(old);
            let fresh = super::super::client();
            assert_eq!(
                fresh.entity_slots,
                if old.is_some_and(|s| s.contains("entity_slots")) {
                    128
                } else {
                    64
                }
            );
            assert_eq!(fresh.entity_view, 650);
            assert!(super::super::testing::kept(FILE).is_some());
            let kept = super::super::testing::kept("client.opt").unwrap();
            if let Some(old) = old {
                assert!(kept.starts_with(old.as_bytes()));
            }
            assert_eq!(ClientOptions::from_text(&fresh.to_text()), fresh);
            // Marker present: a later upstream-default file stays at 24/100.
            super::super::testing::put("client.opt", b"");
            super::super::CLIENT.with(|c| c.set(None));
            let later = super::super::client();
            assert_eq!((later.entity_slots, later.entity_view), (24, 100));
        }
        super::super::testing::reset(false);
    }
    #[test]
    fn migration_preserves_explicit_retail_choices_and_secondary_windows() {
        unmigrated(Some("entity_slots=24\nentity_view=100\n"));
        super::super::testing::put("client-2.opt", b"brightness=9\n");
        super::super::testing::put("client-3.opt", b"entity_slots=24\nentity_view=100\n");
        super::super::testing::put(FILE, b"another_fork_key=kept\n");
        let kept = super::super::client();
        assert_eq!((kept.entity_slots, kept.entity_view), (24, 100));
        let read = |name| {
            ClientOptions::from_text(&String::from_utf8_lossy(
                &super::super::testing::kept(name).unwrap(),
            ))
        };
        let second = read("client-2.opt");
        assert_eq!(
            (second.entity_slots, second.entity_view, second.brightness),
            (64, 650, 9)
        );
        let third = read("client-3.opt");
        assert_eq!((third.entity_slots, third.entity_view), (24, 100));
        assert!(
            super::super::testing::kept(FILE)
                .unwrap()
                .starts_with(b"another_fork_key=kept\n")
        );
        super::super::testing::reset(false);
    }
    #[test]
    fn failed_migration_is_not_marked_complete() {
        unmigrated(None);
        super::super::store::REFUSE.with(|r| r.set(true));
        let c = super::super::client();
        assert_eq!((c.entity_slots, c.entity_view), (64, 650));
        assert!(super::super::testing::kept(FILE).is_none());
        super::super::store::REFUSE.with(|r| r.set(false));
        super::super::CLIENT.with(|c| c.set(None));
        assert_eq!(super::super::client(), c);
        assert!(super::super::testing::kept(FILE).is_some());
        super::super::testing::reset(false);
    }
    #[test]
    fn default_number_settings_write_nothing_and_off_keeps_legacy_keys() {
        let defaults = ClientOptions::default();
        let mut text = String::new();
        write(&defaults, &mut text);
        assert_eq!(text, "");
        let c = ClientOptions {
            target_hp_percent: false,
            xp_details: false,
            hud_numbers: false,
            ..defaults
        };
        write(&c, &mut text);
        assert_eq!(
            text,
            "target_hp_percent=off\nxp_details=off\nhud_numbers=off\n"
        );
        let mut restored = defaults;
        for line in text.lines() {
            let (key, value) = line.split_once('=').unwrap();
            parse(&mut restored, key, value);
        }
        assert_eq!(restored, c);
        parse(&mut restored, "hud_numbers", "on");
        assert!(restored.hud_numbers);
    }
    #[test]
    fn unknown_keys_and_invalid_number_values_leave_defaults_alone() {
        let defaults = ClientOptions::default();
        let mut c = defaults;
        for (key, value) in [
            ("minimap", "off"),
            ("hud_numbers", "false"),
            ("target_hp_percent", "OFF"),
            ("xp_details", "0"),
        ] {
            parse(&mut c, key, value);
        }
        assert_eq!(c, defaults);
    }
}
