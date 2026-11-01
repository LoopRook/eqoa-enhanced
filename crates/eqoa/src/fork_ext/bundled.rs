\\?\C:\Users\hocka\.paseo\worktrees\1usktejc\release-0-4-0\crates\eqoa\src\fork_ext\bundled.rs:

//! Auto-extracted bundled mod packages and embedded assets.
//!
//! Provides compile-time embedded copies of approved bundled mods (wereoxxs_ui,
//! arpg_ui, avoids_modern_ui) and their shared resources for auto-updating players.
//!
//! Installation behavior:
//! - Added once: If a player auto-updates to this client and an included mod is
//!   missing from their mods folder and not recorded in `bundled.json`, it is
//!   installed once (disabled by default). If a mod was partially present, any
//!   missing files are filled in; known shipped files receive safe updates.
//! - Remembers deletions: Once a package has been installed and recorded in
//!   `bundled.json`, deleting it again means it stays gone.
//! - Updates only bytes matching a previously shipped hash; player edits stay.
//! - Fails safe: If the config directory or `bundled.json` is unavailable or unwritable,
//!   or if the fallback mods folder is used, nothing is installed.
//! - Symlinks and junctions: Writes through symlinks or reparse points are rejected.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
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
    pub files_updated: usize,
    pub kept_edited: BTreeMap<String, usize>,
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
        data: include_bytes!(
            "../../../../bundled-mods/arpg_ui/resources/52x52 SpellSlotBorder.png"
        ),
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
        data: include_bytes!(
            "../../../../bundled-mods/arpg_ui/resources/itsmars_exp_highlight.png"
        ),
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
        data: include_bytes!(
            "../../../../bundled-mods/arpg_ui/resources/itsmars_orb_highlight.png"
        ),
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
        data: include_bytes!(
            "../../../../bundled-mods/avoids_modern_ui/modules/avoids_ability_bar.lua"
        ),
    },
    BundledFile {
        path: "avoids_modern_ui/modules/avoids_chat_log.lua",
        package: "avoids_modern_ui",
        data: include_bytes!(
            "../../../../bundled-mods/avoids_modern_ui/modules/avoids_chat_log.lua"
        ),
    },
    BundledFile {
        path: "avoids_modern_ui/modules/avoids_common.lua",
        package: "avoids_modern_ui",
        data: include_bytes!("../../../../bundled-mods/avoids_modern_ui/modules/avoids_common.lua"),
    },
    BundledFile {
        path: "avoids_modern_ui/modules/avoids_effects.lua",
        package: "avoids_modern_ui",
        data: include_bytes!(
            "../../../../bundled-mods/avoids_modern_ui/modules/avoids_effects.lua"
        ),
    },
    BundledFile {
        path: "avoids_modern_ui/modules/avoids_exp_bars.lua",
        package: "avoids_modern_ui",
        data: include_bytes!(
            "../../../../bundled-mods/avoids_modern_ui/modules/avoids_exp_bars.lua"
        ),
    },
    BundledFile {
        path: "avoids_modern_ui/modules/avoids_group_frames.lua",
        package: "avoids_modern_ui",
        data: include_bytes!(
            "../../../../bundled-mods/avoids_modern_ui/modules/avoids_group_frames.lua"
        ),
    },
    BundledFile {
        path: "avoids_modern_ui/modules/avoids_health_bar.lua",
        package: "avoids_modern_ui",
        data: include_bytes!(
            "../../../../bundled-mods/avoids_modern_ui/modules/avoids_health_bar.lua"
        ),
    },
    BundledFile {
        path: "avoids_modern_ui/modules/avoids_mana_bar.lua",
        package: "avoids_modern_ui",
        data: include_bytes!(
            "../../../../bundled-mods/avoids_modern_ui/modules/avoids_mana_bar.lua"
        ),
    },
    BundledFile {
        path: "avoids_modern_ui/modules/avoids_persist.lua",
        package: "avoids_modern_ui",
        data: include_bytes!(
            "../../../../bundled-mods/avoids_modern_ui/modules/avoids_persist.lua"
        ),
    },
    BundledFile {
        path: "avoids_modern_ui/modules/avoids_pet_frame.lua",
        package: "avoids_modern_ui",
        data: include_bytes!(
            "../../../../bundled-mods/avoids_modern_ui/modules/avoids_pet_frame.lua"
        ),
    },
    BundledFile {
        path: "avoids_modern_ui/modules/avoids_quest_log.lua",
        package: "avoids_modern_ui",
        data: include_bytes!(
            "../../../../bundled-mods/avoids_modern_ui/modules/avoids_quest_log.lua"
        ),
    },
    BundledFile {
        path: "avoids_modern_ui/modules/avoids_target_level.lua",
        package: "avoids_modern_ui",
        data: include_bytes!(
            "../../../../bundled-mods/avoids_modern_ui/modules/avoids_target_level.lua"
        ),
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
        data: include_bytes!(
            "../../../../bundled-mods/resources/fonts/Cinzel/static/Cinzel-Regular.ttf"
        ),
    },
    BundledFile {
        path: "resources/fonts/Libre_Baskerville/OFL.txt",
        package: "resources",
        data: include_bytes!("../../../../bundled-mods/resources/fonts/Libre_Baskerville/OFL.txt"),
    },
    BundledFile {
        path: "resources/fonts/Libre_Baskerville/static/LibreBaskerville-Regular.ttf",
        package: "resources",
        data: include_bytes!(
            "../../../../bundled-mods/resources/fonts/Libre_Baskerville/static/LibreBaskerville-Regular.ttf"
        ),
    },
    BundledFile {
        path: "resources/fonts/Merriweather_Sans/OFL.txt",
        package: "resources",
        data: include_bytes!("../../../../bundled-mods/resources/fonts/Merriweather_Sans/OFL.txt"),
    },
    BundledFile {
        path: "resources/fonts/Merriweather_Sans/static/MerriweatherSans-Regular.ttf",
        package: "resources",
        data: include_bytes!(
            "../../../../bundled-mods/resources/fonts/Merriweather_Sans/static/MerriweatherSans-Regular.ttf"
        ),
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

const MAX_REGISTRY_BYTES: usize = 1024 * 1024;
const MAX_BUNDLE_FILE_BYTES: usize = 16 * 1024 * 1024;
type ShippedFiles = BTreeMap<String, BTreeSet<String>>;

#[derive(Default)]
struct Registry {
    // Preserve the old per-folder package lists, including deletion records.
    folders: BTreeMap<String, BTreeSet<String>>,
    shipped: BTreeMap<String, ShippedFiles>,
}

fn no_links(path: &Path) -> Result<(), String> {
    for component in path.ancestors() {
        match fs::symlink_metadata(component) {
            Ok(meta) => {
                let mut link = meta.file_type().is_symlink();
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    link |= meta.file_attributes() & 0x400 != 0;
                }
                if link {
                    return Err(format!(
                        "link/reparse point in bundled path: {}",
                        component.display()
                    ));
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("cannot inspect bundled path: {e}")),
        }
    }
    Ok(())
}

fn read_bounded(path: &Path, limit: usize) -> Result<Option<Vec<u8>>, String> {
    no_links(path)?;
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("cannot read bundle file: {e}")),
    };
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("bundle path is not a regular file".into());
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err("bundled file exceeds size limit".into());
    }
    Ok(Some(bytes))
}

fn read_registry(config_dir: &Path) -> Result<Registry, String> {
    let Some(bytes) = read_bounded(&bundled_registry_path(config_dir), MAX_REGISTRY_BYTES)? else {
        return Ok(Registry::default());
    };
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| format!("cannot read bundled.json: {e}"))?;
    if !value.is_object() {
        return Err("bundled.json must be an object".into());
    }
    Ok(Registry {
        folders: serde_json::from_value(
            value
                .get("folders")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({})),
        )
        .map_err(|e| format!("invalid bundled folder records: {e}"))?,
        shipped: serde_json::from_value(
            value
                .get("shipped")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({})),
        )
        .map_err(|e| format!("invalid bundled hash records: {e}"))?,
    })
}

pub fn load_bundled_registry_map(config_dir: Option<&Path>) -> BTreeMap<String, BTreeSet<String>> {
    config_dir
        .and_then(|cfg| read_registry(cfg).ok())
        .map(|r| r.folders)
        .unwrap_or_default()
}

pub fn load_bundled_registry(config_dir: Option<&Path>, mods_dir: &Path) -> BTreeSet<String> {
    let key = mods_folder_key(mods_dir);
    load_bundled_registry_map(config_dir)
        .get(&key)
        .cloned()
        .unwrap_or_default()
}

fn save_bundled_registry(config_dir: &Path, registry: &Registry) -> Result<(), String> {
    let path = bundled_registry_path(config_dir);
    no_links(&path)?;
    let bytes = serde_json::to_vec_pretty(
        &serde_json::json!({"folders": registry.folders, "shipped": registry.shipped}),
    )
    .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_REGISTRY_BYTES {
        return Err("bundled.json exceeds 1 MiB".into());
    }
    super::addons::storage::atomic_replace(&path, &bytes)
        .map_err(|e| format!("cannot write bundled.json: {e}"))
}

fn hash(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}

fn shipped_hashes(file: &BundledFile) -> BTreeSet<String> {
    let mut hashes = BTreeSet::from([hash(file.data)]);
    // Bootstrap pre-version-registry installs using actual previously shipped
    // manifests. Never learn a trusted hash from the player's current bytes.
    for source in [
        include_str!("bundled_shipped_hashes.txt"),
        include_str!("../../../../bundled-mods/SOURCE.txt"),
    ] {
        for line in source.lines() {
            if let Some((sha, path)) = line.split_once("  ") {
                if path == file.path
                    && sha.len() == 64
                    && sha.bytes().all(|b| b.is_ascii_hexdigit())
                {
                    hashes.insert(sha.to_ascii_lowercase());
                }
            }
        }
    }
    hashes
}

/// Update installed packages, complete first installs, remember package deletions.
pub fn install_bundled_mods(
    mods_dir: &Path,
    config_dir: Option<&Path>,
) -> Result<InstallResult, String> {
    if mods_dir.file_name().and_then(|n| n.to_str()) == Some("eqoa-mods-unavailable") {
        return Err("cannot install bundled mods into unavailable mods folder fallback".into());
    }
    let cfg = config_dir.ok_or("config directory unavailable")?;
    no_links(mods_dir)?;
    let mut registry = read_registry(cfg)?;
    let key = mods_folder_key(mods_dir);
    let mut recorded = registry.folders.get(&key).cloned().unwrap_or_default();
    let mut packages = Vec::new();
    for &pkg in PACKAGES {
        let path = mods_dir.join(pkg);
        no_links(&path)?;
        if !recorded.contains(pkg) || path.is_dir() {
            packages.push(pkg);
        }
    }
    if !packages.is_empty() {
        no_links(&mods_dir.join("resources"))?;
        packages.push("resources");
    }
    let files = EMBEDDED_FILES
        .iter()
        .filter(|f| packages.contains(&f.package))
        .collect::<Vec<_>>();
    let shipped = registry.shipped.entry(key.clone()).or_default();
    for file in &files {
        shipped
            .entry(file.path.to_string())
            .or_default()
            .extend(shipped_hashes(file));
    }
    registry.folders.insert(key.clone(), recorded.clone());
    // Check writable registry before touching package files; preserve histories
    // on partial failures so a later start can finish safely.
    save_bundled_registry(cfg, &registry)?;
    let mut result = InstallResult::default();
    for &pkg in &packages {
        for file in files.iter().filter(|f| f.package == pkg) {
            extract_single_file(
                mods_dir,
                file,
                &registry.shipped[&key][file.path],
                &mut result,
            )?;
        }
        if pkg != "resources" && recorded.insert(pkg.to_string()) {
            result.packages_extracted.push(pkg.to_string());
        }
    }
    registry.folders.insert(key, recorded);
    save_bundled_registry(cfg, &registry)?;
    for (package, count) in &result.kept_edited {
        crate::say(&format!(
            "Bundled mods: {package}: kept {count} player-edited file(s)"
        ));
    }
    Ok(result)
}

fn extract_single_file(
    mods_dir: &Path,
    file: &BundledFile,
    known: &BTreeSet<String>,
    result: &mut InstallResult,
) -> Result<(), String> {
    let path = mods_dir.join(file.path);
    if !normalize_path(&path).starts_with(normalize_path(mods_dir)) {
        return Err(format!("path traversal detected for {}", file.path));
    }
    no_links(&path)?;
    let existing = fs::symlink_metadata(&path).ok();
    if let Some(meta) = &existing {
        if !meta.is_file() {
            return Err(format!("not a regular bundled file: {}", file.path));
        }
        // Oversized local files cannot be ours. Keep them without reading them.
        if meta.len() > MAX_BUNDLE_FILE_BYTES as u64 {
            result.skipped_existing += 1;
            *result.kept_edited.entry(file.package.into()).or_default() += 1;
            return Ok(());
        }
    }
    if let Some(current) = read_bounded(&path, MAX_BUNDLE_FILE_BYTES)? {
        result.skipped_existing += 1;
        if current == file.data {
            return Ok(());
        }
        if !known.contains(&hash(&current)) {
            *result.kept_edited.entry(file.package.into()).or_default() += 1;
            return Ok(());
        }
    }
    if file.data.len() > MAX_BUNDLE_FILE_BYTES {
        return Err("embedded bundle file exceeds size limit".into());
    }
    super::addons::storage::atomic_replace(&path, file.data)
        .map_err(|e| format!("cannot write {}: {e}", file.path))?;
    if existing.is_some() {
        result.files_updated += 1;
    } else {
        result.files_extracted += 1;
    }
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

#[cfg(test)]
#[path = "bundled_update_tests.rs"]
mod update_tests;

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
        assert!(arpg_lua.is_file(), "missing files inside an installed package are restored");

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
