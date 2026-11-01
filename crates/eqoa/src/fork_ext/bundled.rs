//! Auto-extracted bundled mod packages and embedded assets.
//!
//! Provides compile-time embedded copies of approved bundled mods (wereoxxs_ui,
//! arpg_ui, avoids_modern_ui) and their shared resources for auto-updating players.
//!
//! Installation behavior:
//! - Added once: If a player auto-updates to this client and an included mod is
//!   missing from their mods folder and not recorded in `bundled.json`, it is
//!   installed once (disabled by default). If a mod was partially present, any
//!   missing files are filled in, while existing files keep their bytes.
//! - Remembers deletions: Once a package has been installed and recorded in
//!   `bundled.json`, deleting it again means it stays gone.
//! - Never overwrites: Existing files are never overwritten.
//! - Fails safe: If the config directory or `bundled.json` is unavailable or unwritable,
//!   or if the fallback mods folder is used, nothing is installed.
//! - Symlinks and junctions: Writes through symlinks or reparse points are rejected.

use std::collections::{BTreeMap, BTreeSet};
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

fn canonicalize_best_effort(path: &Path) -> PathBuf {
    if let Ok(c) = path.canonicalize() {
        return c;
    }
    let mut tail = Vec::new();
    let mut cur = path;
    while let Some(parent) = cur.parent() {
        if let Some(name) = cur.file_name() {
            tail.push(name);
        }
        if let Ok(c_parent) = parent.canonicalize() {
            let mut result = c_parent;
            for component in tail.into_iter().rev() {
                result.push(component);
            }
            return result;
        }
        cur = parent;
    }
    normalize_path(path)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn mods_folder_key(mods_dir: &Path) -> String {
    use sha2::{Digest, Sha256};
    let canonical = canonicalize_best_effort(mods_dir);
    let mut s = canonical.to_string_lossy().into_owned();
    if let Some(stripped) = s.strip_prefix(r"\\?\UNC\") {
        s = format!(r"\\{stripped}");
    } else if let Some(stripped) = s.strip_prefix(r"\\?\") {
        s = stripped.to_string();
    }
    #[cfg(windows)]
    {
        s = s.replace('/', "\\").to_lowercase();
    }
    #[cfg(not(windows))]
    {
        s = s.replace('\\', "/");
    }

    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(target_arch = "wasm32")]
pub fn mods_folder_key(_mods_dir: &Path) -> String {
    String::new()
}

pub fn parse_bundled_registry(json_str: &str) -> BTreeMap<String, BTreeSet<String>> {
    let mut folders = BTreeMap::new();
    let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) else {
        return folders;
    };

    if let Some(folders_obj) = val.get("folders").and_then(|v| v.as_object()) {
        for (k, v) in folders_obj {
            if let Some(arr) = v.as_array() {
                let pkgs: BTreeSet<String> = arr
                    .iter()
                    .filter_map(|x| x.as_str().map(String::from))
                    .collect();
                folders.insert(k.clone(), pkgs);
            }
        }
    }
    // Any legacy flat format (without "folders") is ignored and will be rewritten in the new format
    folders
}

pub fn load_bundled_registry_map(config_dir: Option<&Path>) -> BTreeMap<String, BTreeSet<String>> {
    let Some(config_dir) = config_dir else {
        return BTreeMap::new();
    };
    let path = bundled_registry_path(config_dir);
    let Ok(data) = fs::read_to_string(&path) else {
        return BTreeMap::new();
    };
    parse_bundled_registry(&data)
}

pub fn load_bundled_registry(config_dir: Option<&Path>, mods_dir: &Path) -> BTreeSet<String> {
    let key = mods_folder_key(mods_dir);
    let all_folders = load_bundled_registry_map(config_dir);
    all_folders.get(&key).cloned().unwrap_or_default()
}

pub fn save_bundled_registry(
    config_dir: &Path,
    folders: &BTreeMap<String, BTreeSet<String>>,
) -> Result<(), String> {
    let registry_file = bundled_registry_path(config_dir);
    let Some(parent) = registry_file.parent() else {
        return Err("invalid registry parent directory".into());
    };
    fs::create_dir_all(parent).map_err(|e| format!("cannot create addons dir: {e}"))?;

    let val = serde_json::json!({
        "folders": folders
    });
    let text = serde_json::to_string_pretty(&val).map_err(|e| format!("cannot serialize registry: {e}"))?;

    let tmp_file = registry_file.with_extension(format!("tmp_{}", std::process::id()));
    let res = fs::write(&tmp_file, text)
        .map_err(|e| format!("cannot write tmp registry: {e}"))
        .and_then(|_| {
            fs::rename(&tmp_file, &registry_file).map_err(|e| format!("cannot rename registry: {e}"))
        });
    if res.is_err() && tmp_file.exists() {
        let _ = fs::remove_file(&tmp_file);
    }
    res
}

/// Installs bundled packages and shared resources into `mods_dir` if missing and unrecorded.
pub fn install_bundled_mods(
    mods_dir: &Path,
    config_dir: Option<&Path>,
) -> Result<InstallResult, String> {
    if mods_dir.file_name().and_then(|n| n.to_str()) == Some("eqoa-mods-unavailable") {
        let err = "cannot install bundled mods into unavailable mods folder fallback".to_string();
        crate::say(&format!("Could not install bundled mods: {err}"));
        return Err(err);
    }

    let Some(cfg) = config_dir else {
        let err = "config directory unavailable".to_string();
        crate::say(&format!("Could not install bundled mods: {err}"));
        return Err(err);
    };

    let folder_key = mods_folder_key(mods_dir);
    let mut all_folders = load_bundled_registry_map(Some(cfg));
    let mut recorded = all_folders.get(&folder_key).cloned().unwrap_or_default();
    let mut result = InstallResult::default();
    let mut packages_to_extract = Vec::new();

    for &pkg in PACKAGES {
        if !recorded.contains(pkg) {
            packages_to_extract.push(pkg);
        }
    }

    if packages_to_extract.is_empty() {
        return Ok(result);
    }

    // Verify registry is writable before creating dirs or writing files
    all_folders.insert(folder_key.clone(), recorded.clone());
    save_bundled_registry(cfg, &all_folders).map_err(|e| {
        let err = format!("cannot write bundled.json registry: {e}");
        crate::say(&format!("Could not install bundled mods: {err}"));
        err
    })?;

    if !mods_dir.exists() {
        fs::create_dir_all(mods_dir).map_err(|e| format!("cannot create mods dir: {e}"))?;
    }

    // Extract selected packages (only missing files added, existing files kept)
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

    all_folders.insert(folder_key, recorded);
    save_bundled_registry(cfg, &all_folders).map_err(|e| {
        let err = format!("cannot save bundled.json registry: {e}");
        crate::say(&format!("Could not install bundled mods: {err}"));
        err
    })?;

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

    // Symlink / junction / reparse point check on every component between mods_dir and dest_path
    for p in dest_path.ancestors() {
        if p == mods_dir {
            break;
        }
        if let Ok(meta) = fs::symlink_metadata(p) {
            if meta.file_type().is_symlink() {
                let msg = format!("symlink in bundled mod path component: {}", p.display());
                crate::say(&format!("Cannot extract {}: {msg}", file.path));
                return Err(msg);
            }
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if meta.file_attributes() & 0x400 != 0 {
                    let msg = format!("reparse point in bundled mod path component: {}", p.display());
                    crate::say(&format!("Cannot extract {}: {msg}", file.path));
                    return Err(msg);
                }
            }
        }
    }

    if dest_path.exists() {
        result.skipped_existing += 1;
        return Ok(());
    }

    if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("cannot create parent dir for {}: {e}", file.path))?;
    }

    // Write atomically via temporary file, ensuring cleanup on failure
    let tmp_path = dest_path.with_extension(format!("tmp_{}", std::process::id()));
    let res = fs::write(&tmp_path, file.data)
        .map_err(|e| format!("cannot write tmp file for {}: {e}", file.path))
        .and_then(|_| {
            fs::rename(&tmp_path, &dest_path)
                .map_err(|e| format!("cannot rename tmp file to {}: {e}", file.path))
        });
    if res.is_err() && tmp_path.exists() {
        let _ = fs::remove_file(&tmp_path);
    }
    res?;

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
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mods_dir = std::env::var_os("EQOA_FORGE_MODS")
            .map(PathBuf::from)
            .unwrap_or_else(crate::fork_ext::mods::mods_dir);
        let config_dir = std::env::var_os("EQOA_CONFIG")
            .map(PathBuf::from)
            .or_else(crate::settings::config_dir);
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

        let registry = load_bundled_registry(Some(&config_dir), &mods_dir);
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
        assert!(res.packages_extracted.contains(&"arpg_ui".to_string()));

        let arpg_content = fs::read_to_string(&arpg_file).unwrap();
        assert_eq!(arpg_content, "CUSTOM ARPG CONTENT", "existing file must be untouched");

        // Registry records all 3 so future deletes are remembered
        let registry = load_bundled_registry(Some(&config_dir), &mods_dir);
        assert!(registry.contains("arpg_ui"));
        assert!(registry.contains("wereoxxs_ui"));
        assert!(registry.contains("avoids_modern_ui"));

        // Subsequent run skips all recorded packages
        let res2 = install_bundled_mods(&mods_dir, Some(&config_dir)).unwrap();
        assert!(res2.packages_extracted.is_empty());

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

    #[test]
    fn test_partially_present_unrecorded_package_gets_completed_and_recorded() {
        let temp = std::env::temp_dir().join(format!("eqoa-partial-pkg-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        let mods_dir = temp.join("mods");
        let config_dir = temp.join("config");

        let arpg_readme = mods_dir.join("arpg_ui/README.md");
        fs::create_dir_all(arpg_readme.parent().unwrap()).unwrap();
        fs::write(&arpg_readme, "ORIGINAL CUSTOM README").unwrap();

        let arpg_lua = mods_dir.join("arpg_ui/arpg_ui.lua");
        assert!(!arpg_lua.exists());

        let res = install_bundled_mods(&mods_dir, Some(&config_dir)).unwrap();
        assert!(res.packages_extracted.contains(&"arpg_ui".to_string()));
        assert!(arpg_lua.is_file(), "missing file in unrecorded package must be extracted");
        assert_eq!(
            fs::read_to_string(&arpg_readme).unwrap(),
            "ORIGINAL CUSTOM README",
            "existing file must keep its original bytes"
        );

        let registry = load_bundled_registry(Some(&config_dir), &mods_dir);
        assert!(registry.contains("arpg_ui"));

        fs::remove_file(&arpg_lua).unwrap();
        let res2 = install_bundled_mods(&mods_dir, Some(&config_dir)).unwrap();
        assert!(!res2.packages_extracted.contains(&"arpg_ui".to_string()));
        assert!(!arpg_lua.exists(), "once recorded, deletions are respected");

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_unavailable_config_dir_installs_nothing() {
        let temp = std::env::temp_dir().join(format!("eqoa-noconfig-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        let mods_dir = temp.join("mods");

        let res = install_bundled_mods(&mods_dir, None);
        assert!(res.is_err(), "must fail when config_dir is None");
        assert!(!mods_dir.exists() || fs::read_dir(&mods_dir).unwrap().next().is_none(), "must install nothing");

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_fallback_mods_dir_installs_nothing() {
        let temp = std::env::temp_dir().join(format!("eqoa-fallback-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        let bad_mods_dir = temp.join("eqoa-mods-unavailable");
        let config_dir = temp.join("config");

        let res = install_bundled_mods(&bad_mods_dir, Some(&config_dir));
        assert!(res.is_err(), "must reject fallback mods folder");
        assert!(!bad_mods_dir.exists() || fs::read_dir(&bad_mods_dir).unwrap().next().is_none(), "must install nothing");

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_unwritable_registry_installs_nothing() {
        let temp = std::env::temp_dir().join(format!("eqoa-unwrite-reg-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        let mods_dir = temp.join("mods");
        let config_dir = temp.join("config");

        let blocker = config_dir.join("addons");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(&blocker, b"blocking").unwrap();

        let res = install_bundled_mods(&mods_dir, Some(&config_dir));
        assert!(res.is_err(), "must fail when registry cannot be written");
        assert!(!mods_dir.exists() || fs::read_dir(&mods_dir).unwrap().next().is_none(), "must install nothing");

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_symlink_or_junction_in_mods_path_rejected() {
        let temp = std::env::temp_dir().join(format!("eqoa-symlink-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        let mods_dir = temp.join("mods");
        let config_dir = temp.join("config");
        let outside_dir = temp.join("outside");
        fs::create_dir_all(&outside_dir).unwrap();
        fs::create_dir_all(&mods_dir).unwrap();

        let link_target = mods_dir.join("arpg_ui");
        let mut created = false;

        #[cfg(windows)]
        {
            if std::os::windows::fs::symlink_dir(&outside_dir, &link_target).is_ok() {
                created = true;
            } else {
                let status = std::process::Command::new("cmd")
                    .args(&["/c", "mklink", "/J", link_target.to_str().unwrap(), outside_dir.to_str().unwrap()])
                    .status();
                if status.map(|s| s.success()).unwrap_or(false) {
                    created = true;
                }
            }
        }
        #[cfg(unix)]
        {
            if std::os::unix::fs::symlink(&outside_dir, &link_target).is_ok() {
                created = true;
            }
        }

        if created {
            let res = install_bundled_mods(&mods_dir, Some(&config_dir));
            assert!(res.is_err(), "must reject symlink/junction in mods path");
            assert!(!outside_dir.join("arpg_ui.lua").exists(), "must not write through symlink or junction");
        } else {
            eprintln!("SKIP symlink test: neither symlink nor junction could be created");
        }

        let _ = fs::remove_dir_all(&temp);
    }
    #[test]
    fn test_from_env_setup_discovers_all_three_bundled_packages_on_fresh_mods_folder() {
        let temp = std::env::temp_dir().join(format!("eqoa-fresh-from-env-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        let mods_dir = temp.join("mods");
        let config_dir = temp.join("config");

        // Simulate from_env: fresh mods folder, install synchronously before discovery
        reset_installed_once_for_test();
        let res = install_bundled_mods(&mods_dir, Some(&config_dir)).unwrap();
        assert_eq!(res.packages_extracted, vec!["wereoxxs_ui", "arpg_ui", "avoids_modern_ui"]);

        let discovery = crate::fork_ext::addons::discovery::discover(&mods_dir);
        let ids: BTreeSet<String> = discovery.addons.iter().map(|a| a.id.clone()).collect();
        assert!(ids.contains("wereoxxs_ui"), "wereoxxs_ui must be discovered on first start");
        assert!(ids.contains("arpg_ui"), "arpg_ui must be discovered on first start");
        assert!(ids.contains("avoids_modern_ui"), "avoids_modern_ui must be discovered on first start");
        assert_eq!(ids.len(), 3);

        let _ = fs::remove_dir_all(&temp);
    }
    #[test]
    fn test_second_mods_folder_gets_all_packages_when_first_folder_has_records() {
        let temp = std::env::temp_dir().join(format!("eqoa-multi-folder-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        let mods_dir_a = temp.join("install_a/mods");
        let mods_dir_b = temp.join("install_b/mods");
        let config_dir = temp.join("config");

        // First install into folder A
        let res_a = install_bundled_mods(&mods_dir_a, Some(&config_dir)).unwrap();
        assert_eq!(res_a.packages_extracted, vec!["wereoxxs_ui", "arpg_ui", "avoids_modern_ui"]);

        // Now install into folder B (different install location)
        let res_b = install_bundled_mods(&mods_dir_b, Some(&config_dir)).unwrap();
        assert_eq!(
            res_b.packages_extracted,
            vec!["wereoxxs_ui", "arpg_ui", "avoids_modern_ui"],
            "second mods folder must get all packages even when first folder has records"
        );
        assert!(mods_dir_b.join("wereoxxs_ui/wereoxxs_ui.lua").is_file());
        assert!(mods_dir_b.join("arpg_ui/arpg_ui.lua").is_file());
        assert!(mods_dir_b.join("avoids_modern_ui/avoids_modern_ui.lua").is_file());

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_deletion_in_folder_a_still_remembered() {
        let temp = std::env::temp_dir().join(format!("eqoa-del-folder-a-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        let mods_dir_a = temp.join("install_a/mods");
        let mods_dir_b = temp.join("install_b/mods");
        let config_dir = temp.join("config");

        // Install both
        install_bundled_mods(&mods_dir_a, Some(&config_dir)).unwrap();
        install_bundled_mods(&mods_dir_b, Some(&config_dir)).unwrap();

        assert!(mods_dir_a.join("wereoxxs_ui").is_dir());
        assert!(mods_dir_b.join("wereoxxs_ui").is_dir());

        // Player deletes wereoxxs_ui in folder A
        fs::remove_dir_all(mods_dir_a.join("wereoxxs_ui")).unwrap();
        assert!(!mods_dir_a.join("wereoxxs_ui").exists());

        // Second run on folder A respects deletion
        let res_a2 = install_bundled_mods(&mods_dir_a, Some(&config_dir)).unwrap();
        assert!(res_a2.packages_extracted.is_empty());
        assert!(!mods_dir_a.join("wereoxxs_ui").exists(), "deleted package in folder A must not be reinstalled");

        // Folder B still has its copy intact
        assert!(mods_dir_b.join("wereoxxs_ui").is_dir(), "folder B retains its installed package");

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_flat_registry_listing_all_three_packages_installs_all_three_on_fresh_mods_folder() {
        let temp = std::env::temp_dir().join(format!("eqoa-flat-ignored-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        let mods_dir = temp.join("install_fresh/mods");
        let config_dir = temp.join("config");
        let registry_file = bundled_registry_path(&config_dir);

        // Pre-create flat registry listing all three packages
        fs::create_dir_all(registry_file.parent().unwrap()).unwrap();
        let flat_json = serde_json::json!({
            "installed": ["wereoxxs_ui", "arpg_ui", "avoids_modern_ui"]
        });
        fs::write(&registry_file, serde_json::to_string_pretty(&flat_json).unwrap()).unwrap();

        // Run install: flat registry must be ignored on fresh folder, installing all three
        let res = install_bundled_mods(&mods_dir, Some(&config_dir)).unwrap();
        assert_eq!(
            res.packages_extracted,
            vec!["wereoxxs_ui", "arpg_ui", "avoids_modern_ui"],
            "flat registry must be ignored so fresh mods folder gets all packages"
        );
        assert!(mods_dir.join("wereoxxs_ui/wereoxxs_ui.lua").is_file());
        assert!(mods_dir.join("arpg_ui/arpg_ui.lua").is_file());
        assert!(mods_dir.join("avoids_modern_ui/avoids_modern_ui.lua").is_file());

        // Verify registry file has been rewritten in the new per-folder format
        let new_registry_data = fs::read_to_string(&registry_file).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&new_registry_data).unwrap();
        let key = mods_folder_key(&mods_dir);
        let folders = parsed.get("folders").expect("registry must have 'folders' object");
        let recorded = folders.get(&key).expect("registry must record current folder under its SHA-256 key");
        let recorded_arr: Vec<String> = recorded.as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_string()).collect();
        assert!(recorded_arr.contains(&"wereoxxs_ui".to_string()));
        assert!(recorded_arr.contains(&"arpg_ui".to_string()));
        assert!(recorded_arr.contains(&"avoids_modern_ui".to_string()));

        // Ensure no raw filesystem path is stored anywhere in bundled.json
        let raw_path = mods_dir.to_string_lossy();
        assert!(!new_registry_data.contains(raw_path.as_ref()), "bundled.json must never contain raw folder paths");

        let _ = fs::remove_dir_all(&temp);
    }
}
