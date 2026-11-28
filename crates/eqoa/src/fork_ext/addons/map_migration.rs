//! One-time native-to-mod journal copy. The legacy file is always read-only.
use super::storage::Storage;
use std::{
    fs,
    io::Write,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};
pub(super) fn migrate(config: &Path) -> Result<(), String> {
    #[cfg(unix)]
    super::rooted::register_root(config);
    let root = config.join("addons");
    let marker = root.join("journal-migration-v1");
    super::file_io::no_links(&marker)?;
    if marker.exists() {
        return Ok(());
    }
    let _marker_guard = super::sandbox::lock_parents(&marker, true)?;
    let key = Storage::key_for_addon_id("eqoa_creature_journal");
    let private = root.join(format!("{key}-files"));
    let target = private.join("files/data/journal.json");
    super::file_io::no_links(&target)?;
    let source = config.join("journal.json");
    super::file_io::no_links(&source)?;
    if !target.exists() && source.is_file() {
        let bytes = super::sandbox::read_bounded(&source, super::async_json::READ_CAP)?;
        let _guard = super::sandbox::lock_parents(&target, true)?;
        super::sandbox::check_quota(&private, &target, bytes.len())?;
        // Publish a fully synced copy without replacing an existing journal.
        // A same-directory hard link is atomic and fails if another startup won.
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let temp = target.with_file_name(format!(
            ".journal-migrate-{}-{}.tmp",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        let written = file.write_all(&bytes).and_then(|()| file.sync_all());
        drop(file);
        let result = written.and_then(|()| match fs::hard_link(&temp, &target) {
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
            r => r,
        });
        let _ = fs::remove_file(&temp);
        result.map_err(|e| e.to_string())?;
    }
    super::storage::atomic_replace(&marker, b"1\n").map_err(|e| e.to_string())
}
