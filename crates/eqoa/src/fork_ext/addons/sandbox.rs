//! Shared virtual mounts for legacy and native file/media entry points.
use super::discovery::DiscoveredAddon;
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Clone)]
pub(crate) struct Mounts {
    pub addon: DiscoveredAddon,
    pub data: Option<PathBuf>,
}
impl Mounts {
    pub fn key(&self, path: &str) -> Result<PathBuf, String> {
        super::file_io::relative(&self.addon, path)
    }
    pub fn read_path(&self, key: &Path) -> Result<PathBuf, String> {
        if let Some(root) = &self.data {
            let tombstone = root.join("removed").join(key);
            let _guard = lock_parents(&tombstone, false)?;
            super::file_io::no_links(&tombstone)?;
            if tombstone.exists() {
                return Err("file not found (private tombstone)".into());
            }
            let overlay = root.join("files").join(key);
            super::file_io::no_links(&overlay)?;
            if overlay.exists() {
                return Ok(overlay);
            }
        }
        if key.starts_with("data") {
            return Err("file not found".into());
        }
        Ok(self.addon.script_root.join(key))
    }
    pub fn media(&self, path: &str) -> Result<PathBuf, String> {
        let normalized = path.replace('\\', "/");
        if normalized.starts_with("resources/")
            || normalized.starts_with("package_resources/")
            || Path::new(path).is_absolute()
            || normalized.contains(':')
            || normalized.starts_with('/')
        {
            let key = self.key(path)?;
            let source = self.read_path(&key)?;
            super::file_io::no_links(&source)?;
            return source
                .is_file()
                .then_some(source)
                .ok_or("resource not found".into());
        }
        for prefix in ["package_resources/", "resources/"] {
            let key = self.key(&format!("{prefix}{normalized}"))?;
            match self.read_path(&key) {
                Ok(p) if p.is_file() => {
                    super::file_io::no_links(&p)?;
                    return Ok(p);
                }
                Ok(_) => {}
                Err(e) if e.starts_with("file not found") => {}
                Err(e) => return Err(e),
            }
        }
        // Older loose-package manifests may designate their own package root
        // as a media directory. This grants no sibling-package visibility.
        for dir in &self.addon.resource_dirs {
            if *dir == self.addon.package_root {
                let candidate = dir.join(&normalized);
                super::file_io::no_links(&candidate)?;
                if candidate.is_file() {
                    return Ok(candidate);
                }
            }
        }
        Err("resource not found".into())
    }
    pub fn read(&self, path: &str) -> Result<Vec<u8>, String> {
        let key = self.key(path)?;
        read_bounded(&self.read_path(&key)?, 1024 * 1024)
    }
    pub fn write(&self, path: &str, bytes: &[u8]) -> Result<(), String> {
        if bytes.len() > 1024 * 1024 {
            return Err("file exceeds 1 MiB".into());
        }
        let key = self.key(path)?;
        let root = self
            .data
            .as_ref()
            .ok_or("addon data directory unavailable")?;
        let target = root.join("files").join(&key);
        let deleted = root.join("removed").join(&key);
        let _guard = lock_parents(&target, true)?;
        let _deleted_guard = lock_parents(&deleted, true)?;
        super::file_io::no_links(&deleted)?;
        check_quota(root, &target, bytes.len())?;
        super::storage::atomic_replace(&target, bytes).map_err(|e| e.to_string())?;
        if deleted.exists() {
            fs::remove_file(&deleted).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    pub fn remove(&self, path: &str) -> Result<bool, String> {
        let key = self.key(path)?;
        let source = match self.read_path(&key) {
            Ok(p) => p,
            Err(e) if e.starts_with("file not found") => return Ok(false),
            Err(e) => return Err(e),
        };
        super::file_io::no_links(&source)?;
        if !source.is_file() {
            return Ok(false);
        }
        let root = self
            .data
            .as_ref()
            .ok_or("addon data directory unavailable")?;
        let target = root.join("files").join(&key);
        let deleted = root.join("removed").join(&key);
        let _guard = lock_parents(&target, false)?;
        let _deleted_guard = lock_parents(&deleted, true)?;
        super::file_io::no_links(&target)?;
        super::file_io::no_links(&deleted)?;
        // Commit the tombstone before removing the overlay: a failure never reveals backing data.
        if !key.starts_with("data") {
            check_quota(root, &deleted, 0)?;
            super::storage::atomic_replace(&deleted, &[]).map_err(|e| e.to_string())?;
        }
        if target.exists() {
            fs::remove_file(&target).map_err(|e| e.to_string())?;
        }
        Ok(true)
    }
}

pub(crate) struct PathGuard {
    _parents: Vec<fs::File>,
}
pub(crate) fn lock_parents(path: &Path, create: bool) -> Result<PathGuard, String> {
    let parent = path.parent().ok_or("file has no parent")?;
    let mut dirs = parent.ancestors().collect::<Vec<_>>();
    dirs.reverse();
    let mut handles = Vec::new();
    for dir in dirs {
        if !dir.exists() {
            super::file_io::no_links(dir)?;
            if !create {
                break;
            }
            match fs::create_dir(dir) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e.to_string()),
            }
        }
        super::file_io::no_links(dir)?;
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            let file = fs::OpenOptions::new()
                .read(true)
                .share_mode(3)
                .custom_flags(0x02200000)
                .open(dir)
                .map_err(|e| e.to_string())?;
            use std::os::windows::fs::MetadataExt;
            if file
                .metadata()
                .map_err(|e| e.to_string())?
                .file_attributes()
                & 0x400
                != 0
            {
                return Err("reparse point in addon path".into());
            }
            handles.push(file);
        }
        #[cfg(not(windows))]
        {
            let _ = (&mut handles, dir);
            return Err(
                "unsupported: secure rooted file operations are not implemented on this platform"
                    .into(),
            );
        }
    }
    Ok(PathGuard { _parents: handles })
}
pub(crate) fn lock_resource(path: &Path) -> Result<PathGuard, String> {
    let mut guard = lock_parents(path, false)?;
    super::file_io::no_links(path)?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
        let file = fs::OpenOptions::new()
            .read(true)
            .access_mode(0)
            .share_mode(3)
            .custom_flags(0x00200000)
            .open(path)
            .map_err(|e| e.to_string())?;
        if file
            .metadata()
            .map_err(|e| e.to_string())?
            .file_attributes()
            & 0x400
            != 0
        {
            return Err("reparse point in addon resource".into());
        }
        guard._parents.push(file);
    }
    Ok(guard)
}
pub(crate) fn read_bounded(path: &Path, max: usize) -> Result<Vec<u8>, String> {
    let _guard = lock_resource(path)?;
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(3).custom_flags(0x00200000);
    }
    let file = options.open(path).map_err(|e| e.to_string())?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if file
            .metadata()
            .map_err(|e| e.to_string())?
            .file_attributes()
            & 0x400
            != 0
        {
            return Err("reparse point in addon file".into());
        }
    }
    let mut bytes = Vec::new();
    file.take(max as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > max {
        return Err("file exceeds byte limit".into());
    }
    Ok(bytes)
}
pub(crate) fn check_quota(root: &Path, replaced: &Path, new_len: usize) -> Result<(), String> {
    check_quota_inner(root, replaced, new_len, None)
}
pub(crate) fn check_settings_quota(
    root: &Path,
    slot: &Path,
    value: &serde_json::Value,
) -> Result<(), String> {
    let size = serde_json::to_vec(value).map_err(|e| e.to_string())?.len();
    if size > 1024 * 1024 {
        return Err("addon settings exceeds 1 MiB quota".into());
    }
    check_quota_inner(root, &root.join(".settings-reservation"), size, Some(slot))
}
fn settings_usage(root: &Path, excluded: Option<&Path>) -> Result<u64, String> {
    let Some(key) = root
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.strip_suffix("-files"))
    else {
        return Ok(0);
    };
    let Some(parent) = root.parent() else {
        return Ok(0);
    };
    fn read(path: &Path, max: u64) -> Result<serde_json::Value, String> {
        super::file_io::no_links(path)?;
        if fs::metadata(path).map_err(|e| e.to_string())?.len() > max {
            return Err("settings file exceeds quota".into());
        }
        #[cfg(windows)]
        let bytes = read_bounded(path, max as usize)?;
        #[cfg(not(windows))]
        let bytes = fs::read(path).map_err(|e| e.to_string())?;
        serde_json::from_slice(&bytes).map_err(|e| e.to_string())
    }
    let default = parent.join(format!("{key}.json"));
    let mut size = 0;
    if excluded != Some(default.as_path()) && default.is_file() {
        size = serde_json::to_vec(&read(&default, 1024 * 1024)?)
            .map_err(|e| e.to_string())?
            .len() as u64;
    }
    let profiles = parent.join("profiles");
    if profiles.exists() {
        super::file_io::no_links(&profiles)?;
        let mut count = 0;
        for file in fs::read_dir(&profiles).map_err(|e| e.to_string())? {
            let path = file.map_err(|e| e.to_string())?.path();
            if excluded == Some(path.as_path()) || path.extension().is_none_or(|e| e != "json") {
                continue;
            }
            count += 1;
            if count > 256 {
                return Err("settings profile entry quota exceeded".into());
            }
            let document = read(&path, 16 * 1024 * 1024)?;
            if let Some(value) = document.get("addons").and_then(|a| a.get(key)) {
                size = size.saturating_add(
                    serde_json::to_vec(value).map_err(|e| e.to_string())?.len() as u64,
                );
            }
            if size > 16 * 1024 * 1024 {
                return Err("addon settings exceeds 16 MiB quota".into());
            }
        }
    }
    Ok(size)
}
fn check_quota_inner(
    root: &Path,
    replaced: &Path,
    new_len: usize,
    settings_slot: Option<&Path>,
) -> Result<(), String> {
    let mut pending = vec![root.to_path_buf()];
    let mut size = (new_len as u64).saturating_add(settings_usage(root, settings_slot)?);
    if size > 16 * 1024 * 1024 {
        return Err("addon storage exceeds 16 MiB quota".into());
    }
    let mut count = 0;
    while let Some(dir) = pending.pop() {
        if !dir.exists() {
            continue;
        }
        super::file_io::no_links(&dir)?;
        for e in fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let e = e.map_err(|e| e.to_string())?;
            let path = e.path();
            super::file_io::no_links(&path)?;
            count += 1;
            if count > 4096 {
                return Err("addon storage entry quota exceeded".into());
            }
            let metadata = e.metadata().map_err(|e| e.to_string())?;
            if metadata.is_dir() {
                pending.push(path);
            } else if path != replaced {
                size = size.saturating_add(metadata.len().max(1));
            }
            if size > 16 * 1024 * 1024 {
                return Err("addon storage exceeds 16 MiB".into());
            }
        }
    }
    Ok(())
}
