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

/// Once, at the first start of a build with this folder: every `crash-*.txt`
/// in `<config>/crashes/` was written by the fork (upstream's `crashfile` is
/// new in the build that adds this), so it moves into the fork's folder and
/// no old local report (log tail, state snapshot) can go to a server at a
/// login. Later starts leave `crashes/` to upstream. How many moved.
pub fn migrate_legacy(config: &Path) -> usize {
    let own = in_config(config);
    let marker = own.join(MOVED);
    if marker.exists() || std::fs::create_dir_all(&own).is_err() {
        return 0;
    }
    let mut moved = 0;
    let mut failed = false;
    for entry in std::fs::read_dir(config.join("crashes")).into_iter().flatten().flatten() {
        let from = entry.path();
        let is_report = from
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("crash-") && n.ends_with(".txt"));
        let Some(name) = from.file_name().filter(|_| is_report) else { continue };
        let to = own.join(name);
        let _ = std::fs::remove_file(&to);
        if std::fs::rename(&from, &to).is_ok() || (std::fs::copy(&from, &to).is_ok() && std::fs::remove_file(&from).is_ok()) {
            moved += 1;
        } else {
            failed = true;
        }
    }
    if !failed {
        let _ = std::fs::write(&marker, "");
    }
    if moved > 0 {
        eprintln!("report: {moved} local crash report(s) of an older build moved to {DIR}/ (never sent to a server)");
    }
    moved
}
