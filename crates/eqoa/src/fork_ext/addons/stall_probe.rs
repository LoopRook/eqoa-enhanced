// Code in this file is LoopRook's addition for EQOA Enhanced and is under the GNU Affero General Public License v3 or later (see LICENSE).
//! An always-on probe that names where a slow frame's time went.
//!
//! The log's `stall:` line says a frame took 1.3 s and that its `hud` phase did (owner logs 2026-11-24). `hud`
//! holds the addon manager, every mod's Lua, the profile and settings files, the map, the HUD. This probe
//! says which part: a section that takes `SLOW` or more writes one line of its own,
//! `slow: <section> <detail> took N ms`, as it ends, a moment before the `stall:` line.
//!
//! Cost: two clock reads and a thread-local counter per section, nothing stored, nothing allocated unless a
//! section is slow (then one line, at most `MAX_LINES` in a run). Sections are placed at the places that can
//! wait: a mod's frame, its events, the profile and settings writes, the file syncs, the waits for the
//! background writer, the font build, the map, the HUD's stages.
//!
//! The background writers (`write_queue.rs`) report here too: a write that took `SLOW_WRITE` or more is a line,
//! and a slow line says whether a background write was running at that moment (a frame's file call can wait
//! behind another file's sync on the same disk).
use std::{
    cell::Cell,
    sync::{
        OnceLock,
        atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

/// A section this long or longer is said.
pub(crate) const SLOW: Duration = Duration::from_millis(150);
/// A background write this long or longer is said.
pub(crate) const SLOW_WRITE: Duration = Duration::from_millis(300);
/// Lines said in one run, at most (a machine that is slow all the time must not fill the log).
const MAX_LINES: u32 = 120;

static SAID: AtomicU32 = AtomicU32::new(0);
/// Background writes running now, and when the oldest running one began (ms since `epoch`).
static WRITING: AtomicUsize = AtomicUsize::new(0);
static WRITING_SINCE_MS: AtomicU64 = AtomicU64::new(0);
/// The longest background write so far, in ms.
static LONGEST_WRITE_MS: AtomicU64 = AtomicU64::new(0);

thread_local! {
    static DEPTH: Cell<u32> = const { Cell::new(0) };
}
#[cfg(test)]
thread_local! {
    static SEEN: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn epoch() -> Instant {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    *EPOCH.get_or_init(Instant::now)
}
fn since_epoch_ms() -> u64 {
    epoch().elapsed().as_millis() as u64
}

/// A section of the frame, timed until this drops.
pub(crate) struct Scope<'a> {
    label: &'static str,
    detail: &'a str,
    start: Instant,
}

/// `label`: what the section is (a fixed phrase); the time is said if it is slow.
pub(crate) fn scope(label: &'static str) -> Scope<'static> {
    scope_of(label, "")
}

/// `label` and `detail` (the mod, the file, the call): said together if the section is slow.
pub(crate) fn scope_of<'a>(label: &'static str, detail: &'a str) -> Scope<'a> {
    DEPTH.with(|d| d.set(d.get() + 1));
    Scope { label, detail, start: Instant::now() }
}

impl Drop for Scope<'_> {
    fn drop(&mut self) {
        let took = self.start.elapsed();
        let depth = DEPTH.with(|d| {
            let now = d.get().saturating_sub(1);
            d.set(now);
            now
        });
        if took >= SLOW {
            say(depth, self.label, self.detail, took);
        }
    }
}

thread_local! {
    static STAGE: std::cell::RefCell<Option<Scope<'static>>> = const { std::cell::RefCell::new(None) };
}

/// A run of sections one after another in a long function: `stage` ends the one before (said if it was slow) and
/// starts the next, so each boundary is one call in the code and nothing around it changes. `stage_end` ends the
/// last one.
pub(crate) fn stage(label: &'static str) {
    STAGE.with(|s| {
        let mut s = s.borrow_mut();
        *s = None;
        *s = Some(scope(label));
    });
}

pub(crate) fn stage_end() {
    STAGE.with(|s| *s.borrow_mut() = None);
}

fn say(depth: u32, label: &str, detail: &str, took: Duration) {
    if SAID.fetch_add(1, Ordering::Relaxed) >= MAX_LINES {
        return;
    }
    let detail = if detail.is_empty() { String::new() } else { format!(" {detail}") };
    let inside = if depth == 0 { String::new() } else { format!(" (inside {depth} more)") };
    let line = format!("slow: {label}{detail} took {} ms{inside}{}", took.as_millis(), background());
    emit(&line);
}

/// What the background writers were doing: part of a slow line.
fn background() -> String {
    let running = WRITING.load(Ordering::Relaxed);
    if running == 0 {
        return String::new();
    }
    let for_ms = since_epoch_ms().saturating_sub(WRITING_SINCE_MS.load(Ordering::Relaxed));
    format!("; a background write was running, {for_ms} ms so far")
}

fn emit(line: &str) {
    #[cfg(test)]
    {
        SEEN.with(|s| s.borrow_mut().push(line.to_owned()));
        ALL_SEEN.lock().unwrap().push(line.to_owned());
    }
    crate::say(line);
}

/// A background write begins (the writers call this on their thread).
pub(crate) fn write_began() {
    if WRITING.fetch_add(1, Ordering::Relaxed) == 0 {
        WRITING_SINCE_MS.store(since_epoch_ms(), Ordering::Relaxed);
    }
}

/// A background write ended; `what` names it (the file), `owner` the mod.
pub(crate) fn write_ended(what: &str, owner: Option<&str>, took: Duration) {
    WRITING.fetch_sub(1, Ordering::Relaxed);
    let ms = took.as_millis() as u64;
    LONGEST_WRITE_MS.fetch_max(ms, Ordering::Relaxed);
    if took >= SLOW_WRITE && SAID.fetch_add(1, Ordering::Relaxed) < MAX_LINES {
        let owner = owner.map(|o| format!(" for {o}")).unwrap_or_default();
        emit(&format!(
            "slow: background write of {what}{owner} took {ms} ms (the frame does not wait for it, unless a slow line says it did)"
        ));
    }
}

/// Every line any thread said (the background writers' lines come from their own thread).
#[cfg(test)]
static ALL_SEEN: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
#[cfg(test)]
pub(crate) fn said_anywhere_for_test(containing: &str) -> Vec<String> {
    ALL_SEEN.lock().unwrap().iter().filter(|l| l.contains(containing)).cloned().collect()
}

#[cfg(test)]
pub(crate) fn take_seen_for_test() -> Vec<String> {
    SEEN.with(|s| std::mem::take(&mut *s.borrow_mut()))
}
#[cfg(test)]
pub(crate) fn reset_for_test() {
    SAID.store(0, Ordering::Relaxed);
    SEEN.with(|s| s.borrow_mut().clear());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    // The counters are process-wide: these tests run on the test threads and read their own thread's lines.
    #[test]
    fn stall_probe_names_a_slow_section_with_its_detail_and_stays_quiet_for_a_quick_one() {
        reset_for_test();
        {
            let _quick = scope_of("mod frame", "quick_mod");
            sleep(Duration::from_millis(5));
        }
        assert!(take_seen_for_test().is_empty(), "control: a quick section says nothing");
        {
            let _slow = scope_of("mod frame", "slow_mod");
            sleep(SLOW + Duration::from_millis(60));
        }
        let seen = take_seen_for_test();
        assert_eq!(seen.len(), 1, "{seen:?}");
        assert!(seen[0].starts_with("slow: mod frame slow_mod took "), "{}", seen[0]);
        assert!(seen[0].ends_with(" ms") || seen[0].contains(" ms"), "{}", seen[0]);
    }

    #[test]
    fn stall_probe_says_which_inner_section_was_the_slow_one() {
        reset_for_test();
        {
            let _outer = scope("mods frame");
            {
                let _inner = scope_of("mod file write", "journal.json");
                sleep(SLOW + Duration::from_millis(60));
            }
        }
        let seen = take_seen_for_test();
        assert_eq!(seen.len(), 2, "{seen:?}");
        assert!(seen[0].starts_with("slow: mod file write journal.json took "), "{seen:?}");
        assert!(seen[0].contains("(inside 1 more)"), "the inner one is inside the outer: {seen:?}");
        assert!(seen[1].starts_with("slow: mods frame took "), "{seen:?}");
    }

    #[test]
    fn stall_probe_stages_end_one_before_the_next_starts() {
        reset_for_test();
        stage("hud: quick");
        sleep(Duration::from_millis(5));
        stage("hud: map");
        sleep(SLOW + Duration::from_millis(60));
        stage("hud: after");
        stage_end();
        let seen = take_seen_for_test();
        assert_eq!(seen.len(), 1, "{seen:?}");
        assert!(seen[0].starts_with("slow: hud: map took "), "{seen:?}");
    }

    #[test]
    fn stall_probe_a_slow_line_says_a_background_write_was_running() {
        reset_for_test();
        write_began();
        {
            let _slow = scope("mod file write");
            sleep(SLOW + Duration::from_millis(60));
        }
        write_ended("journal.json", Some("eqoa_creature_journal"), Duration::from_millis(10));
        let seen = take_seen_for_test();
        assert!(seen[0].contains("a background write was running"), "{seen:?}");
        reset_for_test();
        {
            let _slow = scope("mod file write");
            sleep(SLOW + Duration::from_millis(60));
        }
        let seen = take_seen_for_test();
        assert!(!seen[0].contains("background"), "control: nothing was writing: {seen:?}");
    }

    #[test]
    fn stall_probe_costs_next_to_nothing_for_a_quick_section() {
        let start = Instant::now();
        for _ in 0..100_000 {
            let _s = scope("mod frame");
        }
        // an unoptimised test build: a few microseconds a section at most (the frame has a few hundred)
        assert!(start.elapsed() < Duration::from_secs(2), "100k sections took {:?}", start.elapsed());
    }

    #[test]
    fn stall_probe_a_slow_background_write_is_a_line_of_its_own() {
        reset_for_test();
        write_began();
        write_ended("journal.json", Some("eqoa_creature_journal"), SLOW_WRITE + Duration::from_millis(5));
        let seen = take_seen_for_test();
        assert_eq!(seen.len(), 1, "{seen:?}");
        assert!(seen[0].starts_with("slow: background write of journal.json for eqoa_creature_journal took "), "{seen:?}");
        reset_for_test();
        write_began();
        write_ended("journal.json", None, Duration::from_millis(20));
        assert!(take_seen_for_test().is_empty(), "control: a quick write says nothing");
    }
}
