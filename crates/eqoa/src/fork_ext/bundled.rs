//! Auto-extracted bundled mod packages and embedded assets.
//!
//! Provides compile-time embedded copies of approved bundled mods (wereoxxs_ui,
//! arpg_ui, avoids_modern_ui) and their shared resources for auto-updating players.
//! Ensures mods stay off until the player turns them on, never overwrites existing
//! files, and never reinstalls packages the player intentionally deleted.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

pub const PACKAGES: &[&str] = &["wereoxxs_ui", "arpg_ui", "avoids_modern_ui"];

#[derive(Clone, Copy, Debug)]
pub struct BundledFile {
    pub path: &'static str,
    pub package: &'static str,
    pub data: &'static [u8],
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct InstallResult {
    pub packages_extracted: Vec<String>,
    pub files_extracted: usize,
    pub skipped_existing: usize,
}

pub const EMBEDDED_FILES: &[BundledFile] = &[
    BundledFile {
        path: "arpg_ui/arpg_ui.lua",
        package: "arpg_ui",
        data: include_bytes!("../../../../bundled-mods/arpg_ui/arpg_ui.lua"),
    },
    BundledFile {
        path: "arpg_ui/README.md",
        package: "arpg_ui",
        data: include_bytes!("../../../../bundled-mods/arpg_ui/README.md"),
    },
    BundledFile {
        path: "arpg_ui/resources/52x52 SpellSlotBorder.png",
        package: "arpg_ui",
        data: include_bytes!("../../../../bundled-mods/arpg_ui/resources/52x52 SpellSlotBorder.png"),
    },
    BundledFile {
        path: "arpg_ui/resources/64x64 BagSlotBorder.png",
        package: "arpg_ui",
        data: include_bytes!("../../../../bundled-mods/arpg_ui/resources/64x64 BagSlotBorder.png"),
    },
    BundledFile {
        path: "arpg_ui/resources/BlankSlot.png",
        package: "arpg_ui",
        data: include_bytes!("../../../../bundled-mods/arpg_ui/resources/BlankSlot.png"),
    },
    BundledFile {
        path: "arpg_ui/resources/DarkOrbBorder.png",
        package: "arpg_ui",
        data: include_bytes!("../../../../bundled-mods/arpg_ui/resources/DarkOrbBorder.png"),
    },
    BundledFile {
        path: "arpg_ui/resources/Divider.png",
        package: "arpg_ui",
        data: include_bytes!("../../../../bundled-mods/arpg_ui/resources/Divider.png"),
    },
    BundledFile {
        path: "arpg_ui/resources/HealthPanel.png",
        package: "arpg_ui",
        data: include_bytes!("../../../../bundled-mods/arpg_ui/resources/HealthPanel.png"),
    },
    BundledFile {
        path: "arpg_ui/resources/itsmars_exp_back.png",
        package: "arpg_ui",
        data: include_bytes!("../../../../bundled-mods/arpg_ui/resources/itsmars_exp_back.png"),
    },
    BundledFile {
        path: "arpg_ui/resources/itsmars_exp_fill.png",
        package: "arpg_ui",
        data: include_bytes!("../../../../bundled-mods/arpg_ui/resources/itsmars_exp_fill.png"),
    },
    BundledFile {
        path: "arpg_ui/resources/itsmars_exp_highlight.png",
        package: "arpg_ui",
        data: include_bytes!("../../../../bundled-mods/arpg_ui/resources/itsmars_exp_highlight.png"),
    },
    BundledFile {
        path: "arpg_ui/resources/itsmars_exp_shadow.png",
        package: "arpg_ui",
        data: include_bytes!("../../../../bundled-mods/arpg_ui/resources/itsmars_exp_shadow.png"),
    },
    BundledFile {
        path: "arpg_ui/resources/itsmars_orb_back1.png",
        package: "arpg_ui",
        data: include_bytes!("../../../../bundled-mods/arpg_ui/resources/itsmars_orb_back1.png"),
    },
    BundledFile {
        path: "arpg_ui/resources/itsmars_orb_back2.png",
        package: "arpg_ui",
        data: include_bytes!("../../../../bundled-mods/arpg_ui/resources/itsmars_orb_back2.png"),
    },
    BundledFile {
        path: "arpg_ui/resources/itsmars_orb_border.png",
        package: "arpg_ui",
        data: include_bytes!("../../../../bundled-mods/arpg_ui/resources/itsmars_orb_border.png"),
    },
    BundledFile {
        path: "arpg_ui/resources/itsmars_orb_fill.png",
        package: "arpg_ui",
        data: include_bytes!("../../../../bundled-mods/arpg_ui/resources/itsmars_orb_fill.png"),
    },
    BundledFile {
        path: "arpg_ui/resources/itsmars_orb_highlight.png",
        package: "arpg_ui",
        data: include_bytes!("../../../../bundled-mods/arpg_ui/resources/itsmars_orb_highlight.png"),
    },
    BundledFile {
        path: "arpg_ui/resources/itsmars_orb_shadow.png",
        package: "arpg_ui",
        data: include_bytes!("../../../../bundled-mods/arpg_ui/resources/itsmars_orb_shadow.png"),
    },
    BundledFile {
        path: "arpg_ui/resources/itsmars_scroll_fill.png",
        package: "arpg_ui",
        data: include_bytes!("../../../../bundled-mods/arpg_ui/resources/itsmars_scroll_fill.png"),
    },
    BundledFile {
        path: "arpg_ui/resources/itsmars_scroll_lip.png",
        package: "arpg_ui",
        data: include_bytes!("../../../../bundled-mods/arpg_ui/resources/itsmars_scroll_lip.png"),
    },
    BundledFile {
        path: "arpg_ui/resources/NamePanel.png",
        package: "arpg_ui",
        data: include_bytes!("../../../../bundled-mods/arpg_ui/resources/NamePanel.png"),
    },
    BundledFile {
        path: "arpg_ui/resources/TooltipBackground.png",
        package: "arpg_ui",
        data: include_bytes!("../../../../bundled-mods/arpg_ui/resources/TooltipBackground.png"),
    },
    BundledFile {
        path: "avoids_modern_ui/avoids_modern_ui.lua",
        package: "avoids_modern_ui",
        data: include_bytes!("../../../../bundled-mods/avoids_modern_ui/avoids_modern_ui.lua"),
    },
    BundledFile {
        path: "avoids_modern_ui/modules/avoids_ability_bar.lua",
        package: "avoids_modern_ui",
        data: include_bytes!("../../../../bundled-mods/avoids_modern_ui/modules/avoids_ability_bar.lua"),
    },
    BundledFile {
        path: "avoids_modern_ui/modules/avoids_chat_log.lua",
        package: "avoids_modern_ui",
        data: include_bytes!("../../../../bundled-mods/avoids_modern_ui/modules/avoids_chat_log.lua"),
    },
    BundledFile {
        path: "avoids_modern_ui/modules/avoids_common.lua",
        package: "avoids_modern_ui",
        data: include_bytes!("../../../../bundled-mods/avoids_modern_ui/modules/avoids_common.lua"),
    },
    BundledFile {
        path: "avoids_modern_ui/modules/avoids_effects.lua",
        package: "avoids_modern_ui",
        data: include_bytes!("../../../../bundled-mods/avoids_modern_ui/modules/avoids_effects.lua"),
    },
    BundledFile {
        path: "avoids_modern_ui/modules/avoids_exp_bars.lua",
        package: "avoids_modern_ui",
        data: include_bytes!("../../../../bundled-mods/avoids_modern_ui/modules/avoids_exp_bars.lua"),
    },
    BundledFile {
        path: "avoids_modern_ui/modules/avoids_group_frames.lua",
        package: "avoids_modern_ui",
        data: include_bytes!("../../../../bundled-mods/avoids_modern_ui/modules/avoids_group_frames.lua"),
    },
    BundledFile {
        path: "avoids_modern_ui/modules/avoids_health_bar.lua",
        package: "avoids_modern_ui",
        data: include_bytes!("../../../../bundled-mods/avoids_modern_ui/modules/avoids_health_bar.lua"),
    },
    BundledFile {
        path: "avoids_modern_ui/modules/avoids_mana_bar.lua",
        package: "avoids_modern_ui",
        data: include_bytes!("../../../../bundled-mods/avoids_modern_ui/modules/avoids_mana_bar.lua"),
    },
    BundledFile {
        path: "avoids_modern_ui/modules/avoids_persist.lua",
        package: "avoids_modern_ui",
        data: include_bytes!("../../../../bundled-mods/avoids_modern_ui/modules/avoids_persist.lua"),
    },
    BundledFile {
        path: "avoids_modern_ui/modules/avoids_pet_frame.lua",
        package: "avoids_modern_ui",
        data: include_bytes!("../../../../bundled-mods/avoids_modern_ui/modules/avoids_pet_frame.lua"),
    },
    BundledFile {
        path: "avoids_modern_ui/modules/avoids_quest_log.lua",
        package: "avoids_modern_ui",
        data: include_bytes!("../../../../bundled-mods/avoids_modern_ui/modules/avoids_quest_log.lua"),
    },
    BundledFile {
        path: "avoids_modern_ui/modules/avoids_target_level.lua",
        package: "avoids_modern_ui",
        data: include_bytes!("../../../../bundled-mods/avoids_modern_ui/modules/avoids_target_level.lua"),
    },
    BundledFile {
        path: "avoids_modern_ui/README.md",
        package: "avoids_modern_ui",
        data: include_bytes!("../../../../bundled-mods/avoids_modern_ui/README.md"),
    },
    BundledFile {
        path: "resources/fonts/Cinzel/OFL.txt",
        package: "resources",
        data: include_bytes!("../../../../bundled-mods/resources/fonts/Cinzel/OFL.txt"),
    },
    BundledFile {
        path: "resources/fonts/Cinzel/static/Cinzel-Regular.ttf",
        package: "resources",
        data: include_bytes!("../../../../bundled-mods/resources/fonts/Cinzel/static/Cinzel-Regular.ttf"),
    },
    BundledFile {
        path: "resources/fonts/Libre_Baskerville/OFL.txt",
        package: "resources",
        data: include_bytes!("../../../../bundled-mods/resources/fonts/Libre_Baskerville/OFL.txt"),
    },
    BundledFile {
        path: "resources/fonts/Libre_Baskerville/static/LibreBaskerville-Regular.ttf",
        package: "resources",
        data: include_bytes!("../../../../bundled-mods/resources/fonts/Libre_Baskerville/static/LibreBaskerville-Regular.ttf"),
    },
    BundledFile {
        path: "resources/fonts/Merriweather_Sans/OFL.txt",
        package: "resources",
        data: include_bytes!("../../../../bundled-mods/resources/fonts/Merriweather_Sans/OFL.txt"),
    },
    BundledFile {
        path: "resources/fonts/Merriweather_Sans/static/MerriweatherSans-Regular.ttf",
        package: "resources",
        data: include_bytes!("../../../../bundled-mods/resources/fonts/Merriweather_Sans/static/MerriweatherSans-Regular.ttf"),
    },
    BundledFile {
        path: "resources/gear-icon.png",
        package: "resources",
        data: include_bytes!("../../../../bundled-mods/resources/gear-icon.png"),
    },
    BundledFile {
        path: "wereoxxs_ui/modules/ability_bars.lua",
        package: "wereoxxs_ui",
        data: include_bytes!("../../../../bundled-mods/wereoxxs_ui/modules/ability_bars.lua"),
    },
    BundledFile {
        path: "wereoxxs_ui/modules/casting_bar.lua",
        package: "wereoxxs_ui",
        data: include_bytes!("../../../../bundled-mods/wereoxxs_ui/modules/casting_bar.lua"),
    },
    BundledFile {
        path: "wereoxxs_ui/modules/chat_window.lua",
        package: "wereoxxs_ui",
        data: include_bytes!("../../../../bundled-mods/wereoxxs_ui/modules/chat_window.lua"),
    },
    BundledFile {
        path: "wereoxxs_ui/modules/class_art.lua",
        package: "wereoxxs_ui",
        data: include_bytes!("../../../../bundled-mods/wereoxxs_ui/modules/class_art.lua"),
    },
    BundledFile {
        path: "wereoxxs_ui/modules/class_icon.lua",
        package: "wereoxxs_ui",
        data: include_bytes!("../../../../bundled-mods/wereoxxs_ui/modules/class_icon.lua"),
    },
    BundledFile {
        path: "wereoxxs_ui/modules/damage_meter.lua",
        package: "wereoxxs_ui",
        data: include_bytes!("../../../../bundled-mods/wereoxxs_ui/modules/damage_meter.lua"),
    },
    BundledFile {
        path: "wereoxxs_ui/modules/meter.lua",
        package: "wereoxxs_ui",
        data: include_bytes!("../../../../bundled-mods/wereoxxs_ui/modules/meter.lua"),
    },
    BundledFile {
        path: "wereoxxs_ui/modules/paint.lua",
        package: "wereoxxs_ui",
        data: include_bytes!("../../../../bundled-mods/wereoxxs_ui/modules/paint.lua"),
    },
    BundledFile {
        path: "wereoxxs_ui/modules/party_frames.lua",
        package: "wereoxxs_ui",
        data: include_bytes!("../../../../bundled-mods/wereoxxs_ui/modules/party_frames.lua"),
    },
    BundledFile {
        path: "wereoxxs_ui/modules/solo_meter.lua",
        package: "wereoxxs_ui",
        data: include_bytes!("../../../../bundled-mods/wereoxxs_ui/modules/solo_meter.lua"),
    },
    BundledFile {
        path: "wereoxxs_ui/modules/tape_compass.lua",
        package: "wereoxxs_ui",
        data: include_bytes!("../../../../bundled-mods/wereoxxs_ui/modules/tape_compass.lua"),
    },
    BundledFile {
        path: "wereoxxs_ui/README.md",
        package: "wereoxxs_ui",
        data: include_bytes!("../../../../bundled-mods/wereoxxs_ui/README.md"),
    },
    BundledFile {
        path: "wereoxxs_ui/wereoxxs_ui.lua",
        package: "wereoxxs_ui",
        data: include_bytes!("../../../../bundled-mods/wereoxxs_ui/wereoxxs_ui.lua"),
    },
];

pub fn bundled_registry_path(config_dir: &Path) -> PathBuf {
    config_dir.join("addons").join("bundled.json")
}

pub fn load_bundled_registry(config_dir: Option<&Path>) -> BTreeSet<String> {
    let Some(config_dir) = config_dir else {
        return BTreeSet::new();
    };
    let path = bundled_registry_path(config_dir);
    let Ok(data) = fs::read_to_string(&path) else {
        return BTreeSet::new();
    };
    parse_bundled_registry(&data)
}

pub fn parse_bundled_registry(json_str: &str) -> BTreeSet<String> {
    let mut recorded = BTreeSet::new();
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
        if let Some(arr) = val.get("installed").and_then(|v| v.as_array()) {
            for item in arr {
                if let Some(s) = item.as_str() {
                    recorded.insert(s.to_string());
                }
            }
        } else if let Some(arr) = val.get("packages").and_then(|v| v.as_array()) {
            for item in arr {
                if let Some(s) = item.as_str() {
                    recorded.insert(s.to_string());
                }
            }
        } else if let Some(arr) = val.as_array() {
            for item in arr {
                if let Some(s) = item.as_str() {
                    recorded.insert(s.to_string());
                }
            }
        }
    }
    recorded
}

pub fn save_bundled_registry(config_dir: &Path, recorded: &BTreeSet<String>) -> Result<(), String> {
    let registry_file = bundled_registry_path(config_dir);
    let Some(parent) = registry_file.parent() else {
        return Err("invalid registry parent directory".into());
    };
    fs::create_dir_all(parent).map_err(|e| format!("cannot create addons dir: {e}"))?;

    let val = serde_json::json!({
        "installed": recorded.iter().cloned().collect::<Vec<_>>()
    });
    let text = serde_json::to_string_pretty(&val).map_err(|e| format!("cannot serialize registry: {e}"))?;

    let tmp_file = registry_file.with_extension(format!("tmp_{}", std::process::id()));
    fs::write(&tmp_file, text).map_err(|e| format!("cannot write tmp registry: {e}"))?;
    fs::rename(&tmp_file, &registry_file).map_err(|e| format!("cannot rename registry: {e}"))?;
    Ok(())
}

/// Installs bundled packages and shared resources into `mods_dir` if missing and unrecorded.
pub fn install_bundled_mods(
    mods_dir: &Path,
    config_dir: Option<&Path>,
) -> Result<InstallResult, String> {
    if !mods_dir.exists() {
        fs::create_dir_all(mods_dir).map_err(|e| format!("cannot create mods dir: {e}"))?;
    }

    let mut recorded = load_bundled_registry(config_dir);
    let mut result = InstallResult::default();
    let mut packages_to_extract = Vec::new();
    let mut registry_changed = false;

    for &pkg in PACKAGES {
        let pkg_dir = mods_dir.join(pkg);
        if pkg_dir.is_dir() {
            // Existing package in mods folder: record it if not already recorded, but never touch contents
            if !recorded.contains(pkg) {
                recorded.insert(pkg.to_string());
                registry_changed = true;
            }
        } else if !recorded.contains(pkg) {
            // Missing from mods folder AND not recorded in bundled.json: install it!
            packages_to_extract.push(pkg);
        }
    }

    if packages_to_extract.is_empty() {
        if registry_changed {
            if let Some(cfg) = config_dir {
                let _ = save_bundled_registry(cfg, &recorded);
            }
        }
        return Ok(result);
    }

    // Extract selected packages
    for &pkg in &packages_to_extract {
        for file in EMBEDDED_FILES {
            if file.package == pkg {
                extract_single_file(mods_dir, file, &mut result)?;
            }
        }
        recorded.insert(pkg.to_string());
        result.packages_extracted.push(pkg.to_string());
    }

    // Shared resources: add only missing files
    for file in EMBEDDED_FILES {
        if file.package == "resources" {
            extract_single_file(mods_dir, file, &mut result)?;
        }
    }

    if let Some(cfg) = config_dir {
        if let Err(e) = save_bundled_registry(cfg, &recorded) {
            crate::say(&format!("Could not save bundled.json: {e}"));
        }
    }

    Ok(result)
}

fn extract_single_file(
    mods_dir: &Path,
    file: &BundledFile,
    result: &mut InstallResult,
) -> Result<(), String> {
    let dest_path = mods_dir.join(file.path);

    // Security check: ensure normalized path stays inside mods_dir
    let normalized = normalize_path(&dest_path);
    let normalized_mods = normalize_path(mods_dir);
    if !normalized.starts_with(&normalized_mods) {
        return Err(format!("path traversal detected for {}", file.path));
    }

    if dest_path.exists() {
        result.skipped_existing += 1;
        return Ok(());
    }

    if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("cannot create parent dir for {}: {e}", file.path))?;
    }

    // Write atomically via temporary file
    let tmp_path = dest_path.with_extension(format!("tmp_{}", std::process::id()));
    fs::write(&tmp_path, file.data).map_err(|e| format!("cannot write tmp file for {}: {e}", file.path))?;
    fs::rename(&tmp_path, &dest_path).map_err(|e| format!("cannot rename tmp file to {}: {e}", file.path))?;

    result.files_extracted += 1;
    Ok(())
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for c in path.components() {
        match c {
            std::path::Component::ParentDir => {
                components.pop();
            }
            std::path::Component::CurDir => {}
            _ => components.push(c.as_os_str()),
        }
    }
    components.into_iter().collect()
}

static INSTALLED_ONCE: AtomicBool = AtomicBool::new(false);

pub fn install_bundled_mods_if_needed() {
    if INSTALLED_ONCE.swap(true, Ordering::Relaxed) {
        return;
    }
    let mods_dir = crate::fork_ext::mods::mods_dir();
    let config_dir = crate::settings::config_dir();
    match install_bundled_mods(&mods_dir, config_dir.as_deref()) {
        Ok(summary) => {
            if !summary.packages_extracted.is_empty() {
                crate::say(&format!(
                    "Installed bundled mods: {}",
                    summary.packages_extracted.join(", ")
                ));
            }
        }
        Err(e) => {
            crate::say(&format!("Could not install bundled mods: {e}"));
        }
    }
}

#[allow(dead_code)]
pub fn reset_installed_once_for_test() {
    INSTALLED_ONCE.store(false, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::collections::HashMap;

    #[test]
    fn test_embedded_files_match_bundled_mods_directory() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let bundled_dir = manifest_dir.join("../../bundled-mods");
        assert!(bundled_dir.is_dir(), "bundled-mods directory must exist at {}", bundled_dir.display());

        let mut disk_files = HashMap::new();
        for entry in walk_dir(&bundled_dir) {
            let rel = entry.strip_prefix(&bundled_dir).unwrap().to_str().unwrap().replace('\\', "/");
            if rel == "SOURCE.txt" {
                continue;
            }
            let data = fs::read(&entry).unwrap();
            let mut hasher = Sha256::new();
            hasher.update(&data);
            let hash = hasher.finalize();
            disk_files.insert(rel, (data, hash));
        }

        let mut embedded_map = HashMap::new();
        for file in EMBEDDED_FILES {
            let mut hasher = Sha256::new();
            hasher.update(file.data);
            let hash = hasher.finalize();
            assert!(
                embedded_map.insert(file.path, (file.data, hash)).is_none(),
                "duplicate embedded file: {}",
                file.path
            );
        }

        // Check for missing or extra files
        for path in disk_files.keys() {
            assert!(
                embedded_map.contains_key(path.as_str()),
                "disk file missing from embedded list: {}",
                path
            );
        }
        for path in embedded_map.keys() {
            assert!(
                disk_files.contains_key(*path),
                "embedded file missing from disk: {}",
                path
            );
        }

        // Check content / hash match
        for (path, (disk_data, disk_hash)) in &disk_files {
            let (emb_data, emb_hash) = embedded_map.get(path.as_str()).unwrap();
            assert_eq!(
                emb_hash, disk_hash,
                "hash mismatch for embedded vs disk file: {}",
                path
            );
            assert_eq!(
                *emb_data,
                disk_data.as_slice(),
                "byte mismatch for embedded vs disk file: {}",
                path
            );
        }
    }

    fn walk_dir(dir: &Path) -> Vec<PathBuf> {
        let mut files = Vec::new();
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    files.extend(walk_dir(&path));
                } else if path.is_file() {
                    files.push(path);
                }
            }
        }
        files
    }

    #[test]
    fn test_fresh_mods_folder_installs_all_packages() {
        let temp = std::env::temp_dir().join(format!("eqoa-fresh-mods-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        let mods_dir = temp.join("mods");
        let config_dir = temp.join("config");

        let res = install_bundled_mods(&mods_dir, Some(&config_dir)).unwrap();
        assert_eq!(res.packages_extracted, vec!["wereoxxs_ui", "arpg_ui", "avoids_modern_ui"]);
        assert!(res.files_extracted >= 50);

        assert!(mods_dir.join("wereoxxs_ui/wereoxxs_ui.lua").is_file());
        assert!(mods_dir.join("arpg_ui/arpg_ui.lua").is_file());
        assert!(mods_dir.join("avoids_modern_ui/avoids_modern_ui.lua").is_file());
        assert!(mods_dir.join("resources/gear-icon.png").is_file());

        let registry = load_bundled_registry(Some(&config_dir));
        assert!(registry.contains("wereoxxs_ui"));
        assert!(registry.contains("arpg_ui"));
        assert!(registry.contains("avoids_modern_ui"));

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_existing_package_is_untouched() {
        let temp = std::env::temp_dir().join(format!("eqoa-existing-pkg-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        let mods_dir = temp.join("mods");
        let config_dir = temp.join("config");

        let arpg_file = mods_dir.join("arpg_ui/arpg_ui.lua");
        fs::create_dir_all(arpg_file.parent().unwrap()).unwrap();
        fs::write(&arpg_file, "CUSTOM ARPG CONTENT").unwrap();

        let res = install_bundled_mods(&mods_dir, Some(&config_dir)).unwrap();
        // arpg_ui already existed, so only wereoxxs_ui and avoids_modern_ui were extracted
        assert_eq!(res.packages_extracted, vec!["wereoxxs_ui", "avoids_modern_ui"]);

        let arpg_content = fs::read_to_string(&arpg_file).unwrap();
        assert_eq!(arpg_content, "CUSTOM ARPG CONTENT", "existing file must be untouched");

        // Registry records all 3 so future deletes are remembered
        let registry = load_bundled_registry(Some(&config_dir));
        assert!(registry.contains("arpg_ui"));
        assert!(registry.contains("wereoxxs_ui"));
        assert!(registry.contains("avoids_modern_ui"));

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_deleted_package_is_not_reinstalled() {
        let temp = std::env::temp_dir().join(format!("eqoa-del-pkg-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        let mods_dir = temp.join("mods");
        let config_dir = temp.join("config");

        // 1. Initial install
        install_bundled_mods(&mods_dir, Some(&config_dir)).unwrap();
        assert!(mods_dir.join("wereoxxs_ui").is_dir());

        // 2. Player deletes wereoxxs_ui
        fs::remove_dir_all(mods_dir.join("wereoxxs_ui")).unwrap();
        assert!(!mods_dir.join("wereoxxs_ui").exists());

        // 3. Second run
        let res2 = install_bundled_mods(&mods_dir, Some(&config_dir)).unwrap();
        assert!(res2.packages_extracted.is_empty());
        assert!(!mods_dir.join("wereoxxs_ui").exists(), "deleted package must not be reinstalled");

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_extracted_bytes_equal_embedded_bytes() {
        let temp = std::env::temp_dir().join(format!("eqoa-bytes-pkg-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        let mods_dir = temp.join("mods");
        let config_dir = temp.join("config");

        install_bundled_mods(&mods_dir, Some(&config_dir)).unwrap();

        for file in EMBEDDED_FILES {
            let extracted_path = mods_dir.join(file.path);
            let disk_bytes = fs::read(&extracted_path).unwrap_or_else(|e| panic!("failed to read {}: {e}", extracted_path.display()));
            assert_eq!(disk_bytes.as_slice(), file.data, "byte mismatch for {}", file.path);
        }

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_unwritable_folder_gives_message_and_does_not_panic() {
        let temp = std::env::temp_dir().join(format!("eqoa-unwrite-pkg-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).unwrap();
        // Point mods_dir to a file so create_dir_all fails
        let blocker = temp.join("file_blocking_dir");
        fs::write(&blocker, b"blocking").unwrap();
        let bad_mods = blocker.join("mods");

        let config_dir = temp.join("config");
        let res = install_bundled_mods(&bad_mods, Some(&config_dir));
        assert!(res.is_err());

        let _ = fs::remove_dir_all(&temp);
    }
}
