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
// Native imports are bounded read-only staging, separate from a mod's save quota.
pub(super) fn import_key(key: &Path) -> bool {
    let mut parts=key.iter();
    parts.next().is_some_and(|s|s.to_string_lossy().eq_ignore_ascii_case("data"))
        && parts.next().is_some_and(|s|s.to_string_lossy().eq_ignore_ascii_case("imports"))
}
pub(super) fn import_path(root: &Path,path: &Path) -> bool {
    path.strip_prefix(root).ok().is_some_and(|relative| {
        let mut parts=relative.iter();
        parts.next().is_some_and(|s|s.to_string_lossy().eq_ignore_ascii_case("files")) && import_key(&parts.collect::<PathBuf>())
    })
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
        if import_key(&self.key(path)?) {return Err("imports staging is read-only".into())}
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
        super::deferred_sync::atomic_replace_deferred(&target, bytes, Some(&self.addon.id)).map_err(|e| e.to_string())?;
        if deleted.exists() {
            fs::remove_file(&deleted).map_err(|e| e.to_string())?;
        }
        super::exchange::saved(self, &key);
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
            super::deferred_sync::atomic_replace_deferred(&deleted, &[], Some(&self.addon.id)).map_err(|e| e.to_string())?;
        }
        if target.exists() {
            super::deferred_sync::remove_file(&target).map_err(|e| e.to_string())?;
        }
        super::exchange::saved(self, &key);
        Ok(true)
    }
}

/// Unix counterparts of the Windows handle checks: every folder is opened without following a link, its
/// identity (device and inode) is compared with what the path shows, and the chain is checked again at the end.
/// Links owned by root (macOS' `/var`, `/tmp`) are the system's own and are followed; any other link is refused.
#[cfg(unix)]
pub(crate) mod unix {
    use std::{fs, os::unix::fs::{MetadataExt, OpenOptionsExt}, path::Path};

    pub(crate) fn identity(file: &fs::File) -> Result<(u64, u64), String> {
        let meta = file.metadata().map_err(|e| e.to_string())?;
        Ok((meta.dev(), meta.ino()))
    }
    pub(crate) fn trusted_link(meta: &fs::Metadata) -> bool {
        meta.file_type().is_symlink() && meta.uid() == 0
    }
    pub(crate) fn open_dir(dir: &Path) -> Result<fs::File, String> {
        let seen = fs::symlink_metadata(dir).map_err(|e| e.to_string())?;
        let mut options = fs::OpenOptions::new();
        options.read(true);
        // O_NOFOLLOW refuses a link as the last component; a root-owned system link is followed on purpose.
        options.custom_flags(libc::O_DIRECTORY | if trusted_link(&seen) { 0 } else { libc::O_NOFOLLOW });
        let file = options.open(dir).map_err(|e| e.to_string())?;
        let now = fs::symlink_metadata(dir).map_err(|e| e.to_string())?;
        if now.file_type().is_symlink() && !trusted_link(&now) {
            return Err("symlink in addon path".into());
        }
        still_there(dir, identity(&file)?)?;
        Ok(file)
    }
    pub(crate) fn still_there(dir: &Path, id: (u64, u64)) -> Result<(), String> {
        let meta = fs::metadata(dir).map_err(|e| e.to_string())?;
        if (meta.dev(), meta.ino()) != id {
            return Err("folder replaced in addon path".into());
        }
        Ok(())
    }
    pub(crate) fn open_file(path: &Path) -> Result<fs::File, String> {
        let file = fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)
            .map_err(|e| e.to_string())?;
        let meta = file.metadata().map_err(|e| e.to_string())?;
        if !meta.is_file() {
            return Err("addon resource is not a regular file".into());
        }
        still_there(path, (meta.dev(), meta.ino())).map_err(|_| "resource replaced in addon path".to_string())?;
        Ok(file)
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
    #[cfg(unix)]
    let mut held: Vec<(PathBuf, (u64, u64))> = Vec::new();
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
        #[cfg(unix)]
        {
            let file = unix::open_dir(dir)?;
            held.push((dir.to_path_buf(), unix::identity(&file)?));
            handles.push(file);
        }
        #[cfg(not(any(windows, unix)))]
        {
            let _ = (&mut handles, dir);
            return Err(
                "unsupported: secure rooted file operations are not implemented on this platform"
                    .into(),
            );
        }
    }
    // A folder swapped for a link while the chain was being walked no longer resolves to the folder
    // that was opened: refuse (Unix cannot block the rename the way Windows' share mode does).
    #[cfg(unix)]
    for (dir, id) in &held {
        unix::still_there(dir, *id)?;
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
    #[cfg(unix)]
    guard._parents.push(unix::open_file(path)?);
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
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        if !file.metadata().map_err(|e| e.to_string())?.is_file() {
            return Err("addon file is not a regular file".into());
        }
    }
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
    let _probe = super::stall_probe::scope("mod file quota scan");
    check_quota_inner(root, replaced, new_len, None, None)
}
pub(crate) fn check_settings_quota(
    root: &Path,
    slot: &Path,
    value: &serde_json::Value,
) -> Result<(), String> {
    check_settings_quota_known(root, slot, value, None)
}
/// The same check, with the other profiles' usage already read (`profile_usage` with `slot` left out).
pub(crate) fn check_settings_quota_known(
    root: &Path,
    slot: &Path,
    value: &serde_json::Value,
    known: Option<&ProfileUsage>,
) -> Result<(), String> {
    let size = serde_json::to_vec(value).map_err(|e| e.to_string())?.len();
    if size > 1024 * 1024 {
        return Err("addon settings exceeds 1 MiB quota".into());
    }
    check_quota_inner(root, &root.join(".settings-reservation"), size, Some(slot), known)
}
#[cfg(test)]
thread_local! {static PROFILE_READS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };}
/// Test seam: how many profile files the quota checks of this thread read since the last call.
#[cfg(test)]
pub(crate) fn take_profile_reads_for_test() -> usize {
    PROFILE_READS.with(|n| n.replace(0))
}

fn read_settings_file(path: &Path, max: u64) -> Result<serde_json::Value, String> {
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

/// What every mod's settings take in the profile files (all but the one being replaced), read once: a save of a
/// profile checks each of its mods against this instead of reading every profile again for each mod.
pub(crate) struct ProfileUsage(std::collections::HashMap<String, u64>);

pub(crate) fn profile_usage(addons: &Path, excluded: Option<&Path>) -> Result<ProfileUsage, String> {
    let mut sizes = std::collections::HashMap::new();
    let profiles = addons.join("profiles");
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
            #[cfg(test)]
            PROFILE_READS.with(|n| n.set(n.get() + 1));
            let document = read_settings_file(&path, 16 * 1024 * 1024)?;
            let Some(entries) = document.get("addons").and_then(|a| a.as_object()) else {
                continue;
            };
            for (key, value) in entries {
                let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?.len() as u64;
                let size = sizes.entry(key.clone()).or_insert(0u64);
                *size = size.saturating_add(bytes);
            }
        }
    }
    Ok(ProfileUsage(sizes))
}

fn settings_usage(root: &Path, excluded: Option<&Path>, known: Option<&ProfileUsage>) -> Result<u64, String> {
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
    let default = parent.join(format!("{key}.json"));
    let mut size = 0u64;
    if excluded != Some(default.as_path()) && default.is_file() {
        size = serde_json::to_vec(&read_settings_file(&default, 1024 * 1024)?)
            .map_err(|e| e.to_string())?
            .len() as u64;
    }
    let scanned;
    let usage = match known {
        Some(usage) => usage,
        None => {
            scanned = profile_usage(parent, excluded)?;
            &scanned
        }
    };
    size = size.saturating_add(usage.0.get(key).copied().unwrap_or(0));
    if size > 16 * 1024 * 1024 {
        return Err("addon settings exceeds 16 MiB quota".into());
    }
    Ok(size)
}
fn check_quota_inner(
    root: &Path,
    replaced: &Path,
    new_len: usize,
    settings_slot: Option<&Path>,
    known: Option<&ProfileUsage>,
) -> Result<(), String> {
    let mut pending = vec![root.to_path_buf()];
    let mut size = (new_len as u64).saturating_add(settings_usage(root, settings_slot, known)?);
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
            if import_path(root,&path) {continue}
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
