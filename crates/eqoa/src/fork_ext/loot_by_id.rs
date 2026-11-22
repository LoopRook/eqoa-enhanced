// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Opening a corpse by its id (`eqoa.actions.open_loot(id, {keep_target = true})`): the loot request (0x0015) names
//! the corpse, so the player's target and auto-attack are left alone.
//!
//! What the server does with a 0x0015 whose corpse is not the target is not known (no server or decomp on the
//! development machine), so the first real test decides, from the server's own answer:
//! * a corpse list (0x0018) for a request sent while the target was NOT the corpse proves the by-id method;
//! * no answer within `REPLY_TICKS`, or a refusal that does not say why (0x0085 with code 3 or an unknown code), for such
//!   a request while nothing is proven falls back to the old method (target the corpse, loot, give the target back) for the rest of the session;
//! * if that retry gets no answer either, the fault was not the by-id method: the choice goes back to "not decided".
//!
//! A request sent while the target already was the corpse proves nothing either way (it is the old request), so it
//! never changes the method. Plain data and a game-step counter; no `Play`.
use std::cell::{Cell, RefCell};

/// How long a by-id open may stay unanswered before it counts against the method: 3 s on the 30 Hz clock.
pub(crate) const REPLY_TICKS: u64 = 90;
/// How long a close mark (who closed the corpse window) stays valid: 5 s.
const CLOSE_MARK_TICKS: u64 = 150;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Method {
    /// Not decided yet: by id is tried.
    Unknown,
    /// A corpse list answered a by-id request that was sent while the target was another.
    ById,
    /// The old method: target the corpse, ask, give the target back.
    Retarget,
}

#[derive(Clone, Copy, Debug)]
struct Probe {
    vm: u64,
    corpse: u32,
    sent: u64,
    by_id: bool,
    /// Sent while the player's target was another entity: its answer (or its silence) says something.
    real_test: bool,
}

/// What the caller does after the method fell back: send the open again the old way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Fallback {
    pub vm: u64,
    pub corpse: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Refusal {
    /// "Out of range." or 0x0085 code 1: the server handled the request and says why; nothing learned about the method.
    OutOfRange,
    /// 0x0085 code 2 ("someone else is already looting"): a specific answer to the request; nothing learned either.
    Understood,
    /// 0x0085 code 3 ("you cannot loot that corpse") or a code the client does not know: it may be the server's way of
    /// saying the request had no target.
    Unclear,
}

struct State {
    ticks: u64,
    method: Method,
    /// `Retarget` was chosen after a silence and its first retry has not been answered yet.
    provisional: bool,
    probe: Option<Probe>,
    /// A fallback decided where the old request cannot be sent (an event handler holds `&Play`): sent at the next step.
    retry: Option<Fallback>,
    close_mark: Option<(&'static str, u64)>,
}

thread_local! {
    static STATE: RefCell<State> = const { RefCell::new(State { ticks: 0, method: Method::Unknown, provisional: false, probe: None, retry: None, close_mark: None }) };
    static LOGGED_START: Cell<bool> = const { Cell::new(false) };
}

fn say(line: String) {
    crate::fork_ext::addons::window_takeover::log(line);
}

/// One 30 Hz game step.
pub(crate) fn step() {
    STATE.with(|s| s.borrow_mut().ticks += 1);
}

pub(crate) fn method() -> Method {
    STATE.with(|s| s.borrow().method)
}

/// Whether a `keep_target` request is sent by id (everything but a fallen-back session).
pub(crate) fn use_by_id() -> bool {
    method() != Method::Retarget
}

pub(crate) fn method_name(method: Method) -> &'static str {
    match method {
        Method::Unknown | Method::ById => "keep_target",
        Method::Retarget => "retarget",
    }
}

/// A by-id request went out for `corpse`; `target_was` is the player's target at that moment.
pub(crate) fn note_sent_by_id(vm: u64, corpse: u32, target_was: u32) {
    let real_test = target_was != corpse;
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let sent = s.ticks;
        s.probe = Some(Probe { vm, corpse, sent, by_id: true, real_test });
    });
    if !LOGGED_START.with(|l| l.replace(true)) {
        say(format!(
            "takeover: loot open method: BY ID (corpse {corpse:#x}, the target is left alone; {}). Falls back to target-then-loot if the server does not answer.",
            if real_test { "this request is the test" } else { "the target already was this corpse, so this request proves nothing yet" }
        ));
    }
}

/// A fallen-back request went out the old way.
pub(crate) fn note_sent_retarget(vm: u64, corpse: u32) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let sent = s.ticks;
        s.probe = Some(Probe { vm, corpse, sent, by_id: false, real_test: false });
    });
}

fn fall_back(s: &mut State, probe: Probe, why: &str) -> Fallback {
    s.method = Method::Retarget;
    s.provisional = true;
    s.probe = None;
    say(format!(
        "takeover: loot open method: TARGET-THEN-LOOT (fallback; {why}). The by-id open for corpse {:#x} got no usable answer, so this session targets the corpse, loots it and gives the target back.",
        probe.corpse
    ));
    Fallback { vm: probe.vm, corpse: probe.corpse }
}

/// A corpse list (0x0018) arrived while a probe was waiting. `opened` is the corpse the client's own request named for it
/// (`None` when two requests overlapped and the reply cannot be told apart); `still_waiting(vm, corpse)` says whether the
/// mod still waits for the open. The list confirms the method only when it is the probe's own corpse and the mod still
/// waits: a list for a corpse the player asked for settles nothing (the probe goes on waiting), an ambiguous one or one
/// for a request nobody waits for any more drops the probe without a verdict.
pub(crate) fn note_list_reply(opened: Option<u32>, still_waiting: impl Fn(u64, u32) -> bool) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let Some(p) = s.probe else { return };
        if !still_waiting(p.vm, p.corpse) {
            s.probe = None;
            return;
        }
        match opened {
            Some(id) if id != p.corpse => return,
            None => {
                s.probe = None;
                return;
            }
            Some(_) => {}
        }
        s.probe = None;
        if p.by_id && p.real_test && s.method == Method::Unknown {
            s.method = Method::ById;
            say(format!(
                "takeover: loot open method CONFIRMED: BY ID. The server opened corpse {:#x} for a request that did not target it (opens no longer move your target)",
                p.corpse
            ));
        } else if !p.by_id && s.provisional {
            s.provisional = false;
            say(format!(
                "takeover: loot open method CONFIRMED: TARGET-THEN-LOOT. The server opened corpse {:#x} for the old request",
                p.corpse
            ));
        }
    });
}

/// The server refused the waiting open. True when this settles the method as a fallback: the old request is queued
/// (`take_retry`) and the mod's open must stay in flight, not end as refused. A refusal when the mod no longer waits for
/// the open (`still_waiting`) settles nothing.
pub(crate) fn note_refusal(kind: Refusal, still_waiting: impl Fn(u64, u32) -> bool) -> bool {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let Some(p) = s.probe.take() else { return false };
        if !still_waiting(p.vm, p.corpse) {
            return false;
        }
        if kind == Refusal::Unclear && p.by_id && p.real_test && s.method == Method::Unknown {
            let fb = fall_back(&mut s, p, "the server refused it without saying why");
            s.retry = Some(fb);
            return true;
        }
        if !p.by_id && s.provisional {
            s.provisional = false;
        }
        false
    })
}

/// The fallback queued by a refusal, to be sent now.
pub(crate) fn take_retry() -> Option<Fallback> {
    STATE.with(|s| s.borrow_mut().retry.take())
}

/// Called every game step: a probe unanswered for `REPLY_TICKS`. `still_waiting(vm, corpse)` says whether the mod's
/// open is still in flight (a cancelled one is dropped, never retried). Returns the fallback to perform.
pub(crate) fn poll_timeout(still_waiting: impl Fn(u64, u32) -> bool) -> Option<Fallback> {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let p = s.probe?;
        if s.ticks.saturating_sub(p.sent) < REPLY_TICKS {
            return None;
        }
        s.probe = None;
        if !still_waiting(p.vm, p.corpse) {
            return None;
        }
        if p.by_id && p.real_test && s.method == Method::Unknown {
            return Some(fall_back(&mut s, p, "no answer in 3 s"));
        }
        if !p.by_id && s.provisional {
            s.method = Method::Unknown;
            s.provisional = false;
            say(format!(
                "takeover: loot open method: UNDECIDED again. Neither the by-id nor the old request for corpse {:#x} was answered, so the fault is not the by-id method; by id is tried again",
                p.corpse
            ));
        }
        None
    })
}

/// The mod's open is no longer waiting for the corpse (cancelled, ended): its probe is dropped.
pub(crate) fn drop_probe() {
    STATE.with(|s| s.borrow_mut().probe = None);
}

/// Who closed the corpse window: a mark set when it is known (the player's click on the game's window, the mod's own
/// close), taken when the close is reported.
pub(crate) fn mark_close(by: &'static str) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let at = s.ticks;
        s.close_mark = Some((by, at));
    });
}

/// `closed_by` for a `loot.changed` that reports the window gone: `"player"` or `"mod"` when a fresh mark says so,
/// else `"server"` for the server's own close (0x0016) and `"other"` for anything seen on a frame (a walk away, a zone).
pub(crate) fn take_closed_by(native_cause: &str) -> &'static str {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let now = s.ticks;
        match s.close_mark.take() {
            Some((by, at)) if now.saturating_sub(at) <= CLOSE_MARK_TICKS => by,
            _ if native_cause == "loot_close" => "server",
            _ => "other",
        }
    })
}

/// The world ended (a zone change, a respawn teleport, a disconnect, the world reset): no request of the old world waits,
/// no mark applies. The chosen method is kept: the zone is the same server, which answers the same way. Called from
/// `window_takeover::release_all`, which every one of those ends runs.
pub(crate) fn clear() {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.probe = None;
        s.retry = None;
        s.close_mark = None;
    });
}

/// The connection ended (`Event::Disconnected`, which is also how the player switches server): the next server may treat
/// a loot request that names a corpse the player has not targeted differently, so the method is decided again, and the
/// log says which method is in use again at the next request.
pub(crate) fn reset_session() {
    clear();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.method = Method::Unknown;
        s.provisional = false;
    });
    LOGGED_START.with(|l| l.set(false));
}

#[cfg(test)]
pub(crate) fn reset_for_test() {
    STATE.with(|s| {
        *s.borrow_mut() = State { ticks: 0, method: Method::Unknown, provisional: false, probe: None, retry: None, close_mark: None };
    });
    LOGGED_START.with(|l| l.set(false));
}

/// A test world in which the client opens corpses the old way (the state after a fallback), so a test written for the
/// old rules (Auto-loot waits out a fight, the target is moved and put back) keeps meaning what it meant.
#[cfg(test)]
pub(crate) fn force_method_for_test(method: Method) {
    STATE.with(|s| s.borrow_mut().method = method);
}

#[cfg(test)]
pub(crate) fn advance_for_test(ticks: u64) {
    STATE.with(|s| s.borrow_mut().ticks += ticks);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() {
        reset_for_test();
        let _ = crate::fork_ext::addons::window_takeover::take_log_for_test();
    }

    #[test]
    fn a_list_for_a_request_that_did_not_target_the_corpse_proves_by_id() {
        fresh();
        note_sent_by_id(1, 0x20, 0x99);
        note_list_reply(Some(0x20), |_, _| true);
        assert_eq!(method(), Method::ById);
        assert!(use_by_id());
        let log = crate::fork_ext::addons::window_takeover::take_log_for_test().join("\n");
        assert!(log.contains("BY ID"), "{log}");
        assert!(log.contains("CONFIRMED"), "{log}");
    }

    #[test]
    fn a_request_for_the_targeted_corpse_proves_nothing_either_way() {
        fresh();
        note_sent_by_id(1, 0x20, 0x20);
        advance_for_test(REPLY_TICKS + 5);
        assert!(poll_timeout(|_, _| true).is_none(), "silence for the old request is not the by-id method's fault");
        assert_eq!(method(), Method::Unknown);
        note_sent_by_id(1, 0x20, 0x20);
        note_list_reply(Some(0x20), |_, _| true);
        assert_eq!(method(), Method::Unknown, "an answer to it proves nothing");
    }

    #[test]
    fn silence_after_a_real_by_id_request_falls_back_once_and_says_so() {
        fresh();
        note_sent_by_id(7, 0x20, 0x99);
        advance_for_test(REPLY_TICKS - 1);
        assert!(poll_timeout(|_, _| true).is_none(), "not yet");
        advance_for_test(1);
        assert_eq!(poll_timeout(|_, _| true), Some(Fallback { vm: 7, corpse: 0x20 }));
        assert_eq!(method(), Method::Retarget);
        assert!(!use_by_id());
        assert_eq!(method_name(method()), "retarget");
        let log = crate::fork_ext::addons::window_takeover::take_log_for_test().join("\n");
        assert!(log.contains("TARGET-THEN-LOOT"), "{log}");
        assert!(log.contains("fallback"), "{log}");
    }

    #[test]
    fn a_cancelled_open_is_not_retried() {
        fresh();
        note_sent_by_id(7, 0x20, 0x99);
        advance_for_test(REPLY_TICKS);
        assert!(poll_timeout(|_, _| false).is_none());
        assert_eq!(method(), Method::Unknown);
    }

    #[test]
    fn an_unexplained_refusal_falls_back_but_a_reasoned_one_does_not() {
        fresh();
        note_sent_by_id(7, 0x20, 0x99);
        assert!(!note_refusal(Refusal::OutOfRange, |_, _| true));
        note_sent_by_id(7, 0x20, 0x99);
        assert!(!note_refusal(Refusal::Understood, |_, _| true));
        assert_eq!(method(), Method::Unknown, "the server said why: nothing learned about the method");
        assert!(take_retry().is_none());
        note_sent_by_id(7, 0x20, 0x99);
        assert!(note_refusal(Refusal::Unclear, |_, _| true));
        assert_eq!(method(), Method::Retarget);
        assert_eq!(take_retry(), Some(Fallback { vm: 7, corpse: 0x20 }));
        assert!(take_retry().is_none(), "sent once");
    }

    #[test]
    fn a_proven_method_never_falls_back() {
        fresh();
        note_sent_by_id(7, 0x20, 0x99);
        note_list_reply(Some(0x20), |_, _| true);
        note_sent_by_id(7, 0x21, 0x99);
        assert!(!note_refusal(Refusal::Unclear, |_, _| true));
        note_sent_by_id(7, 0x21, 0x99);
        advance_for_test(REPLY_TICKS + 1);
        assert!(poll_timeout(|_, _| true).is_none());
        assert_eq!(method(), Method::ById);
    }

    #[test]
    fn when_the_old_request_is_silent_too_the_choice_is_undone() {
        fresh();
        note_sent_by_id(7, 0x20, 0x99);
        advance_for_test(REPLY_TICKS);
        let fb = poll_timeout(|_, _| true).expect("fell back");
        note_sent_retarget(fb.vm, fb.corpse);
        advance_for_test(REPLY_TICKS);
        assert!(poll_timeout(|_, _| true).is_none());
        assert_eq!(method(), Method::Unknown, "neither request was answered: not the by-id method's fault");
    }

    #[test]
    fn when_the_old_request_is_answered_the_fallback_is_confirmed() {
        fresh();
        note_sent_by_id(7, 0x20, 0x99);
        advance_for_test(REPLY_TICKS);
        let fb = poll_timeout(|_, _| true).expect("fell back");
        note_sent_retarget(fb.vm, fb.corpse);
        note_list_reply(Some(0x20), |_, _| true);
        assert_eq!(method(), Method::Retarget);
        note_sent_by_id(7, 0x21, 0x99); // never sent in a fallen-back session, but a stray probe changes nothing
        advance_for_test(REPLY_TICKS + 1);
        assert!(poll_timeout(|_, _| true).is_none());
        assert_eq!(method(), Method::Retarget);
    }

    #[test]
    fn a_list_for_another_corpse_or_for_a_request_nobody_waits_for_confirms_nothing() {
        fresh();
        // the player's own corpse opens first: the probe keeps waiting for its own
        note_sent_by_id(1, 0x20, 0x99);
        note_list_reply(Some(0x30), |_, _| true);
        assert_eq!(method(), Method::Unknown);
        note_list_reply(Some(0x20), |_, _| true);
        assert_eq!(method(), Method::ById, "its own list confirms");
        // an ambiguous list (two requests overlapped) drops the probe without a verdict
        fresh();
        note_sent_by_id(1, 0x20, 0x99);
        note_list_reply(None, |_, _| true);
        assert_eq!(method(), Method::Unknown);
        advance_for_test(REPLY_TICKS + 1);
        assert!(poll_timeout(|_, _| true).is_none(), "and nothing is left to time out");
        // the mod's request is gone
        fresh();
        note_sent_by_id(1, 0x20, 0x99);
        note_list_reply(Some(0x20), |_, _| false);
        assert_eq!(method(), Method::Unknown);
        fresh();
        note_sent_by_id(1, 0x20, 0x99);
        assert!(!note_refusal(Refusal::Unclear, |_, _| false));
        assert_eq!(method(), Method::Unknown, "a refusal for a request nobody waits for falls back to nothing");
    }

    #[test]
    fn a_disconnect_forgets_the_method_and_a_zone_change_keeps_it() {
        fresh();
        note_sent_by_id(1, 0x20, 0x99);
        note_list_reply(Some(0x20), |_, _| true);
        note_sent_by_id(1, 0x21, 0x99);
        clear();
        assert_eq!(method(), Method::ById, "same server");
        advance_for_test(REPLY_TICKS + 1);
        assert!(poll_timeout(|_, _| true).is_none(), "the waiting request is forgotten");
        reset_session();
        assert_eq!(method(), Method::Unknown, "another server decides again");
    }

    #[test]
    fn who_closed_the_window_is_marked_taken_once_and_expires() {
        fresh();
        mark_close("player");
        assert_eq!(take_closed_by("loot_close"), "player");
        assert_eq!(take_closed_by("loot_close"), "server", "the mark is used up");
        assert_eq!(take_closed_by("frame"), "other");
        mark_close("mod");
        advance_for_test(CLOSE_MARK_TICKS + 1);
        assert_eq!(take_closed_by("frame"), "other", "a stale mark is not used");
    }
}
