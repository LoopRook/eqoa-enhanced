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
use serde_json::{json, Value as Json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const MAX_JSON_BYTES: usize = 1024 * 1024;

pub(super) struct Storage {
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

    /// Computes the legacy v1 storage key (hashing absolute paths). Retained for migration.
    pub fn v1_key(addon: &DiscoveredAddon) -> Result<String, String> {
        identity_v1(&addon.entry, &addon.package_root, &addon.id)
    }

    /// Discovers candidate legacy v1 keys for migration.
    pub fn candidate_v1_keys(addon: &DiscoveredAddon) -> Vec<String> {
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

        if let Some(game_root) = mods_root.parent() {
            if let Some(parent) = game_root.parent() {
                if let Ok(entries) = fs::read_dir(parent) {
                    for dir_entry in entries.flatten() {
                        let sibling_game_root = dir_entry.path();
                        if sibling_game_root != game_root && sibling_game_root.is_dir() {
                            let sibling_mods = sibling_game_root.join("mods");
                            let sibling_pkg = sibling_mods.join(&rel_pkg);
                            let sibling_entry = sibling_mods.join(&rel_entry);
                            if sibling_pkg.exists() {
                                add(&sibling_entry, &sibling_pkg);
                            }
                        }
                    }
                }
            }
        }

        keys
    }

    /// Migrates legacy v1 profile data and state.json enabled flags to the folder-independent v2 key.
    pub fn migrate_addon(&self, addon: &DiscoveredAddon) -> Result<(), String> {
        self.migrate_addon_with_candidates(addon, &[])
    }

    /// Migrates legacy v1 profile data with optional explicit candidate keys (useful in tests).
    pub fn migrate_addon_with_candidates(
        &self,
        addon: &DiscoveredAddon,
        extra_candidates: &[String],
    ) -> Result<(), String> {
        let new_key = Self::key(addon)?;
        let mut candidates = Self::candidate_v1_keys(addon);
        for extra in extra_candidates {
            if !candidates.contains(extra) {
                candidates.push(extra.clone());
            }
        }

        let new_profile = self.profile_path(&new_key)?;
        if !new_profile.is_file() {
            let mut matches: Vec<(String, PathBuf, std::time::SystemTime)> = Vec::new();
            for cand in &candidates {
                if let Ok(path) = self.profile_path(cand) {
                    if path.is_file() {
                        let mtime = fs::metadata(&path)
                            .and_then(|m| m.modified())
                            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                        matches.push((cand.clone(), path, mtime));
                    }
                }
            }

            if !matches.is_empty() {
                // Sort by mtime descending (most recently modified first)
                matches.sort_by(|a, b| b.2.cmp(&a.2));
                let (best_key, best_path, best_mtime) = &matches[0];
                if matches.len() > 1 {
                    println!(
                        "Multiple legacy v1 profiles found for addon '{}'; selected most recently modified key {} (modified {:?}) from candidate files",
                        addon.id,
                        best_key,
                        best_mtime
                    );
                } else {
                    println!(
                        "Migrated addon '{}' profile from legacy v1 storage key {} ({}) to folder-independent key {}",
                        addon.id,
                        best_key,
                        best_path.display(),
                        new_key
                    );
                }
                // Copy without deleting the old target
                if let Ok(bytes) = fs::read(best_path) {
                    let _ = atomic_replace(&new_profile, &bytes);
                }
            }
        }

        // Migrate state.json enabled flags
        if let Ok(mut registry) = self.registry() {
            if !registry.contains_key(&new_key) {
                let mut chosen = None;
                for cand in &candidates {
                    if let Some(val) = registry.get(cand).and_then(Json::as_bool) {
                        chosen = Some((cand.clone(), val));
                        break;
                    }
                }
                if let Some((best_key, val)) = chosen {
                    println!(
                        "Migrated addon '{}' enabled flag ({}) from legacy v1 key {} to folder-independent key {}",
                        addon.id,
                        val,
                        best_key,
                        new_key
                    );
                    registry.insert(new_key.clone(), Json::Bool(val));
                    if let Some(root) = &self.root {
                        let target = root.join("state.json");
                        let _ = write_json(&target, &json!({"schemaVersion":1,"enabled":registry}));
                    }
                }
            }
        }

        Ok(())
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
        write_json(&self.profile_path(key)?, &data)
    }

    pub fn load_profile(&self, key: &str, lua: &Lua) -> Result<Option<Value>, String> {
        read_json(&self.profile_path(key)?)?
            .map(|json| super::profile::decode(lua, &json).map_err(|e| e.to_string()))
            .transpose()
    }

    fn profile_path(&self, key: &str) -> Result<PathBuf, String> {
        validate_key(key)?;
        Ok(self
            .root
            .as_ref()
            .ok_or("addon configuration directory unavailable")?
            .join(format!("{key}.json")))
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
    Ok(digest_fields_v2(&entry_bytes, &package_bytes, id.as_bytes()))
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

pub(super) fn atomic_replace(target: &Path, bytes: &[u8]) -> std::io::Result<()> {
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
        store.migrate_addon(&addon).unwrap();

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
        let path_a = store.profile_path(old_v1_a).unwrap();

        // Ensure distinct mtime
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Save newer profile to old_v1_b
        let val_b = lua.load("return {version=2}").eval().unwrap();
        store.save_profile(old_v1_b, val_b).unwrap();
        let path_b = store.profile_path(old_v1_b).unwrap();

        store
            .migrate_addon_with_candidates(
                &addon,
                &[old_v1_a.to_string(), old_v1_b.to_string()],
            )
            .unwrap();

        // Must have picked the most recently modified (val_b, version 2)
        let loaded = store.load_profile(&new_key, &lua).unwrap().unwrap();
        assert_eq!(
            loaded.as_table().unwrap().get::<i32>("version").unwrap(),
            2,
            "must migrate data from the most recently modified v1 file"
        );
        assert!(path_a.is_file(), "older v1 file must be preserved");
        assert!(path_b.is_file(), "newer v1 file must be preserved");
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
