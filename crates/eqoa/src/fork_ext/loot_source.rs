// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Which corpse the client asked to loot (0x0015): `Play::attack_press` tells it (one hook call) and the
//! loot container's `source` reads it when the corpse's list (0x0018) arrives. Plain data, no `Play`.
use std::cell::Cell;
use std::time::Instant;

/// How long a request stays the one a reply belongs to.
pub(crate) const REPLY_MS: u64 = 5000;

thread_local! {
    static START: Instant = Instant::now();
    static REQUEST: Cell<Option<(u32, u64)>> = const { Cell::new(None) };
}
#[cfg(test)]
thread_local! {static SKEW: Cell<u64> = const { Cell::new(0) };}

fn now_ms() -> u64 {
    #[cfg(test)]
    return START.with(|s| s.elapsed().as_millis() as u64) + SKEW.with(|s| s.get());
    #[cfg(not(test))]
    START.with(|s| s.elapsed().as_millis() as u64)
}

/// A test moves the clock on by `ms`.
#[cfg(test)]
pub(crate) fn advance(ms: u64) {
    SKEW.with(|s| s.set(s.get() + ms));
}

/// The client sent a loot request for corpse `id`.
pub(crate) fn requested(id: u32) {
    let now = now_ms();
    REQUEST.with(|r| r.set(Some((id, now))));
}

/// The corpse the latest request named, when it was made within `REPLY_MS` of now. A reply uses its
/// request up: the next reply is for the next request.
pub(crate) fn take_recent() -> Option<u32> {
    let now = now_ms();
    REQUEST.with(|r| r.take()).filter(|(_, at)| now.saturating_sub(*at) <= REPLY_MS).map(|(id, _)| id)
}
