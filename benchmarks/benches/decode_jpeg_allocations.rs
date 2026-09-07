use std::alloc::{GlobalAlloc, Layout, System};
use std::fs::read;
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};

use zune_benches::sample_path;
use zune_jpeg::zune_core::bytestream::ZCursor;
use zune_jpeg::{JpegDecoder, ScanlineReadStatus, ScanlineStatus};

struct TrackingAllocator;

static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static PEAK_BYTES: AtomicUsize = AtomicUsize::new(0);
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

#[global_allocator]
static ALLOCATOR: TrackingAllocator = TrackingAllocator;

fn record_growth(size: usize) {
    let live = LIVE_BYTES.fetch_add(size, Ordering::Relaxed) + size;
    let mut peak = PEAK_BYTES.load(Ordering::Relaxed);
    while live > peak {
        match PEAK_BYTES.compare_exchange_weak(peak, live, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => break,
            Err(current) => peak = current
        }
    }
}

fn record_allocation(size: usize) {
    ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
    record_growth(size);
}

fn record_deallocation(size: usize) {
    LIVE_BYTES.fetch_sub(size, Ordering::Relaxed);
}

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            record_allocation(layout.size());
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            record_allocation(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        record_deallocation(layout.size());
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_pointer = unsafe { System.realloc(pointer, layout, new_size) };
        if !new_pointer.is_null() {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            if new_size >= layout.size() {
                record_growth(new_size - layout.size());
            } else {
                record_deallocation(layout.size() - new_size);
            }
        }
        new_pointer
    }
}

#[derive(Clone, Copy)]
struct AllocationProfile {
    peak_live_bytes: usize,
    allocations:     usize
}

fn profile<T>(operation: impl FnOnce() -> T) -> AllocationProfile {
    let baseline = LIVE_BYTES.load(Ordering::SeqCst);
    let allocation_start = ALLOCATIONS.load(Ordering::SeqCst);
    PEAK_BYTES.store(baseline, Ordering::SeqCst);
    let value = operation();
    black_box(&value);
    let result = AllocationProfile {
        peak_live_bytes: PEAK_BYTES.load(Ordering::SeqCst).saturating_sub(baseline),
        allocations:     ALLOCATIONS.load(Ordering::SeqCst) - allocation_start
    };
    drop(value);
    assert_eq!(LIVE_BYTES.load(Ordering::SeqCst), baseline);
    result
}

fn decode_scanlines(data: &[u8], rows_per_read: usize) -> u64 {
    let mut decoder = JpegDecoder::new(ZCursor::new(data));
    let mut scanlines = decoder.scanline_output();
    assert_eq!(scanlines.start().unwrap(), ScanlineStatus::Ready);
    let row_bytes = scanlines.output_row_bytes().unwrap();
    let height = scanlines.output_height().unwrap();
    let mut rows = vec![0; row_bytes * rows_per_read];
    let mut checksum = 0_u64;

    while scanlines.output_scanline() < height {
        match scanlines.read_scanlines(&mut rows, row_bytes).unwrap() {
            ScanlineReadStatus::RowsProcessed { rows: written } => {
                checksum = rows[..written * row_bytes]
                    .iter()
                    .fold(checksum, |sum, byte| sum.wrapping_add(u64::from(*byte)));
            }
            status => panic!("unexpected scanline status {status:?}")
        }
    }
    assert_eq!(scanlines.finish().unwrap(), ScanlineStatus::Complete);
    checksum
}

fn main() {
    let data =
        read(sample_path().join("test-images/jpeg/benchmarks/speed_bench_hv_subsampling.jpg"))
            .unwrap();

    let full = profile(|| JpegDecoder::new(ZCursor::new(&data)).decode().unwrap());
    let one_row = profile(|| decode_scanlines(&data, 1));
    let direct = profile(|| decode_scanlines(&data, 64));

    println!(
        "full output: peak={} bytes, allocations={}",
        full.peak_live_bytes, full.allocations
    );
    println!(
        "one-row scanlines: peak={} bytes, allocations={}",
        one_row.peak_live_bytes, one_row.allocations
    );
    println!(
        "64-row scanlines: peak={} bytes, allocations={}",
        direct.peak_live_bytes, direct.allocations
    );

    assert!(one_row.peak_live_bytes < full.peak_live_bytes);
    assert!(direct.peak_live_bytes < full.peak_live_bytes);
    assert!(one_row.allocations <= full.allocations + 4);
    assert!(direct.allocations <= full.allocations + 4);
}
