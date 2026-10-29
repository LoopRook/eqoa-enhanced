//! Bounded plain-data profiles and versioned enabled state for native addons.
//!
//! Contract: caller config/addons; schema-1 enabled registry; bounded plain JSON
//! profiles via profile::{encode,decode}. Identity uses SHA-256 over a versioned
//! domain and length-delimited canonical entry/package paths and opaque ID.
//! Moving the package changes its identity. Raw IDs never become filenames.
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

pub(super) struct Storage {
    root: Option<PathBuf>,
}

impl Storage {
    pub fn new(config: Option<PathBuf>) -> Self {
        Self {
            root: config.map(|root| root.join("addons")),
        }
    }

    pub fn key(addon: &DiscoveredAddon) -> Result<String, String> {
        identity(&addon.entry, &addon.package_root, &addon.id)
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

fn identity(entry: &Path, package: &Path, id: &str) -> Result<String, String> {
    let entry = entry
        .canonicalize()
        .map_err(|e| format!("entry identity: {e}"))?;
    let package = package
        .canonicalize()
        .map_err(|e| format!("package identity: {e}"))?;
    // encoded_bytes preserves non-Unicode platform paths without lossy collisions.
    Ok(digest_fields(
        entry.as_os_str().as_encoded_bytes(),
        package.as_os_str().as_encoded_bytes(),
        id.as_bytes(),
    ))
}

fn digest_fields(entry: &[u8], package: &[u8], id: &[u8]) -> String {
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
fn atomic_replace(target: &Path, bytes: &[u8]) -> std::io::Result<()> {
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
        fn key(&self) -> String {
            let entry = self.0.join("entry.lua");
            fs::write(&entry, "").unwrap();
            identity(&entry, &self.0, "same.opaque.id").unwrap()
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
        let one = identity(&a, &scratch.0, "bc").unwrap();
        let two = identity(&ab, &scratch.0, "c").unwrap();
        assert_ne!(one, two);
        assert_ne!(
            digest_fields(b"a", b"bc", b"d"),
            digest_fields(b"ab", b"c", b"d")
        );
        assert_eq!(one.len(), 64);
        assert!(one.bytes().all(|b| b.is_ascii_hexdigit()));
        let other = Scratch::new();
        assert_ne!(scratch.key(), other.key());
        assert!(
            Storage::new(Some(scratch.0.clone()))
                .profile_path("../../unsafe")
                .is_err()
        );
    }
}
