// Code in this file is LoopRook's addition for EQOA Enhanced and is under the GNU Affero General Public License v3 or later (see LICENSE).
//! A background writer for the files a mod's settings land in.
//!
//! The atomic replace in `storage.rs` creates a temporary file, writes it, `sync_all`s it and renames it.
//! The sync can take well over a second on a slow disk, and it used to run on the frame thread (a freeze of
//! 1.3 to 1.8 seconds when a mod saved its settings, such as the chat window being moved). The frame thread
//! now validates, encodes and quota-checks as before, then hands the bytes here and carries on.
//!
//! * One worker thread per manager, started by the first write and ended when the queue is dropped (the
//!   drop writes whatever is still waiting first).
//! * Writes are coalesced per target file: a write still waiting when a newer one for the same file
//!   arrives is replaced (last value wins). Writes to different files run in the order they arrived.
//! * `flush` waits until every accepted write has been tried; the manager calls it where a write must not
//!   be lost or reordered (mod disable, logout, profile switch, quit, and before any synchronous write or
//!   read of the same files).
//! * A write that fails is kept as a [`Failure`] for the manager to show as a diagnostic for that mod.
use std::{
    collections::VecDeque,
    panic::{AssertUnwindSafe, catch_unwind},
    path::PathBuf,
    sync::{Arc, Condvar, Mutex},
    thread::{self, JoinHandle},
};

/// The write itself: runs on the worker thread, once.
pub(crate) type Task = Box<dyn FnOnce() -> Result<(), String> + Send>;

/// A write that did not reach the disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Failure {
    /// The mod the write was made for; none for the profile file as a whole.
    pub owner: Option<String>,
    pub message: String,
}

struct Job {
    key: PathBuf,
    owner: Option<String>,
    run: Task,
}

#[derive(Default)]
struct State {
    pending: VecDeque<Job>,
    busy: bool,
    failures: Vec<Failure>,
    closed: bool,
}

struct Shared {
    state: Mutex<State>,
    /// Wakes the worker (new job, close) and the waiters (idle) alike.
    changed: Condvar,
    /// Test seam: the worker sleeps this long before each write, like a slow disk.
    #[cfg(test)]
    delay_ms: std::sync::atomic::AtomicU64,
}

pub(crate) struct WriteQueue {
    shared: Arc<Shared>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl Default for WriteQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl WriteQueue {
    pub fn new() -> Self {
        Self {
            shared: Arc::new(Shared {
                state: Mutex::new(State::default()),
                changed: Condvar::new(),
                #[cfg(test)]
                delay_ms: std::sync::atomic::AtomicU64::new(0),
            }),
            worker: Mutex::new(None),
        }
    }

    /// Accepts a write for `key` (the target file). Returns at once; the write happens on the worker thread.
    /// A write for the same `key` that has not started yet is replaced by this one.
    pub fn submit(&self, key: PathBuf, owner: Option<String>, run: Task) {
        let job = Job { key, owner, run };
        let job = {
            let mut state = self.shared.state.lock().unwrap();
            if state.closed {
                Some(job)
            } else {
                if let Some(waiting) = state.pending.iter_mut().find(|j| j.key == job.key) {
                    *waiting = job;
                } else {
                    state.pending.push_back(job);
                }
                None
            }
        };
        if let Some(job) = job {
            // Closed (only while being dropped): write on this thread rather than lose it.
            self.run_inline(job);
            return;
        }
        if !self.ensure_worker() {
            // No thread could be made: write on this thread rather than lose the write.
            let drained: Vec<Job> = self.shared.state.lock().unwrap().pending.drain(..).collect();
            for job in drained {
                self.run_inline(job);
            }
            return;
        }
        self.shared.changed.notify_all();
    }

    /// Waits until every accepted write has been tried (written or failed).
    pub fn flush(&self) {
        let mut state = self.shared.state.lock().unwrap();
        while !state.pending.is_empty() || state.busy {
            state = self.shared.changed.wait(state).unwrap();
        }
    }

    /// The writes that failed since the last call.
    pub fn take_failures(&self) -> Vec<Failure> {
        std::mem::take(&mut self.shared.state.lock().unwrap().failures)
    }

    /// Whether any write is waiting or in progress (not blocking).
    #[cfg(test)]
    pub fn is_busy(&self) -> bool {
        let state = self.shared.state.lock().unwrap();
        !state.pending.is_empty() || state.busy
    }

    /// Test seam: every write waits this long on the worker first (a disk whose sync takes a second).
    #[cfg(test)]
    pub fn set_delay_for_test(&self, delay: std::time::Duration) {
        self.shared
            .delay_ms
            .store(delay.as_millis() as u64, std::sync::atomic::Ordering::Relaxed);
    }

    fn run_inline(&self, job: Job) {
        let owner = job.owner.clone();
        if let Err(message) = attempt(job.run) {
            self.shared.state.lock().unwrap().failures.push(Failure { owner, message });
        }
    }

    fn ensure_worker(&self) -> bool {
        let mut worker = self.worker.lock().unwrap();
        if worker.is_some() {
            return true;
        }
        let shared = self.shared.clone();
        match thread::Builder::new()
            .name("addon-settings-write".into())
            .spawn(move || work(&shared))
        {
            Ok(handle) => {
                *worker = Some(handle);
                true
            }
            Err(_) => false,
        }
    }
}

impl Drop for WriteQueue {
    fn drop(&mut self) {
        self.shared.state.lock().unwrap().closed = true;
        self.shared.changed.notify_all();
        if let Some(handle) = self.worker.lock().unwrap().take() {
            // The worker empties the queue before it looks at `closed`.
            let _ = handle.join();
        }
    }
}

fn attempt(run: Task) -> Result<(), String> {
    catch_unwind(AssertUnwindSafe(run)).unwrap_or_else(|_| Err("the write panicked".into()))
}

fn work(shared: &Shared) {
    loop {
        let job = {
            let mut state = shared.state.lock().unwrap();
            loop {
                if let Some(job) = state.pending.pop_front() {
                    state.busy = true;
                    break job;
                }
                if state.closed {
                    return;
                }
                state = shared.changed.wait(state).unwrap();
            }
        };
        #[cfg(test)]
        {
            let ms = shared.delay_ms.load(std::sync::atomic::Ordering::Relaxed);
            if ms > 0 {
                thread::sleep(std::time::Duration::from_millis(ms));
            }
        }
        let owner = job.owner.clone();
        let result = attempt(job.run);
        let mut state = shared.state.lock().unwrap();
        state.busy = false;
        if let Err(message) = result {
            state.failures.push(Failure { owner, message });
        }
        drop(state);
        shared.changed.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::atomic::{AtomicUsize, Ordering},
        time::{Duration, Instant},
    };

    fn recorder() -> (Arc<Mutex<Vec<u32>>>, impl Fn(u32) -> Task) {
        let log = Arc::new(Mutex::new(Vec::new()));
        let l = log.clone();
        (log, move |n| {
            let l = l.clone();
            Box::new(move || {
                l.lock().unwrap().push(n);
                Ok(())
            }) as Task
        })
    }

    #[test]
    fn write_queue_submit_returns_before_a_slow_write_finishes_and_flush_waits_for_it() {
        let queue = WriteQueue::new();
        queue.set_delay_for_test(Duration::from_millis(400));
        let (log, job) = recorder();
        let start = Instant::now();
        queue.submit(PathBuf::from("a"), None, job(1));
        assert!(start.elapsed() < Duration::from_millis(200), "submit blocked on the write: {:?}", start.elapsed());
        assert!(log.lock().unwrap().is_empty(), "the write has not run yet");
        queue.flush();
        assert_eq!(*log.lock().unwrap(), vec![1]);
        assert!(!queue.is_busy());
    }

    #[test]
    fn write_queue_waiting_writes_for_one_file_collapse_to_the_last_and_other_files_keep_order() {
        let queue = WriteQueue::new();
        queue.set_delay_for_test(Duration::from_millis(150));
        let (log, job) = recorder();
        // 1 starts at once; 2, 3, 4 queue behind it: 2 and 4 are for the same file, so 4 replaces 2.
        queue.submit(PathBuf::from("a"), None, job(1));
        std::thread::sleep(Duration::from_millis(40));
        queue.submit(PathBuf::from("b"), None, job(2));
        queue.submit(PathBuf::from("c"), None, job(3));
        queue.submit(PathBuf::from("b"), None, job(4));
        queue.flush();
        assert_eq!(*log.lock().unwrap(), vec![1, 4, 3]);
    }

    #[test]
    fn write_queue_failures_are_kept_for_their_owner_and_do_not_stop_later_writes() {
        let queue = WriteQueue::new();
        let ran = Arc::new(AtomicUsize::new(0));
        queue.submit(PathBuf::from("a"), Some("mod_a".into()), Box::new(|| Err("disk full".into())));
        let r = ran.clone();
        queue.submit(
            PathBuf::from("b"),
            None,
            Box::new(move || {
                r.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }),
        );
        queue.submit(PathBuf::from("c"), Some("mod_c".into()), Box::new(|| panic!("boom")));
        queue.flush();
        assert_eq!(ran.load(Ordering::SeqCst), 1);
        let failures = queue.take_failures();
        assert_eq!(failures.len(), 2);
        assert_eq!(failures[0], Failure { owner: Some("mod_a".into()), message: "disk full".into() });
        assert_eq!(failures[1].owner.as_deref(), Some("mod_c"));
        assert!(queue.take_failures().is_empty(), "taken once");
    }

    #[test]
    fn write_queue_dropping_it_writes_what_is_still_waiting() {
        let (log, job) = recorder();
        {
            let queue = WriteQueue::new();
            queue.set_delay_for_test(Duration::from_millis(100));
            queue.submit(PathBuf::from("a"), None, job(1));
            queue.submit(PathBuf::from("b"), None, job(2));
        }
        assert_eq!(*log.lock().unwrap(), vec![1, 2]);
    }
}
