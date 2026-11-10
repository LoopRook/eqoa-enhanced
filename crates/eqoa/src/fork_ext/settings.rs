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
        let has = |key: &str| {
            text.lines()
                .any(|line| line.split_once('=').is_some_and(|(k, _)| k == key))
        };
        let (slots, view) = (has("entity_slots"), has("entity_view"));
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
            eprintln!("settings: enhanced defaults marker: {error}");
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
        super::CLIENT.with(|client| client.set(Some(ClientOptions::default())));
    }
}
pub(super) fn write(c: &ClientOptions, text: &mut String) {
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
