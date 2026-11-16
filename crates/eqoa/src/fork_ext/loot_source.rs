// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Which corpse the client asked to loot (0x0015): `Play::attack_press` tells it (one hook call) and the
//! loot container's `source` reads it when the corpse's list (0x0018) arrives. Plain data, no `Play`.
use std::cell::Cell;
use std::time::Instant;

/// How long a request stays the one a reply belongs to.
pub(crate) const REPLY_MS: u64 = 5000;

/// The loot request not yet answered: the corpse it named, when it was sent, and whether another was sent
/// while it was still waiting (then the next reply could be for either, and names neither).
#[derive(Clone, Copy)]
struct Pending {
    id: u32,
    at: u64,
    crowded: bool,
}

thread_local! {
    static START: Instant = Instant::now();
    static REQUEST: Cell<Option<Pending>> = const { Cell::new(None) };
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

/// The client sent a loot request for corpse `id`. One still waiting (sent less than `REPLY_MS` ago) makes
/// the next reply ambiguous: it is not told which corpse it is for.
pub(crate) fn requested(id: u32) {
    let now = now_ms();
    let crowded = REQUEST.with(|r| r.get()).is_some_and(|p| now.saturating_sub(p.at) <= REPLY_MS);
    REQUEST.with(|r| r.set(Some(Pending { id, at: now, crowded })));
}

/// The corpse the one request in flight named, when it was made within `REPLY_MS` of now and nothing else
/// was asked meanwhile. A reply uses the request up: the next reply is for the next request.
pub(crate) fn take_recent() -> Option<u32> {
    let now = now_ms();
    REQUEST.with(|r| r.take()).filter(|p| !p.crowded && now.saturating_sub(p.at) <= REPLY_MS).map(|p| p.id)
}

/// The world ended or the loot window closed: no request is waiting.
pub(crate) fn clear() {
    REQUEST.with(|r| r.set(None));
}
