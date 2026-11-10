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
    migrate_configs();
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
pub(super) fn write(_c: &ClientOptions, _text: &mut String) {}

pub(super) fn parse(_c: &mut ClientOptions, key: &str, value: &str) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let on = match value {
            "on" => true,
            "off" => false,
            _ => return,
        };
        match key {
            "target_of_target" => {
                crate::fork_ext::addons::options::set_toggle("eqoa_target_of_target", "target_of_target", on);
            }
            "mana_bars" => {
                crate::fork_ext::addons::options::set_toggle("eqoa_mana_bars", "mana_bars", on);
            }
            "stats_box" => {
                crate::fork_ext::addons::options::set_toggle("eqoa_stats_box", "stats_box", on);
            }
            "target_hp_percent" => {
                crate::fork_ext::addons::options::set_toggle("eqoa_hud_numbers", "target_hp_percent", on);
            }
            "xp_details" => {
                crate::fork_ext::addons::options::set_toggle("eqoa_hud_numbers", "xp_details", on);
            }
            "hud_numbers" => {
                crate::fork_ext::addons::options::set_toggle("eqoa_hud_numbers", "hud_numbers", on);
            }
            _ => {}
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (key, value);
    }
}

pub fn migrate_configs() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        // 1. client.opt: migrate legacy keys to mod options toggles and strip them
        if let Some(bytes) = crate::settings::read_config("client.opt") {
            let text = String::from_utf8_lossy(&bytes);
            let mut modified = false;
            let mut cleaned_lines = Vec::new();
            for line in text.lines() {
                if let Some((k, v)) = line.split_once('=') {
                    let k = k.trim();
                    let v = v.trim();
                    let on = match v {
                        "on" => Some(true),
                        "off" => Some(false),
                        _ => None,
                    };
                    match (k, on) {
                        ("target_of_target", Some(b)) => {
                            crate::fork_ext::addons::options::set_toggle("eqoa_target_of_target", "target_of_target", b);
                            modified = true;
                        }
                        ("mana_bars", Some(b)) => {
                            crate::fork_ext::addons::options::set_toggle("eqoa_mana_bars", "mana_bars", b);
                            modified = true;
                        }
                        ("stats_box", Some(b)) => {
                            crate::fork_ext::addons::options::set_toggle("eqoa_stats_box", "stats_box", b);
                            modified = true;
                        }
                        ("target_hp_percent", Some(b)) => {
                            crate::fork_ext::addons::options::set_toggle("eqoa_hud_numbers", "target_hp_percent", b);
                            modified = true;
                        }
                        ("xp_details", Some(b)) => {
                            crate::fork_ext::addons::options::set_toggle("eqoa_hud_numbers", "xp_details", b);
                            modified = true;
                        }
                        ("hud_numbers", Some(b)) => {
                            crate::fork_ext::addons::options::set_toggle("eqoa_hud_numbers", "hud_numbers", b);
                            modified = true;
                        }
                        _ => {
                            cleaned_lines.push(line);
                        }
                    }
                } else {
                    cleaned_lines.push(line);
                }
            }
            if modified {
                let mut out = cleaned_lines.join("\n");
                if !out.is_empty() && !out.ends_with('\n') {
                    out.push('\n');
                }
                let _ = crate::settings::write_config("client.opt", out.as_bytes());
            }
        }

        // 2. keys.opt: migrate key.stats_box to mod.eqoa_stats_box.toggle and strip it
        if let Some(bytes) = crate::settings::read_config("keys.opt") {
            let text = String::from_utf8_lossy(&bytes);
            let mut modified = false;
            let mut cleaned_lines = Vec::new();
            let mut stats_box_key = None;
            for line in text.lines() {
                if let Some(val) = line.strip_prefix("key.stats_box=") {
                    stats_box_key = Some(val.trim().to_ascii_lowercase());
                    modified = true;
                } else {
                    cleaned_lines.push(line);
                }
            }
            if let Some(k) = stats_box_key {
                crate::fork_ext::bindpage::register("eqoa_stats_box", "toggle", "b", "Toggle Stats Box");
                crate::fork_ext::bindpage::set_key("eqoa_stats_box", "toggle", &k);
            } else if modified {
                let mut out = cleaned_lines.join("\n");
                if !out.is_empty() && !out.ends_with('\n') {
                    out.push('\n');
                }
                let _ = crate::settings::write_config("keys.opt", out.as_bytes());
            }
        }
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
    fn migration_moves_legacy_options_and_binds_into_mods_and_strips_files() {
        super::super::testing::reset(false);
        // Stage legacy files: client.opt and keys.opt
        let legacy_client = "lod=on\nbrightness=5\ncamera_distance=11\ntarget_of_target=on\nmana_bars=off\nstats_box=on\ntarget_hp_percent=off\nxp_details=off\nhud_numbers=off\n";
        let legacy_keys = "key.stats_box=j\nkey.attack=q\n";
        super::super::testing::put("client.opt", legacy_client.as_bytes());
        super::super::testing::put("keys.opt", legacy_keys.as_bytes());

        // Run migration
        migrate_configs();

        // 1. Mod option toggles received the migrated values
        assert_eq!(crate::fork_ext::addons::options::get_toggle("eqoa_target_of_target", "target_of_target"), Some(true));
        assert_eq!(crate::fork_ext::addons::options::get_toggle("eqoa_mana_bars", "mana_bars"), Some(false));
        assert_eq!(crate::fork_ext::addons::options::get_toggle("eqoa_stats_box", "stats_box"), Some(true));
        assert_eq!(crate::fork_ext::addons::options::get_toggle("eqoa_hud_numbers", "target_hp_percent"), Some(false));
        assert_eq!(crate::fork_ext::addons::options::get_toggle("eqoa_hud_numbers", "xp_details"), Some(false));
        assert_eq!(crate::fork_ext::addons::options::get_toggle("eqoa_hud_numbers", "hud_numbers"), Some(false));

        // 2. Mod keybind received the migrated key
        assert_eq!(crate::fork_ext::bindpage::get_key("eqoa_stats_box", "toggle"), Some("j".to_string()));

        // 3. client.opt is stripped of legacy keys
        let kept_client = String::from_utf8(super::super::testing::kept("client.opt").unwrap()).unwrap();
        for gone in ["target_of_target", "mana_bars", "stats_box", "target_hp_percent", "xp_details", "hud_numbers"] {
            assert!(!kept_client.contains(gone), "legacy key {gone} must be stripped from client.opt: {kept_client}");
        }
        assert!(kept_client.contains("lod=on\n"));
        assert!(kept_client.contains("brightness=5\n"));

        // 4. keys.opt is stripped of key.stats_box and has mod keybind
        let kept_keys = String::from_utf8(super::super::testing::kept("keys.opt").unwrap()).unwrap();
        assert!(!kept_keys.contains("key.stats_box="), "key.stats_box must be stripped from keys.opt: {kept_keys}");
        assert!(kept_keys.contains("mod.eqoa_stats_box.toggle=j"), "mod bind must be present in keys.opt: {kept_keys}");
        assert!(kept_keys.contains("key.attack=q"), "unrelated keys must be preserved: {kept_keys}");

        // 5. Explicit user choice subsequently made must be preserved across a second run
        crate::fork_ext::addons::options::set_toggle("eqoa_target_of_target", "target_of_target", false);
        migrate_configs();
        assert_eq!(crate::fork_ext::addons::options::get_toggle("eqoa_target_of_target", "target_of_target"), Some(false), "second run does nothing and keeps explicit choices");

        // 6. Negative control: fresh/empty files trigger no spurious options
        super::super::testing::put("client.opt", b"lod=on\n");
        super::super::testing::put("keys.opt", b"key.attack=q\n");
        migrate_configs();
        let fresh_client = String::from_utf8(super::super::testing::kept("client.opt").unwrap()).unwrap();
        assert_eq!(fresh_client, "lod=on\n");
        let fresh_keys = String::from_utf8(super::super::testing::kept("keys.opt").unwrap()).unwrap();
        assert_eq!(fresh_keys, "key.attack=q\n");

        super::super::testing::reset(false);
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
}
