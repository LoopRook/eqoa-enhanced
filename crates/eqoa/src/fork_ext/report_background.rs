// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see LICENSE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Report I/O never runs on the per-frame snapshot path.
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const REFRESH: Duration = Duration::from_secs(30);
const PENDING: &str = "refresh pending";
static BACKGROUND: OnceLock<Option<Worker>> = OnceLock::new();

enum Job {
    State(PathBuf, String),
    Flush(mpsc::Sender<()>),
    Stop,
}

pub(super) struct Worker {
    send: mpsc::SyncSender<Job>,
    enabled: Arc<Mutex<String>>,
    thread: Option<JoinHandle<()>>,
}

impl Worker {
    pub(super) fn spawn(
        scan: impl Fn() -> String + Send + 'static,
        write: impl Fn(&PathBuf, &str) -> std::io::Result<()> + Send + 'static,
    ) -> std::io::Result<Self> {
        // A slow disk cannot grow an unbounded queue. A full slot is retried at
        // the next capture; STATE still holds the latest text for panic reports.
        let (send, receive) = mpsc::sync_channel(1);
        let enabled = Arc::new(Mutex::new(PENDING.into()));
        let cache = Arc::clone(&enabled);
        let thread = std::thread::Builder::new()
            .name("report-io".into())
            .spawn(move || {
                let mut refresh_at = Instant::now();
                let mut written: Option<(PathBuf, String)> = None;
                loop {
                    if Instant::now() >= refresh_at {
                        let fresh = scan();
                        *cache.lock().unwrap_or_else(|e| e.into_inner()) = fresh;
                        refresh_at = Instant::now() + REFRESH;
                    }
                    match receive.recv_timeout(refresh_at.saturating_duration_since(Instant::now()))
                    {
                        Ok(Job::State(path, text)) => {
                            if written
                                .as_ref()
                                .is_none_or(|(old_path, old)| *old_path != path || *old != text)
                            {
                                if write(&path, &text).is_ok() {
                                    written = Some((path, text));
                                }
                            }
                        }
                        Ok(Job::Flush(done)) => {
                            let _ = done.send(());
                        }
                        Ok(Job::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                    }
                }
            })?;
        Ok(Self {
            send,
            enabled,
            thread: Some(thread),
        })
    }

    pub(super) fn offer(&self, path: PathBuf, text: String) {
        let _ = self.send.try_send(Job::State(path, text));
    }

    fn cached_enabled(&self) -> String {
        self.enabled
            .try_lock()
            .map(|s| s.clone())
            .unwrap_or_else(|_| PENDING.into())
    }

    pub(super) fn flush(&self) {
        let (send, receive) = mpsc::channel();
        if self.send.send(Job::Flush(send)).is_ok() {
            let _ = receive.recv_timeout(Duration::from_secs(5));
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.send.send(Job::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Called at report startup, never lazily from a frame. A worker starts with an
/// immediate scan, then refreshes at most once per 30 s (including profile changes).
pub(super) fn start() {
    BACKGROUND.get_or_init(|| {
        Worker::spawn(super::report::enabled_mods_addons_string, |path, text| {
            std::fs::write(path, text)
        })
        .ok()
    });
}

/// Reads memory only; it never starts a worker, scans a directory or reads settings.
pub(crate) fn enabled_mods_addons_cached() -> String {
    BACKGROUND
        .get()
        .and_then(Option::as_ref)
        .map_or_else(|| PENDING.into(), Worker::cached_enabled)
}

pub(super) fn offer(path: PathBuf, text: String) {
    if let Some(Some(worker)) = BACKGROUND.get() {
        worker.offer(path, text);
    }
}

pub(super) fn flush() {
    if let Some(Some(worker)) = BACKGROUND.get() {
        worker.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn report_background_scans_and_changed_writes_run_on_worker_only() {
        let owner = std::thread::current().id();
        let scans = Arc::new(AtomicUsize::new(0));
        let writes = Arc::new(AtomicUsize::new(0));
        let scan_count = Arc::clone(&scans);
        let write_count = Arc::clone(&writes);
        let worker = Worker::spawn(
            move || {
                assert_ne!(std::thread::current().id(), owner);
                scan_count.fetch_add(1, Ordering::SeqCst);
                "Wereoxxs".into()
            },
            move |_, _| {
                assert_ne!(std::thread::current().id(), owner);
                write_count.fetch_add(1, Ordering::SeqCst);
                Ok(())
            },
        )
        .unwrap();
        worker.offer("state.txt".into(), "first".into());
        worker.flush();
        assert_eq!(worker.cached_enabled(), "Wereoxxs");
        for _ in 0..3 {
            worker.offer("state.txt".into(), "first".into());
            worker.flush();
        }
        assert_eq!(
            writes.load(Ordering::SeqCst),
            1,
            "unchanged text must not be rewritten"
        );
        assert_eq!(
            scans.load(Ordering::SeqCst),
            1,
            "captures must not trigger addon scans"
        );
        worker.offer("state.txt".into(), "changed".into());
        worker.flush();
        assert_eq!(writes.load(Ordering::SeqCst), 2);
    }
}
