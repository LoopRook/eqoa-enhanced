// Code in this file is LoopRook's, published under the GNU Affero General Public License v3 or later (see LICENSE).
//! Where this fork keeps its own local crash reports.
//!
//! Upstream's `crashfile.rs` keeps scrubbed crash reports in `<config>/crashes/`
//! (`crash-*.txt`) and sends them to a server that offers to take them. The
//! fork's local reports carry a log tail and a state snapshot and must never
//! be sent, so they live in a folder of their own: neither side's pruning,
//! acknowledgement, deletion or offer reaches the other's files.

use std::path::{Path, PathBuf};

/// The fork's folder, beside upstream's `crashes/`.
pub const DIR: &str = "enhanced-crashes";
/// Written once the files an older build left in `crashes/` have moved.
const MOVED: &str = ".legacy-crashes-moved";

/// The fork's crash folder under the config directory `config`.
pub fn in_config(config: &Path) -> PathBuf {
    config.join(DIR)
}

/// Kept in `crashes/` when a move fails: upstream's sender only takes
/// `crash-*.txt`, so a report renamed to this prefix is never sent, and the
/// next start tries to move it again.
const PARKED: &str = "enhanced-";

/// Once, at the first start of a build with this folder: every `crash-*.txt`
/// in `<config>/crashes/` was written by the fork (upstream's `crashfile` is
/// new in the build that adds this), so it moves into the fork's folder and
/// no old local report (log tail, state snapshot) can go to a server at a
/// login. A report that cannot move is renamed in place out of the sender's
/// pattern (or, failing that, removed), and the migration is only marked done
/// when the folder was read and every report moved, so it retries next start.
/// Later starts leave `crashes/` to upstream. How many moved.
pub fn migrate_legacy(config: &Path) -> usize {
    let own = in_config(config);
    let marker = own.join(MOVED);
    if marker.exists() {
        return 0;
    }
    let made = std::fs::create_dir_all(&own).is_ok();
    let mut moved = 0;
    let mut failed = !made;
    let entries = match std::fs::read_dir(config.join("crashes")) {
        Ok(entries) => Some(entries),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(_) => {
            failed = true;
            None
        }
    };
    for entry in entries.into_iter().flatten() {
        let Ok(entry) = entry else {
            failed = true;
            continue;
        };
        let from = entry.path();
        let Some(name) = from.file_name().and_then(|n| n.to_str()).map(str::to_owned) else { continue };
        let fork_name = name.strip_prefix(PARKED).unwrap_or(&name);
        if !(fork_name.starts_with("crash-") && fork_name.ends_with(".txt")) {
            continue;
        }
        let to = own.join(fork_name);
        if made && {
            let _ = std::fs::remove_file(&to);
            std::fs::rename(&from, &to).is_ok() || (std::fs::copy(&from, &to).is_ok() && std::fs::remove_file(&from).is_ok())
        } {
            moved += 1;
            continue;
        }
        failed = true;
        if name.starts_with(PARKED) {
            continue;
        }
        let parked = from.with_file_name(format!("{PARKED}{name}"));
        if std::fs::rename(&from, &parked).is_err() {
            let _ = std::fs::remove_file(&from);
        }
    }
    if !failed {
        let _ = std::fs::write(&marker, "");
    }
    if moved > 0 {
        eprintln!("report: {moved} local crash report(s) of an older build moved to {DIR}/ (never sent to a server)");
    }
    if failed {
        eprintln!("report: some local crash reports of an older build could not move to {DIR}/; kept out of the server's reach, retried next start");
    }
    moved
}
