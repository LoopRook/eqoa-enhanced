//! Test-only, current-thread allocation meter; disabled outside measured scopes.
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
thread_local! {
    static ENABLED: Cell<bool> = const {Cell::new(false)};
    static COUNT: Cell<u64> = const {Cell::new(0)};
    static BYTES: Cell<u64> = const {Cell::new(0)};
}
struct Meter;
#[global_allocator]
static ALLOCATOR: Meter = Meter;
fn note(bytes: usize) {
    if ENABLED.try_with(|e| e.get()).unwrap_or(false) {
        let _ = COUNT.try_with(|c| c.set(c.get() + 1));
        let _ = BYTES.try_with(|c| c.set(c.get() + bytes as u64));
    }
}
unsafe impl GlobalAlloc for Meter {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        note(l.size());
        unsafe { System.alloc(l) }
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        note(l.size());
        unsafe { System.alloc_zeroed(l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        note(n);
        unsafe { System.realloc(p, l, n) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
}
pub(crate) fn measure<T>(f: impl FnOnce() -> T) -> (T, std::time::Duration, u64, u64) {
    struct Stop;
    impl Drop for Stop {
        fn drop(&mut self) {
            ENABLED.with(|e| e.set(false));
        }
    }
    COUNT.with(|c| c.set(0));
    BYTES.with(|b| b.set(0));
    ENABLED.with(|e| e.set(true));
    let stop = Stop;
    let start = std::time::Instant::now();
    let result = f();
    let elapsed = start.elapsed();
    drop(stop);
    (
        result,
        elapsed,
        COUNT.with(|c| c.get()),
        BYTES.with(|b| b.get()),
    )
}
