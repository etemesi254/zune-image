use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::Ordering::Relaxed;
use std::sync::atomic::{AtomicBool, AtomicUsize};

struct Counting;
static ENABLED: AtomicBool = AtomicBool::new(false);
static CALLS: AtomicUsize = AtomicUsize::new(0);
static BYTES: AtomicUsize = AtomicUsize::new(0);
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn add(bytes: usize) {
    CALLS.fetch_add(1, Relaxed);
    BYTES.fetch_add(bytes, Relaxed);
    PEAK.fetch_max(LIVE.fetch_add(bytes, Relaxed) + bytes, Relaxed);
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() && ENABLED.load(Relaxed) {
            add(layout.size());
        }
        pointer
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() && ENABLED.load(Relaxed) {
            add(layout.size());
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        if ENABLED.load(Relaxed) {
            LIVE.fetch_sub(layout.size(), Relaxed);
        }
        unsafe { System.dealloc(pointer, layout) };
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let result = unsafe { System.realloc(pointer, layout, size) };
        if !result.is_null() && ENABLED.load(Relaxed) {
            LIVE.fetch_sub(layout.size(), Relaxed);
            add(size);
        }
        result
    }
}

pub fn start() {
    assert!(!ENABLED.swap(false, Relaxed));
    for value in [&CALLS, &BYTES, &LIVE, &PEAK] {
        value.store(0, Relaxed);
    }
    ENABLED.store(true, Relaxed);
}

pub fn stop() -> (usize, usize, usize, usize) {
    ENABLED.store(false, Relaxed);
    (
        CALLS.load(Relaxed),
        BYTES.load(Relaxed),
        PEAK.load(Relaxed),
        LIVE.load(Relaxed)
    )
}
