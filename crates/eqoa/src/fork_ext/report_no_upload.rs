// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Enhanced never uploads a crash or bug report (fork policy: local saves only,
//! through `report_core`). Upstream's `crashfile::post_one` POSTs a kept crash
//! file to a report server; its one hook line asks `blocked()` first and
//! returns without opening a connection. Upstream's code stays as it is.

/// Whether upstream's crash-report upload to a report server is refused.
/// Always true in Enhanced; the test below holds the hook to it.
pub fn blocked() -> bool {
    true
}
