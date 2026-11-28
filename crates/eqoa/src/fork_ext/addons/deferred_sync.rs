// Code in this file is LoopRook's addition for EQOA Enhanced and is under the GNU Affero General Public License v3 or later (see LICENSE).
//! File writes of a mod (`eqoa.files.*`, option toggles) without a disk sync on the frame.
//!
//! The atomic replace in `storage.rs` writes a temporary file, `sync_all`s it, then renames it. The sync is
//! the slow part (over a second on the owner's disk) and it ran on the frame thread. Here the frame does the
//! cheap part (temporary file, bytes, rename) and the sync of that file moves to a background thread
//! (`write_queue.rs`).
//!
//! * **Read-your-writes by construction.** The new file is in place when the call returns, so every read
//!   path (`files.read`, `exists`, `list`, `open("r")`, the quota scan, `persist_toggle`'s read-back, a
//!   reload) sees it with no overlay to keep in step. A delete is the same: the tombstone and the removal
//!   are ordinary file operations done at once.
//! * **What is deferred is only the durability.** A crash of the game keeps the write (the operating
//!   system has the bytes). A power cut or system crash within the short time before the background sync
//!   finishes can lose the write or leave the file empty; the previous file is not restored.
//! * No file handle is kept open between the write and the sync: the job reopens the path, so a later
//!   write, rename or remove of the same path never meets a handle of ours (a path gone by then is fine).
//! * Syncs wait per file and collapse to the last one; `flush` waits for all of them. The manager calls it
//!   where a write must be on disk (mod disable, logout, profile switch, quit).
//! * A sync that fails is a [`Failure`] the manager shows in Mods & Addons and the log.
use super::write_queue::{Failure, WriteQueue};
use std::{
    fs,
    io::Write,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

thread_local! {
    // The frame thread's queue (mods run on it only). Dropped with the thread: the drop finishes what waits.
    static SYNCS: WriteQueue = WriteQueue::new();
}

/// `storage::atomic_replace` without the sync on the calling thread. Same temporary file and rename; the
/// file's sync is queued for the background thread. `owner` is the mod to blame if that sync fails.
pub(crate) fn atomic_replace_deferred(target: &Path, bytes: &[u8], owner: Option<&str>) -> std::io::Result<()> {
    // the part of a mod's file write that runs on the frame: said if a disk operation here waits
    let _probe = super::stall_probe::scope_of("mod file write on the frame", target.file_name().and_then(|n| n.to_str()).unwrap_or(""));
    // Linux and macOS: the temporary file is made and renamed relative to the held folder, and the background
    // sync uses the handle of that very file (nothing is opened again by its path).
    #[cfg(unix)]
    {
        let file = super::rooted::write_replace(target, bytes, false, None)?;
        let path = target.to_path_buf();
        SYNCS.with(|q| {
            q.submit(
                path.clone(),
                owner.map(str::to_owned),
                Box::new(move || sync_file(&file, &path)),
            )
        });
        return Ok(());
    }
    #[cfg(not(unix))]
    atomic_replace_by_path(target, bytes, owner)
}

#[cfg(not(unix))]
fn atomic_replace_by_path(target: &Path, bytes: &[u8], owner: Option<&str>) -> std::io::Result<()> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let parent = target
        .parent()
        .ok_or_else(|| std::io::Error::other("storage target has no parent"))?;
    fs::create_dir_all(parent)?;
    let (temporary, mut file) = loop {
        let temporary = parent.join(format!(".addon-{}-d{}.tmp", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        match fs::OpenOptions::new().write(true).create_new(true).open(&temporary) {
            Ok(file) => break (temporary, file),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    };
    // No handle is kept open across the rename or a later remove: the background job reopens the path to sync it.
    let written = file.write_all(bytes).and_then(|_| file.flush());
    drop(file);
    let result = written.and_then(|_| retry_once_after_syncs(|| fs::rename(&temporary, target)));
    if let Err(e) = result {
        let _ = fs::remove_file(&temporary);
        return Err(e);
    }
    let path = target.to_path_buf();
    SYNCS.with(|q| {
        q.submit(
            path.clone(),
            owner.map(str::to_owned),
            Box::new(move || sync_path(&path)),
        )
    });
    Ok(())
}

/// A mod's `remove` of a file: the file goes at once; a sync waiting for that path finds nothing and is done.
pub(crate) fn remove_file(path: &Path) -> std::io::Result<()> {
    retry_once_after_syncs(|| super::sandbox::remove_file(path))
}

/// On a filesystem without POSIX delete/rename semantics (FAT, exFAT, some network shares) a path the background
/// thread has open for its sync can refuse a rename or remove for that moment. Wait for the syncs and try once more.
fn retry_once_after_syncs(op: impl Fn() -> std::io::Result<()>) -> std::io::Result<()> {
    match op() {
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            flush();
            op()
        }
        other => other,
    }
}

/// The background half on Linux and macOS: sync the file that was written, by its own handle.
#[cfg(unix)]
fn sync_file(file: &fs::File, path: &Path) -> Result<(), String> {
    #[cfg(test)]
    if FAIL_NEXT.swap(false, Ordering::SeqCst) {
        return Err(format!("{}: injected failure", path.display()));
    }
    file.sync_all().map_err(|e| format!("{}: {e}", path.display()))
}

/// The background half: sync what is at `path` now. A file that was removed meanwhile needs nothing.
fn sync_path(path: &Path) -> Result<(), String> {
    #[cfg(test)]
    if FAIL_NEXT.swap(false, Ordering::SeqCst) {
        return Err(format!("{}: injected failure", path.display()));
    }
    match fs::OpenOptions::new().write(true).open(path) {
        Ok(file) => file.sync_all().map_err(|e| format!("{}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}
#[cfg(test)]
static FAIL_NEXT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Test seam: the next sync fails (the failure is attributed to the write's owner).
#[cfg(test)]
pub(crate) fn fail_next_sync_for_test() {
    FAIL_NEXT.store(true, Ordering::SeqCst);
}

/// Waits until every queued sync has been tried.
pub(crate) fn flush() {
    SYNCS.with(WriteQueue::flush);
}

/// The syncs that failed since the last call.
pub(crate) fn take_failures() -> Vec<Failure> {
    SYNCS.with(WriteQueue::take_failures)
}

#[cfg(test)]
pub(crate) fn set_delay_for_test(delay: std::time::Duration) {
    SYNCS.with(|q| q.set_delay_for_test(delay));
}
#[cfg(test)]
pub(crate) fn is_busy() -> bool {
    SYNCS.with(WriteQueue::is_busy)
}
