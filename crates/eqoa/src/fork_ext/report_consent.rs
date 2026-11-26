// Code in this file is LoopRook's, published under the GNU Affero General Public License v3 or later (see LICENSE).
//! A crash report never goes to a game server on its own.
//!
//! Upstream's `crashfile` keeps a scrubbed copy of every panic in
//! `<config>/crashes/` and, when the server a player logs into offers to take
//! crash reports (0x7E64), sends each kept one over the game connection to be
//! filed as a support ticket (`send_pending`). The player is never asked: the
//! fork's crash offer (Save to Desktop / Not now / Delete) only knows its own
//! folder, `enhanced-crashes/`, so "Delete" left upstream's copy to be sent at
//! the next login (the owner's report, 2026-11-26: Ticket #3230).
//!
//! The fork's offer has no "send to this server" choice, so there is nothing
//! a player could have said yes to: the in-game send is held (`hold`), and
//! upstream's copy is no longer written (`keep_for_server`). A player who
//! wants a report to reach a person saves it to the Desktop and shares it.
//! "Delete" in the offer removes upstream's copies too (`forget_server_copies`).
//! Reports an older build left in `crashes/` stay on the PC, unsent.

use std::path::Path;

/// Whether a kept crash report may be sent to a game server. No: the offer
/// has no choice to say yes with. (A later "send to this server" choice
/// would turn this into a per-report decision.)
pub fn server_send_allowed() -> bool {
    false
}

/// Upstream's `send_pending` asks first: `Some(0)` (nothing sent) while
/// `server_send_allowed()` is false. One line says so in the log.
pub fn hold(most: usize) -> Option<usize> {
    if server_send_allowed() {
        return None;
    }
    eprintln!(
        "crash: the server offered to take crash reports ({most} bytes); none sent: Enhanced sends a crash report only if you save it and share it yourself"
    );
    Some(0)
}

/// Upstream's `crashfile::write` asks first: no copy is kept for a server
/// that can never be sent one.
pub fn keep_for_server() -> bool {
    server_send_allowed()
}

/// Every upstream-kept crash report (and its `.posted` mark) in `d`
/// (`<config>/crashes/`) removed; how many files went.
pub fn forget_server_copies_in(d: &Path) -> usize {
    let mut gone = 0;
    for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
        let p = e.path();
        let is_report = p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("crash-") && (n.ends_with(".txt") || n.ends_with(".posted")));
        if is_report && std::fs::remove_file(&p).is_ok() {
            gone += 1;
        }
    }
    gone
}

/// "Delete" in the crash offer: upstream's copies in the config directory's
/// `crashes/` go with the fork's.
#[cfg(not(target_arch = "wasm32"))]
pub fn forget_server_copies() {
    if let Some(d) = crate::settings::config_dir() {
        let n = forget_server_copies_in(&d.join("crashes"));
        if n > 0 {
            eprintln!("report: {n} crash file(s) kept for a server removed with the delete");
        }
    }
}
