// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Bounded plain-data profiles and versioned enabled state for native addons.
//!
//! Contract: caller config/addons; schema-1 enabled registry; bounded plain JSON
//! profiles via profile::{encode,decode}. Identity uses SHA-256 over a folder-independent
//! domain (v2) hashing length-delimited relative entry/package paths (relative to mods root)
//! and opaque ID. Legacy v1 digest is retained for transparent migration.
//! Raw IDs never become filenames.
//! Reads use take(1 MiB + 1). Replacement uses create_new same-directory temps,
//! flush/sync then rename, never deleting the destination first. A failed rename
//! preserves the old target and removes only this write's temporary file.

use super::discovery::DiscoveredAddon;
use mlua::{Lua, Value};
use serde_json::{Value as Json, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const MAX_JSON_BYTES: usize = 1024 * 1024;

/// Reusable scanner to bound sibling directory scanning across addons during a single rescan.
#[derive(Default, Debug)]
pub struct SiblingScanner {
    scanned: bool,
    sibling_roots: Vec<PathBuf>,
    pub listing_count: usize,
}

impl SiblingScanner {
    pub fn new() -> Self {
        Self::default()
    }

    /// Scans the parent directory of `game_root` for candidate sibling installs,
    /// running at most once per rescan, examining at most 64 non-hidden, non-system directories.
    pub fn scan(&mut self, mods_root: &Path) -> &[PathBuf] {
        if self.scanned {
            return &self.sibling_roots;
        }
        self.scanned = true;

        let Some(game_root) = mods_root.parent() else {
            return &self.sibling_roots;
        };
        let Some(parent) = game_root.parent() else {
            return &self.sibling_roots;
        };

        let Ok(entries) = fs::read_dir(parent) else {
            return &self.sibling_roots;
        };
        self.listing_count += 1;

        const MAX_SIBLINGS: usize = 64;
        let mut count = 0;

        for entry in entries.flatten() {
            if count >= MAX_SIBLINGS {
                break;
            }
            if is_hidden_or_system(&entry) {
                continue;
            }
            let path = entry.path();
            if path != game_root && path.is_dir() {
                self.sibling_roots.push(path);
                count += 1;
            }
        }

        &self.sibling_roots
    }
}

fn is_hidden_or_system(entry: &fs::DirEntry) -> bool {
    let name = entry.file_name();
    let name_str = name.to_string_lossy();
    if name_str.starts_with('.') || name_str.starts_with('$') {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if let Ok(meta) = entry.metadata() {
            let attrs = meta.file_attributes();
            const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
            const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;
            if attrs & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM) != 0 {
                return true;
            }
        }
    }
    false
}

#[derive(Clone)]
pub(crate) struct Storage {
    root: Option<PathBuf>,
}

impl Storage {
    pub fn new(config: Option<PathBuf>) -> Self {
        Self {
            root: config.map(|root| root.join("addons")),
        }
    }

    /// Computes the folder-independent v2 storage key for an addon.
    pub fn key(addon: &DiscoveredAddon) -> Result<String, String> {
        let mods_root = addon.mods_root();
        identity_relative(&addon.entry, &addon.package_root, &mods_root, &addon.id)
    }

    /// Computes the folder-independent v2 storage key for a standard `<id>/main.lua` addon.
    pub(crate) fn key_for_addon_id(id: &str) -> String {
        let entry = format!("{id}/main.lua");
        digest_fields_v2(entry.as_bytes(), id.as_bytes(), id.as_bytes())
    }

    /// Computes the legacy v1 storage key (hashing absolute paths). Retained for migration.
    pub fn v1_key(addon: &DiscoveredAddon) -> Result<String, String> {
        identity_v1(&addon.entry, &addon.package_root, &addon.id)
    }

    /// Discovers candidate legacy v1 keys for migration without performing directory listings.
    pub fn candidate_v1_keys(addon: &DiscoveredAddon) -> Vec<String> {
        Self::candidate_v1_keys_with_siblings(addon, &[])
    }

    /// Discovers candidate legacy v1 keys for migration with explicitly provided sibling roots.
    pub fn candidate_v1_keys_with_siblings(
        addon: &DiscoveredAddon,
        sibling_roots: &[PathBuf],
    ) -> Vec<String> {
        let mut keys = Vec::new();
        let mut seen = std::collections::HashSet::new();

        let mut add = |entry: &Path, pkg: &Path| {
            if let Ok(k) = identity_v1(entry, pkg, &addon.id) {
                if seen.insert(k.clone()) {
                    keys.push(k);
                }
            }
        };

        // 1. Current entry & package
        add(&addon.entry, &addon.package_root);

        // 2. Alternative entry points in current package
        for name in ["main.lua", "init.lua", &format!("{}.lua", addon.id)] {
            add(&addon.package_root.join(name), &addon.package_root);
        }

        // 3. Sibling folders of the install root
        let mods_root = addon.mods_root();
        let rel_pkg = relative_to_root(&addon.package_root, &mods_root);
        let rel_entry = relative_to_root(&addon.entry, &mods_root);

        for sibling_game_root in sibling_roots {
            let sibling_mods = sibling_game_root.join("mods");
            let sibling_pkg = sibling_mods.join(&rel_pkg);
            let sibling_entry = sibling_mods.join(&rel_entry);
            if sibling_pkg.exists() {
                add(&sibling_entry, &sibling_pkg);
            }
        }

        keys
    }

    /// Migrates legacy v1 profile data and state.json enabled flags to the folder-independent v2 key.
    pub fn migrate_addon(&self, addon: &DiscoveredAddon) -> Result<Vec<String>, String> {
        self.migrate_addon_with_candidates(addon, &[])
    }

    /// Migrates legacy v1 profile data using a shared `SiblingScanner` to bound parent directory listings.
    pub fn migrate_addon_with_scanner(
        &self,
        addon: &DiscoveredAddon,
        scanner: &mut SiblingScanner,
    ) -> Result<Vec<String>, String> {
        let new_key = Self::key(addon)?;
        let new_profile = self.profile_path(&new_key)?;
        let has_v2_profile = new_profile.is_file();

        let current_v1_key = Self::v1_key(addon)?;
        let current_v1_profile = self.profile_path(&current_v1_key)?;
        let has_current_v1 = current_v1_profile.is_file();

        let candidates = if !has_v2_profile && !has_current_v1 {
            // Only search sibling directories if there is NO v2 profile AND NO v1 profile for the current location
            let siblings = scanner.scan(&addon.mods_root());
            Self::candidate_v1_keys_with_siblings(addon, siblings)
        } else {
            Self::candidate_v1_keys(addon)
        };

        self.migrate_addon_with_candidate_list(addon, &new_key, &candidates)
    }

    /// Migrates legacy v1 profile data with optional explicit candidate keys (useful in tests).
    pub fn migrate_addon_with_candidates(
        &self,
        addon: &DiscoveredAddon,
        extra_candidates: &[String],
    ) -> Result<Vec<String>, String> {
        let new_key = Self::key(addon)?;
        let mut candidates = Self::candidate_v1_keys(addon);
        for extra in extra_candidates {
            if !candidates.contains(extra) {
                candidates.push(extra.clone());
            }
        }
        self.migrate_addon_with_candidate_list(addon, &new_key, &candidates)
    }

    fn migrate_addon_with_candidate_list(
        &self,
        addon: &DiscoveredAddon,
        new_key: &str,
        candidates: &[String],
    ) -> Result<Vec<String>, String> {
        let mut messages = Vec::new();
        let new_profile = self.profile_path(new_key)?;

        let mut matches: Vec<(String, PathBuf, std::time::SystemTime)> = Vec::new();
        for cand in candidates {
            if let Ok(path) = self.profile_path(cand) {
                if path.is_file() {
                    let mtime = fs::metadata(&path)
                        .and_then(|m| m.modified())
                        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                    matches.push((cand.clone(), path, mtime));
                }
            }
        }

        // Sort by mtime descending (most recently modified first)
        matches.sort_by(|a, b| b.2.cmp(&a.2));

        let best_candidate = matches.first().map(|(k, p, _)| (k.clone(), p.clone()));

        if !new_profile.is_file() {
            if let Some((best_key, best_path)) = &best_candidate {
                if matches.len() > 1 {
                    messages.push(format!(
                        "multiple legacy v1 profiles found for addon '{}'; selected most recently modified key {}",
                        addon.id, best_key
                    ));
                }
                messages.push(format!(
                    "migrated profile from legacy v1 key {} to {}",
                    best_key, new_key
                ));

                let bytes = fs::read(best_path)
                    .map_err(|e| format!("failed to read legacy v1 profile {}: {e}", best_key))?;
                atomic_replace(&new_profile, &bytes)
                    .map_err(|e| format!("failed to write migrated profile {}: {e}", new_key))?;

                // Migrate overlay files directory if present
                if let Ok(from_files) = self.files_dir(best_key) {
                    if from_files.is_dir() {
                        let to_files = self.files_dir(new_key)?;
                        if !to_files.exists() {
                            let mut file_count = 0;
                            let mut total_bytes = 0u64;
                            const MAX_OVERLAY_FILES: usize = 256;
                            const MAX_OVERLAY_BYTES: u64 = 16 * 1024 * 1024;
                            copy_dir_bounded(
                                &from_files,
                                &to_files,
                                &mut file_count,
                                &mut total_bytes,
                                MAX_OVERLAY_FILES,
                                MAX_OVERLAY_BYTES,
                            )?;
                            messages.push(format!(
                                "migrated files directory from legacy v1 key {} to {}",
                                best_key, new_key
                            ));
                        }
                    }
                }
            }
        }

        // Migrate state.json enabled flags from the SAME candidate as profile (most recent)
        if let Ok(mut registry) = self.registry() {
            if !registry.contains_key(new_key) {
                let chosen = if let Some((best_key, _)) = &best_candidate {
                    registry
                        .get(best_key)
                        .and_then(Json::as_bool)
                        .map(|v| (best_key.clone(), v))
                } else {
                    None
                };

                // Fallback to other candidates only if best_candidate had no enabled flag or no profile existed
                let chosen = chosen.or_else(|| {
                    for cand in candidates {
                        if let Some(val) = registry.get(cand).and_then(Json::as_bool) {
                            return Some((cand.clone(), val));
                        }
                    }
                    None
                });

                if let Some((best_key, val)) = chosen {
                    messages.push(format!(
                        "migrated enabled flag ({val}) from legacy v1 key {} to {}",
                        best_key, new_key
                    ));
                    registry.insert(new_key.to_string(), Json::Bool(val));
                    let target = self
                        .root
                        .as_ref()
                        .ok_or("addon configuration directory unavailable")?
                        .join("state.json");
                    write_json(&target, &json!({"schemaVersion":1,"enabled":registry})).map_err(
                        |e| format!("failed to update state.json during migration: {e}"),
                    )?;
                }
            }
        }

        Ok(messages)
    }

    pub fn data_dir(&self, addon: &DiscoveredAddon) -> Result<Option<PathBuf>, String> {
        let key = Self::key(addon)?;
        Ok(self
            .root
            .as_ref()
            .map(|root| root.join(format!("{key}-files"))))
    }

    pub fn enabled(&self, key: &str) -> Result<Option<bool>, String> {
        validate_key(key)?;
        Ok(self.registry()?.get(key).and_then(Json::as_bool))
    }

    pub fn set_enabled(&self, key: &str, enabled: bool) -> Result<(), String> {
        validate_key(key)?;
        let mut registry = self.registry()?;
        registry.insert(key.to_owned(), Json::Bool(enabled));
        let target = self
            .root
            .as_ref()
            .ok_or("addon configuration directory unavailable")?
            .join("state.json");
        write_json(&target, &json!({"schemaVersion":1,"enabled":registry}))
    }

    pub fn save_profile(&self, key: &str, value: Value) -> Result<(), String> {
        let data = super::profile::encode(value).map_err(|e| e.to_string())?;
        self.save_encoded(key, &data)
    }
    pub(crate) fn save_encoded(&self, key: &str, value: &Json) -> Result<(), String> {
        let path = self.profile_path(key)?;
        super::sandbox::check_settings_quota(&self.files_dir(key)?, &path, value)?;
        write_json(&path, value)
    }

    pub fn load_profile(&self, key: &str, lua: &Lua) -> Result<Option<Value>, String> {
        read_json(&self.profile_path(key)?)?
            .map(|json| super::profile::decode(lua, &json).map_err(|e| e.to_string()))
            .transpose()
    }

    pub fn profile_path(&self, key: &str) -> Result<PathBuf, String> {
        validate_key(key)?;
        Ok(self
            .root
            .as_ref()
            .ok_or("addon configuration directory unavailable")?
            .join(format!("{key}.json")))
    }

    pub fn files_dir(&self, key: &str) -> Result<PathBuf, String> {
        validate_key(key)?;
        Ok(self
            .root
            .as_ref()
            .ok_or("addon configuration directory unavailable")?
            .join(format!("{key}-files")))
    }

    fn registry(&self) -> Result<serde_json::Map<String, Json>, String> {
        let path = self
            .root
            .as_ref()
            .ok_or("addon configuration directory unavailable")?
            .join("state.json");
        let Some(value) = read_json(&path)? else {
            return Ok(Default::default());
        };
        let object = value
            .as_object()
            .filter(|o| o.len() == 2)
            .ok_or("invalid addon state registry")?;
        if object.get("schemaVersion").and_then(Json::as_u64) != Some(1) {
            return Err("unsupported addon state schemaVersion".into());
        }
        let enabled = object
            .get("enabled")
            .and_then(Json::as_object)
            .ok_or("invalid addon enabled registry")?;
        for (key, value) in enabled {
            validate_key(key)?;
            if !value.is_boolean() {
                return Err("addon enabled state must be boolean".into());
            }
        }
        Ok(enabled.clone())
    }
}

fn copy_dir_bounded(
    from: &Path,
    to: &Path,
    file_count: &mut usize,
    total_bytes: &mut u64,
    max_files: usize,
    max_bytes: u64,
) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|e| format!("failed to create files directory: {e}"))?;
    let entries = fs::read_dir(from).map_err(|e| format!("failed to read files directory: {e}"))?;

    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let target = to.join(entry.file_name());

        if path.is_dir() {
            copy_dir_bounded(
                &path,
                &target,
                file_count,
                total_bytes,
                max_files,
                max_bytes,
            )?;
        } else if path.is_file() {
            *file_count += 1;
            if *file_count > max_files {
                return Err("addon files directory file count limit exceeded".into());
            }
            let meta = fs::metadata(&path).map_err(|e| e.to_string())?;
            *total_bytes += meta.len();
            if *total_bytes > max_bytes {
                return Err("addon files directory size limit exceeded".into());
            }
            fs::copy(&path, &target).map_err(|e| format!("failed to copy overlay file: {e}"))?;
        }
    }
    Ok(())
}

fn relative_to_root(path: &Path, root: &Path) -> PathBuf {
    if let Ok(rel) = path.strip_prefix(root) {
        return rel.to_path_buf();
    }
    if let (Ok(c_path), Ok(c_root)) = (path.canonicalize(), root.canonicalize()) {
        if let Ok(rel) = c_path.strip_prefix(&c_root) {
            return rel.to_path_buf();
        }
    }
    path.file_name()
        .map(PathBuf::from)
        .unwrap_or_else(|| path.to_path_buf())
}

fn normalize_components(path: &Path) -> Vec<u8> {
    let mut normalized = String::new();
    for comp in path.components() {
        match comp {
            std::path::Component::Normal(c) => {
                if !normalized.is_empty() {
                    normalized.push('/');
                }
                normalized.push_str(&c.to_string_lossy());
            }
            _ => {}
        }
    }
    normalized.into_bytes()
}

fn identity_relative(
    entry: &Path,
    package: &Path,
    mods_root: &Path,
    id: &str,
) -> Result<String, String> {
    let rel_entry = relative_to_root(entry, mods_root);
    let rel_package = relative_to_root(package, mods_root);
    let entry_bytes = normalize_components(&rel_entry);
    let package_bytes = normalize_components(&rel_package);
    Ok(digest_fields_v2(
        &entry_bytes,
        &package_bytes,
        id.as_bytes(),
    ))
}

fn digest_fields_v2(entry: &[u8], package: &[u8], id: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(b"eqoa-native-addons/storage/v2\0");
    for bytes in [entry, package, id] {
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    format!("{:x}", hash.finalize())
}

fn identity_v1(entry: &Path, package: &Path, id: &str) -> Result<String, String> {
    let entry = entry
        .canonicalize()
        .map_err(|e| format!("entry identity: {e}"))?;
    let package = package
        .canonicalize()
        .map_err(|e| format!("package identity: {e}"))?;
    Ok(digest_fields_v1(
        entry.as_os_str().as_encoded_bytes(),
        package.as_os_str().as_encoded_bytes(),
        id.as_bytes(),
    ))
}

fn digest_fields_v1(entry: &[u8], package: &[u8], id: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(b"eqoa-native-addons/storage/v1\0");
    for bytes in [entry, package, id] {
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    format!("{:x}", hash.finalize())
}

fn validate_key(key: &str) -> Result<(), String> {
    if key.len() == 64
        && key
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        Ok(())
    } else {
        Err("invalid addon storage digest".into())
    }
}

fn read_json(path: &Path) -> Result<Option<Json>, String> {
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    let mut bytes = Vec::new();
    file.take(MAX_JSON_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_JSON_BYTES {
        return Err("addon JSON exceeds 1 MiB".into());
    }
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|e| format!("invalid addon JSON: {e}"))
}

fn write_json(path: &Path, value: &Json) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_JSON_BYTES {
        return Err("addon JSON exceeds 1 MiB".into());
    }
    atomic_replace(path, &bytes).map_err(|e| e.to_string())
}

pub(crate) fn atomic_replace(target: &Path, bytes: &[u8]) -> std::io::Result<()> {
    atomic_replace_with(target, bytes, |from, to| fs::rename(from, to))
}

// Private fault seam: production passes std::fs::rename. No public IO framework.
fn atomic_replace_with(
    target: &Path,
    bytes: &[u8],
    rename: impl FnOnce(&Path, &Path) -> std::io::Result<()>,
) -> std::io::Result<()> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let parent = target
        .parent()
        .ok_or_else(|| std::io::Error::other("storage target has no parent"))?;
    fs::create_dir_all(parent)?;
    let (temporary, mut file) = loop {
        let temporary = parent.join(format!(
            ".addon-{}-{}.tmp",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => break (temporary, file),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    };
    let written = file
        .write_all(bytes)
        .and_then(|_| file.flush())
        .and_then(|_| file.sync_all());
    drop(file);
    let result = written.and_then(|_| rename(&temporary, target));
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "eqoa-forge-storage-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn addon(&self) -> DiscoveredAddon {
            let entry = self.0.join("entry.lua");
            fs::write(&entry, "").unwrap();
            DiscoveredAddon {
                id: "same.opaque.id".to_string(),
                name: "same.opaque.id".to_string(),
                entry: entry.clone(),
                script_root: self.0.clone(),
                package_root: self.0.clone(),
                module_dirs: vec![],
                resource_dirs: vec![],
                api: None,
                entry_mode: Default::default(),
                requested_capabilities: vec![],
                initially_enabled: true,
                mods_root: Some(self.0.clone()),
            }
        }
        fn key(&self) -> String {
            Storage::key(&self.addon()).unwrap()
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            assert_eq!(self.0.parent(), Some(std::env::temp_dir().as_path()));
            assert!(
                self.0
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("eqoa-forge-storage-")
            );
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn forge_storage_v1_digest_pinned_to_reference_hash() {
        let entry = b"main.lua";
        let package = b"wereoxxs_ui";
        let id = b"wereoxxs_ui";
        let hash = digest_fields_v1(entry, package, id);
        assert_eq!(
            hash, "d1a2fc6aa4f4754069e2275aff1409f3d8360eb21c862c424a68fda90777bfd0",
            "v1 storage digest algorithm must remain strictly pinned to protect player migration"
        );
    }

    #[test]
    fn forge_storage_same_package_in_different_absolute_folders_gets_same_key() {
        let scratch1 = Scratch::new();
        let scratch2 = Scratch::new();

        let addon1 = DiscoveredAddon {
            id: "wereoxxs_ui".to_string(),
            name: "Wereoxxs UI".to_string(),
            entry: scratch1.0.join("wereoxxs_ui/wereoxxs_ui.lua"),
            script_root: scratch1.0.join("wereoxxs_ui"),
            package_root: scratch1.0.join("wereoxxs_ui"),
            module_dirs: vec![],
            resource_dirs: vec![],
            api: None,
            entry_mode: Default::default(),
            requested_capabilities: vec![],
            initially_enabled: true,
            mods_root: Some(scratch1.0.clone()),
        };

        let addon2 = DiscoveredAddon {
            id: "wereoxxs_ui".to_string(),
            name: "Wereoxxs UI".to_string(),
            entry: scratch2.0.join("wereoxxs_ui/wereoxxs_ui.lua"),
            script_root: scratch2.0.join("wereoxxs_ui"),
            package_root: scratch2.0.join("wereoxxs_ui"),
            module_dirs: vec![],
            resource_dirs: vec![],
            api: None,
            entry_mode: Default::default(),
            requested_capabilities: vec![],
            initially_enabled: true,
            mods_root: Some(scratch2.0.clone()),
        };

        let key1 = Storage::key(&addon1).unwrap();
        let key2 = Storage::key(&addon2).unwrap();
        assert_eq!(
            key1, key2,
            "same addon package in two different absolute folders must produce identical storage key"
        );
    }

    #[test]
    fn forge_storage_old_v1_data_is_migrated_on_first_load_and_old_file_left_in_place() {
        let scratch = Scratch::new();
        let store = Storage::new(Some(scratch.0.clone()));
        let lua = Lua::new();

        let addon = scratch.addon();
        let v1_key = Storage::v1_key(&addon).unwrap();
        let new_key = Storage::key(&addon).unwrap();
        assert_ne!(v1_key, new_key, "v1 and v2 keys must differ");

        // Save under legacy v1 key
        let old_val = lua
            .load("return {legacy_setting=42, enabled=true}")
            .eval()
            .unwrap();
        store.save_profile(&v1_key, old_val).unwrap();
        store.set_enabled(&v1_key, false).unwrap();

        let v1_path = store.profile_path(&v1_key).unwrap();
        let new_path = store.profile_path(&new_key).unwrap();
        assert!(v1_path.is_file(), "v1 profile file must exist");
        assert!(
            !new_path.is_file(),
            "new profile file must not exist before migration"
        );

        // Migrate on first load
        let messages = store.migrate_addon(&addon).unwrap();
        assert!(
            !messages.is_empty(),
            "migration must report status messages"
        );

        // 1. New key now has the data
        assert!(
            new_path.is_file(),
            "new profile file must exist after migration"
        );
        let loaded = store.load_profile(&new_key, &lua).unwrap().unwrap();
        assert_eq!(
            loaded
                .as_table()
                .unwrap()
                .get::<i32>("legacy_setting")
                .unwrap(),
            42,
            "migrated profile data must match old v1 profile"
        );

        // 2. Old file is left in place
        assert!(
            v1_path.is_file(),
            "old v1 file must be left in place (not deleted)"
        );

        // 3. state.json enabled flags migrated
        assert_eq!(
            store.enabled(&new_key).unwrap(),
            Some(false),
            "state.json enabled flag must be migrated to new key"
        );
        assert_eq!(
            store.enabled(&v1_key).unwrap(),
            Some(false),
            "state.json old v1 enabled flag must remain intact"
        );
    }

    #[test]
    fn forge_storage_picks_most_recently_modified_v1_file_when_several_match() {
        let scratch = Scratch::new();
        let store = Storage::new(Some(scratch.0.clone()));
        let lua = Lua::new();
        let addon = scratch.addon();
        let new_key = Storage::key(&addon).unwrap();

        // Simulate two legacy v1 keys for this addon
        let old_v1_a = "1111111111111111111111111111111111111111111111111111111111111111";
        let old_v1_b = "2222222222222222222222222222222222222222222222222222222222222222";

        // Save older profile to old_v1_a
        let val_a = lua.load("return {version=1}").eval().unwrap();
        store.save_profile(old_v1_a, val_a).unwrap();
        store.set_enabled(old_v1_a, false).unwrap();
        let path_a = store.profile_path(old_v1_a).unwrap();

        // Ensure distinct mtime
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Save newer profile to old_v1_b
        let val_b = lua.load("return {version=2}").eval().unwrap();
        store.save_profile(old_v1_b, val_b).unwrap();
        store.set_enabled(old_v1_b, true).unwrap();
        let path_b = store.profile_path(old_v1_b).unwrap();

        let msgs = store
            .migrate_addon_with_candidates(&addon, &[old_v1_a.to_string(), old_v1_b.to_string()])
            .unwrap();

        // Must report picking the most recently modified key
        assert!(
            msgs.iter()
                .any(|m| m.contains("multiple legacy v1 profiles found")),
            "must log message on multiple matching profiles"
        );

        // Must have picked the most recently modified profile (val_b, version 2)
        let loaded = store.load_profile(&new_key, &lua).unwrap().unwrap();
        assert_eq!(
            loaded.as_table().unwrap().get::<i32>("version").unwrap(),
            2,
            "must migrate data from the most recently modified v1 file"
        );

        // Enabled flag must be migrated from the SAME candidate (val_b -> true, not val_a -> false)
        assert_eq!(
            store.enabled(&new_key).unwrap(),
            Some(true),
            "enabled flag must be migrated from the same most recent candidate as the profile"
        );

        assert!(path_a.is_file(), "older v1 file must be preserved");
        assert!(path_b.is_file(), "newer v1 file must be preserved");
    }

    #[test]
    fn forge_storage_sibling_search_runs_at_most_once_per_rescan_and_never_when_not_needed() {
        let scratch = Scratch::new();
        let store = Storage::new(Some(scratch.0.clone()));
        let mut scanner = SiblingScanner::new();

        let addon1 = scratch.addon();
        let new_key1 = Storage::key(&addon1).unwrap();

        // 1. Scenario A: Addon already has a v2 profile -> sibling scan must NOT run
        store.save_profile(&new_key1, mlua::Value::Nil).unwrap();
        store
            .migrate_addon_with_scanner(&addon1, &mut scanner)
            .unwrap();
        assert_eq!(
            scanner.listing_count, 0,
            "sibling listing must NOT run when v2 profile already exists"
        );

        // 2. Scenario B: Addon has no v2 profile, but has current location v1 profile -> sibling scan must NOT run
        let scratch2 = Scratch::new();
        let store2 = Storage::new(Some(scratch2.0.clone()));
        let mut scanner2 = SiblingScanner::new();
        let addon2 = scratch2.addon();
        let v1_key2 = Storage::v1_key(&addon2).unwrap();
        store2.save_profile(&v1_key2, mlua::Value::Nil).unwrap();
        store2
            .migrate_addon_with_scanner(&addon2, &mut scanner2)
            .unwrap();
        assert_eq!(
            scanner2.listing_count, 0,
            "sibling listing must NOT run when current location v1 profile exists"
        );

        // 3. Scenario C: Two addons both need sibling migration during the same rescan -> parent dir listed at most ONCE
        let root = scratch.0.join("parent_installs");
        let current_game = root.join("current_game");
        let mods_dir = current_game.join("mods");
        let sibling_game = root.join("sibling_game");
        let sibling_mods = sibling_game.join("mods");
        fs::create_dir_all(&mods_dir).unwrap();
        fs::create_dir_all(&sibling_mods).unwrap();

        let entry_a = mods_dir.join("addon_a/main.lua");
        let entry_b = mods_dir.join("addon_b/main.lua");
        fs::create_dir_all(entry_a.parent().unwrap()).unwrap();
        fs::create_dir_all(entry_b.parent().unwrap()).unwrap();
        fs::write(&entry_a, "").unwrap();
        fs::write(&entry_b, "").unwrap();

        let a = DiscoveredAddon {
            id: "addon_a".to_string(),
            name: "addon_a".to_string(),
            entry: entry_a,
            script_root: mods_dir.join("addon_a"),
            package_root: mods_dir.join("addon_a"),
            module_dirs: vec![],
            resource_dirs: vec![],
            api: None,
            entry_mode: Default::default(),
            requested_capabilities: vec![],
            initially_enabled: true,
            mods_root: Some(mods_dir.clone()),
        };

        let b = DiscoveredAddon {
            id: "addon_b".to_string(),
            name: "addon_b".to_string(),
            entry: entry_b,
            script_root: mods_dir.join("addon_b"),
            package_root: mods_dir.join("addon_b"),
            module_dirs: vec![],
            resource_dirs: vec![],
            api: None,
            entry_mode: Default::default(),
            requested_capabilities: vec![],
            initially_enabled: true,
            mods_root: Some(mods_dir.clone()),
        };

        let mut rescan_scanner = SiblingScanner::new();
        store
            .migrate_addon_with_scanner(&a, &mut rescan_scanner)
            .unwrap();
        store
            .migrate_addon_with_scanner(&b, &mut rescan_scanner)
            .unwrap();

        assert_eq!(
            rescan_scanner.listing_count, 1,
            "sibling directory listing must run at most once per rescan across all addons"
        );
    }

    #[test]
    fn forge_storage_sibling_search_never_picks_another_addon_data() {
        let scratch = Scratch::new();
        let store = Storage::new(Some(scratch.0.clone()));
        let lua = Lua::new();

        let root = scratch.0.join("sibling_test_installs");
        let sib_a = root.join("game_a");
        let sib_b = root.join("game_b");
        let current = root.join("game_current");
        fs::create_dir_all(sib_a.join("mods/alpha")).unwrap();
        fs::create_dir_all(sib_b.join("mods/beta")).unwrap();
        fs::create_dir_all(current.join("mods/alpha")).unwrap();
        fs::create_dir_all(current.join("mods/beta")).unwrap();

        fs::write(sib_a.join("mods/alpha/main.lua"), "").unwrap();
        fs::write(sib_b.join("mods/beta/main.lua"), "").unwrap();
        fs::write(current.join("mods/alpha/main.lua"), "").unwrap();
        fs::write(current.join("mods/beta/main.lua"), "").unwrap();

        let alpha_old = DiscoveredAddon {
            id: "alpha".to_string(),
            name: "alpha".to_string(),
            entry: sib_a.join("mods/alpha/main.lua"),
            script_root: sib_a.join("mods/alpha"),
            package_root: sib_a.join("mods/alpha"),
            module_dirs: vec![],
            resource_dirs: vec![],
            api: None,
            entry_mode: Default::default(),
            requested_capabilities: vec![],
            initially_enabled: true,
            mods_root: Some(sib_a.join("mods")),
        };

        let beta_old = DiscoveredAddon {
            id: "beta".to_string(),
            name: "beta".to_string(),
            entry: sib_b.join("mods/beta/main.lua"),
            script_root: sib_b.join("mods/beta"),
            package_root: sib_b.join("mods/beta"),
            module_dirs: vec![],
            resource_dirs: vec![],
            api: None,
            entry_mode: Default::default(),
            requested_capabilities: vec![],
            initially_enabled: true,
            mods_root: Some(sib_b.join("mods")),
        };

        // Save distinct profiles under their v1 keys
        let v1_key_alpha = Storage::v1_key(&alpha_old).unwrap();
        let v1_key_beta = Storage::v1_key(&beta_old).unwrap();
        store
            .save_profile(
                &v1_key_alpha,
                lua.load("return {addon_name=\"alpha_data\"}")
                    .eval()
                    .unwrap(),
            )
            .unwrap();
        store
            .save_profile(
                &v1_key_beta,
                lua.load("return {addon_name=\"beta_data\"}")
                    .eval()
                    .unwrap(),
            )
            .unwrap();

        // Current addons in game_current
        let alpha_cur = DiscoveredAddon {
            id: "alpha".to_string(),
            name: "alpha".to_string(),
            entry: current.join("mods/alpha/main.lua"),
            script_root: current.join("mods/alpha"),
            package_root: current.join("mods/alpha"),
            module_dirs: vec![],
            resource_dirs: vec![],
            api: None,
            entry_mode: Default::default(),
            requested_capabilities: vec![],
            initially_enabled: true,
            mods_root: Some(current.join("mods")),
        };

        let beta_cur = DiscoveredAddon {
            id: "beta".to_string(),
            name: "beta".to_string(),
            entry: current.join("mods/beta/main.lua"),
            script_root: current.join("mods/beta"),
            package_root: current.join("mods/beta"),
            module_dirs: vec![],
            resource_dirs: vec![],
            api: None,
            entry_mode: Default::default(),
            requested_capabilities: vec![],
            initially_enabled: true,
            mods_root: Some(current.join("mods")),
        };

        let mut scanner = SiblingScanner::new();
        store
            .migrate_addon_with_scanner(&alpha_cur, &mut scanner)
            .unwrap();
        store
            .migrate_addon_with_scanner(&beta_cur, &mut scanner)
            .unwrap();

        let alpha_new_key = Storage::key(&alpha_cur).unwrap();
        let beta_new_key = Storage::key(&beta_cur).unwrap();

        let alpha_loaded = store
            .load_profile(&alpha_new_key, &lua)
            .unwrap()
            .expect("alpha profile must be migrated");
        let beta_loaded = store
            .load_profile(&beta_new_key, &lua)
            .unwrap()
            .expect("beta profile must be migrated");

        assert_eq!(
            alpha_loaded
                .as_table()
                .unwrap()
                .get::<String>("addon_name")
                .unwrap(),
            "alpha_data",
            "alpha must migrate only its own profile"
        );
        assert_eq!(
            beta_loaded
                .as_table()
                .unwrap()
                .get::<String>("addon_name")
                .unwrap(),
            "beta_data",
            "beta must migrate only its own profile"
        );
    }

    #[test]
    fn forge_storage_migrates_overlay_files_directory_bounded() {
        let scratch = Scratch::new();
        let store = Storage::new(Some(scratch.0.clone()));
        let addon = scratch.addon();

        let v1_key = Storage::v1_key(&addon).unwrap();
        let new_key = Storage::key(&addon).unwrap();

        // Create overlay files directory under v1 key
        let v1_files = store.files_dir(&v1_key).unwrap();
        fs::create_dir_all(v1_files.join("sub")).unwrap();
        fs::write(v1_files.join("file1.txt"), b"hello").unwrap();
        fs::write(v1_files.join("sub/file2.txt"), b"world").unwrap();

        // Save v1 profile so migration triggers
        store.save_profile(&v1_key, Value::Nil).unwrap();

        let to_files = store.files_dir(&new_key).unwrap();
        assert!(
            !to_files.exists(),
            "v2 files dir must not exist before migration"
        );

        store.migrate_addon(&addon).unwrap();

        assert!(to_files.is_dir(), "v2 files dir must exist after migration");
        assert_eq!(
            fs::read(to_files.join("file1.txt")).unwrap(),
            b"hello",
            "migrated file content must match"
        );
        assert_eq!(
            fs::read(to_files.join("sub/file2.txt")).unwrap(),
            b"world",
            "migrated nested file content must match"
        );

        // v1 files must still exist (copy, not delete)
        assert!(v1_files.join("file1.txt").is_file());
        assert!(v1_files.join("sub/file2.txt").is_file());
    }

    #[test]
    fn forge_storage_two_different_addons_never_collide() {
        let scratch = Scratch::new();
        let addon_a = DiscoveredAddon {
            id: "addon_alpha".to_string(),
            name: "Addon Alpha".to_string(),
            entry: scratch.0.join("addon_alpha/main.lua"),
            script_root: scratch.0.join("addon_alpha"),
            package_root: scratch.0.join("addon_alpha"),
            module_dirs: vec![],
            resource_dirs: vec![],
            api: None,
            entry_mode: Default::default(),
            requested_capabilities: vec![],
            initially_enabled: true,
            mods_root: Some(scratch.0.clone()),
        };

        let addon_b = DiscoveredAddon {
            id: "addon_beta".to_string(),
            name: "Addon Beta".to_string(),
            entry: scratch.0.join("addon_beta/main.lua"),
            script_root: scratch.0.join("addon_beta"),
            package_root: scratch.0.join("addon_beta"),
            module_dirs: vec![],
            resource_dirs: vec![],
            api: None,
            entry_mode: Default::default(),
            requested_capabilities: vec![],
            initially_enabled: true,
            mods_root: Some(scratch.0.clone()),
        };

        let key_a = Storage::key(&addon_a).unwrap();
        let key_b = Storage::key(&addon_b).unwrap();
        assert_ne!(
            key_a, key_b,
            "two different addons must never produce colliding storage keys"
        );
    }

    #[test]
    fn forge_storage_numeric_profile_repeated_replace_and_scratch_isolation() {
        let a = Scratch::new();
        let b = Scratch::new();
        let key = a.key();
        let store = Storage::new(Some(a.0.clone()));
        let other = Storage::new(Some(b.0.clone()));
        let lua = Lua::new();
        for n in [17, 29] {
            let value = lua
                .load(format!("return {{[3]={n}, colors={{1,0.5,0,1}}}}"))
                .eval()
                .unwrap();
            store.save_profile(&key, value).unwrap();
            let loaded = store.load_profile(&key, &lua).unwrap().unwrap();
            assert_eq!(loaded.as_table().unwrap().get::<i32>(3).unwrap(), n);
            assert_eq!(
                loaded
                    .as_table()
                    .unwrap()
                    .get::<mlua::Table>("colors")
                    .unwrap()
                    .get::<f32>(2)
                    .unwrap(),
                0.5
            );
        }
        assert!(other.load_profile(&key, &lua).unwrap().is_none());
        store.set_enabled(&key, false).unwrap();
        assert_eq!(
            Storage::new(Some(a.0.clone())).enabled(&key).unwrap(),
            Some(false)
        );
        assert_eq!(other.enabled(&key).unwrap(), None);
    }

    #[test]
    fn forge_storage_invalid_save_and_failed_replace_preserve_last_good_target() {
        let scratch = Scratch::new();
        let key = scratch.key();
        let store = Storage::new(Some(scratch.0.clone()));
        let lua = Lua::new();
        store
            .save_profile(&key, lua.load("return {[4]=123}").eval().unwrap())
            .unwrap();
        let target = store.profile_path(&key).unwrap();
        let before = fs::read(&target).unwrap();
        let bad = lua.load("return {bad=function() end}").eval().unwrap();
        assert!(store.save_profile(&key, bad).is_err());
        assert_eq!(fs::read(&target).unwrap(), before);
        let siblings = fs::read_dir(target.parent().unwrap()).unwrap().count();
        assert!(
            atomic_replace_with(&target, b"replacement", |_, _| {
                Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "controlled rename failure",
                ))
            })
            .is_err()
        );
        assert_eq!(fs::read(&target).unwrap(), before);
        assert_eq!(
            fs::read_dir(target.parent().unwrap()).unwrap().count(),
            siblings
        );
    }

    #[test]
    fn forge_storage_windows_rename_replaces_existing_without_destination_delete() {
        let scratch = Scratch::new();
        let target = scratch.0.join("replace.json");
        atomic_replace(&target, b"first").unwrap();
        atomic_replace(&target, b"second").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"second");
        assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 1);
    }

    #[test]
    fn forge_storage_rejects_corrupt_oversized_schema_and_unavailable_config() {
        let scratch = Scratch::new();
        let key = scratch.key();
        let store = Storage::new(Some(scratch.0.clone()));
        let lua = Lua::new();
        store.save_profile(&key, Value::Nil).unwrap();
        let target = store.profile_path(&key).unwrap();
        for bytes in [b"{broken".to_vec(), vec![b' '; MAX_JSON_BYTES + 1]] {
            fs::write(&target, bytes).unwrap();
            assert!(store.load_profile(&key, &lua).is_err());
        }
        fs::write(
            store.root.as_ref().unwrap().join("state.json"),
            r#"{"schemaVersion":999,"enabled":{}}"#,
        )
        .unwrap();
        assert!(store.enabled(&key).is_err());
        assert!(Storage::new(None).save_profile(&key, Value::Nil).is_err());
        assert!(Storage::new(None).set_enabled(&key, true).is_err());
    }

    #[test]
    fn forge_storage_identity_is_safe_path_sensitive_and_length_delimited() {
        let scratch = Scratch::new();
        let a = scratch.0.join("a");
        let ab = scratch.0.join("ab");
        fs::write(&a, "").unwrap();
        fs::write(&ab, "").unwrap();
        let one = identity_v1(&a, &scratch.0, "bc").unwrap();
        let two = identity_v1(&ab, &scratch.0, "c").unwrap();
        assert_ne!(one, two);
        assert_ne!(
            digest_fields_v1(b"a", b"bc", b"d"),
            digest_fields_v1(b"ab", b"c", b"d")
        );
        assert_eq!(one.len(), 64);
        assert!(one.bytes().all(|b| b.is_ascii_hexdigit()));
        assert!(
            Storage::new(Some(scratch.0.clone()))
                .profile_path("../../unsafe")
                .is_err()
        );
    }
}
