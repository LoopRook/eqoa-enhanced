// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Which corpse the client asked to loot (0x0015): `Play::attack_press` tells it (one hook call) and the
//! loot container's `source` reads it when the corpse's list (0x0018) arrives. Plain data, no `Play`.
//!
//! The reply does not say which request it answers, so a name is given only when it cannot be wrong:
//! when exactly one request is waiting and no two were ever waiting at once since the last quiet point.
//! Time proves nothing (an old request's reply can still arrive), so nothing is dropped on a timeout.
//! A missing "from" is acceptable; a wrong one is not.
use std::cell::Cell;

#[derive(Clone, Copy, Default)]
struct State {
    /// Requests sent and not yet answered (by a corpse's list, or the server's refusal, 0x0085).
    outstanding: u32,
    /// Two requests were waiting at once: replies cannot be told apart until a quiet point.
    tainted: bool,
    /// The corpse the latest request named.
    last: Option<u32>,
}

thread_local! {static STATE: Cell<State> = const { Cell::new(State { outstanding: 0, tainted: false, last: None }) };}

/// The client sent a loot request for corpse `id`.
pub(crate) fn requested(id: u32) {
    STATE.with(|s| {
        let mut st = s.get();
        st.tainted |= st.outstanding > 0;
        st.outstanding += 1;
        st.last = Some(id);
        s.set(st);
    });
}

/// The server answered a request without a list (it refused: 0x0085).
pub(crate) fn refused() {
    STATE.with(|s| {
        let mut st = s.get();
        st.outstanding = st.outstanding.saturating_sub(1);
        s.set(st);
    });
}

/// A corpse's list arrived (0x0018): the corpse its request named, only when that request was the only one
/// waiting and none overlapped another since the last quiet point. The reply uses its request up.
pub(crate) fn reply() -> Option<u32> {
    STATE.with(|s| {
        let mut st = s.get();
        let named = (!st.tainted && st.outstanding == 1).then_some(st.last).flatten();
        st.outstanding = st.outstanding.saturating_sub(1);
        s.set(st);
        named
    })
}

/// The loot window closed (0x0016). With no request waiting this is a quiet point: the next request starts
/// afresh. With one still waiting (sent before the close arrived) nothing is known, and nothing changes.
pub(crate) fn closed() {
    STATE.with(|s| {
        let st = s.get();
        if st.outstanding == 0 {
            s.set(State::default());
        }
    });
}

/// The world ended: no request of the old world is waiting.
pub(crate) fn clear() {
    STATE.with(|s| s.set(State::default()));
}
