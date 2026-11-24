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
    let result = file.write_all(bytes).and_then(|_| file.flush()).and_then(|_| fs::rename(&temporary, target));
    if let Err(e) = result {
        drop(file);
        let _ = fs::remove_file(&temporary);
        return Err(e);
    }
    // The handle stays valid after the rename (std opens with delete sharing) and syncs the bytes now in place.
    let shown = target.display().to_string();
    SYNCS.with(|q| {
        q.submit(
            target.to_path_buf(),
            owner.map(str::to_owned),
            Box::new(move || file.sync_all().map_err(|e| format!("{shown}: {e}"))),
        )
    });
    Ok(())
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
