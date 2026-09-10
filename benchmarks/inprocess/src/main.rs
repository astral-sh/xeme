//! Repeated safe-core parses. Allocator counts are a separate instrumented build.

use std::hint::black_box;
use std::time::Instant;

use oriole::{Config, EventKind, Parser};

#[cfg(all(feature = "jemalloc", feature = "mimalloc"))]
compile_error!("select at most one allocator");

#[cfg(not(any(feature = "jemalloc", feature = "mimalloc")))]
type SelectedAllocator = std::alloc::System;
#[cfg(feature = "jemalloc")]
type SelectedAllocator = tikv_jemallocator::Jemalloc;
#[cfg(feature = "mimalloc")]
type SelectedAllocator = mimalloc::MiMalloc;

#[cfg(not(any(feature = "jemalloc", feature = "mimalloc")))]
const SELECTED: SelectedAllocator = std::alloc::System;
#[cfg(feature = "jemalloc")]
const SELECTED: SelectedAllocator = tikv_jemallocator::Jemalloc;
#[cfg(feature = "mimalloc")]
const SELECTED: SelectedAllocator = mimalloc::MiMalloc;

#[cfg(not(feature = "allocation-counts"))]
#[global_allocator]
static ALLOCATOR: SelectedAllocator = SELECTED;

#[cfg(feature = "allocation-counts")]
mod counts {
    use std::alloc::{GlobalAlloc, Layout};
    use std::sync::atomic::{AtomicU64, Ordering};

    pub struct CountingAllocator;
    pub static CALLS: AtomicU64 = AtomicU64::new(0);
    pub static BYTES: AtomicU64 = AtomicU64::new(0);

    // SAFETY: Every allocation operation delegates to the selected allocator with
    // the original pointer, layout, and size. The atomics allocate no memory.
    unsafe impl GlobalAlloc for CountingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            CALLS.fetch_add(1, Ordering::Relaxed);
            BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
            // SAFETY: The caller supplied a valid allocation layout.
            unsafe { super::SELECTED.alloc(layout) }
        }
        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            CALLS.fetch_add(1, Ordering::Relaxed);
            BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
            // SAFETY: The caller supplied a valid allocation layout.
            unsafe { super::SELECTED.alloc_zeroed(layout) }
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            // SAFETY: The caller supplies this allocator's live pointer and layout.
            unsafe { super::SELECTED.dealloc(ptr, layout) }
        }
        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            CALLS.fetch_add(1, Ordering::Relaxed);
            BYTES.fetch_add(new_size as u64, Ordering::Relaxed);
            // SAFETY: The caller meets GlobalAlloc's pointer, layout, and size contract.
            unsafe { super::SELECTED.realloc(ptr, layout, new_size) }
        }
    }

    pub fn snapshot() -> (u64, u64) {
        (CALLS.load(Ordering::Relaxed), BYTES.load(Ordering::Relaxed))
    }
}

#[cfg(feature = "allocation-counts")]
#[global_allocator]
static ALLOCATOR: counts::CountingAllocator = counts::CountingAllocator;

fn hash_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(1_099_511_628_211);
    }
}

fn parse(input: &[u8], chunk_size: usize) -> Result<(u64, usize, usize), oriole::Error> {
    let mut parser = Parser::new(Config::default());
    let mut hash = 14_695_981_039_346_656_037;
    let mut elements = 0;
    let mut text_bytes = 0;
    let chunks = input.chunks(chunk_size);
    let count = chunks.len();
    for (index, chunk) in chunks.enumerate() {
        parser.feed(chunk, index + 1 == count)?;
        while let Some(event) = parser.next_event()? {
            match event.kind {
                EventKind::StartElement { name, attributes } => {
                    hash_bytes(&mut hash, b"\xffS");
                    hash_bytes(&mut hash, name.as_bytes());
                    for attribute in attributes {
                        hash_bytes(&mut hash, b"\0");
                        hash_bytes(&mut hash, attribute.name.as_bytes());
                        hash_bytes(&mut hash, b"\0");
                        hash_bytes(&mut hash, attribute.value.as_bytes());
                    }
                    hash_bytes(&mut hash, b"\0");
                    elements += 1;
                }
                EventKind::EndElement { name } => {
                    hash_bytes(&mut hash, b"\xffE");
                    hash_bytes(&mut hash, name.as_bytes());
                    hash_bytes(&mut hash, b"\0");
                }
                EventKind::Text(text) => {
                    hash_bytes(&mut hash, text.as_bytes());
                    text_bytes += text.len();
                }
                _ => {}
            }
        }
    }
    Ok((black_box(hash), elements, text_bytes))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .ok_or("usage: benchmark XML_FILE CHUNK_SIZE ITERATIONS")?;
    let chunk_size: usize = args.next().ok_or("missing chunk size")?.parse()?;
    let iterations: usize = args.next().ok_or("missing iterations")?.parse()?;
    if args.next().is_some() || chunk_size == 0 || iterations == 0 {
        return Err("expected positive chunk size and iterations".into());
    }
    let input = std::fs::read(path)?;
    if input.is_empty() {
        return Err("expected nonempty benchmark input".into());
    }
    let allocator = if cfg!(feature = "jemalloc") {
        "jemalloc"
    } else if cfg!(feature = "mimalloc") {
        "mimalloc"
    } else {
        "system"
    };
    println!(
        "{{\"allocator\":\"{allocator}\",\"instrumented\":{},\"samples\":[",
        cfg!(feature = "allocation-counts")
    );
    let mut expected = None;
    for iteration in 0..=iterations {
        #[cfg(feature = "allocation-counts")]
        let before_counts = counts::snapshot();
        let before = Instant::now();
        let (hash, elements, text_bytes) = parse(black_box(&input), chunk_size)?;
        let elapsed = before.elapsed().as_secs_f64();
        #[cfg(feature = "allocation-counts")]
        let after_counts = counts::snapshot();
        if let Some(expected) = expected {
            assert_eq!(hash, expected, "callback output changed between iterations");
        } else {
            expected = Some(hash);
        }
        print!(
            "{}{{\"iteration\":{iteration},\"warmup\":{},\"seconds\":{elapsed:.9},\"hash\":\"{hash:016x}\",\"elements\":{elements},\"text_bytes\":{text_bytes}",
            if iteration == 0 { "" } else { "," },
            iteration == 0
        );
        #[cfg(feature = "allocation-counts")]
        print!(
            ",\"allocation_calls\":{},\"requested_bytes\":{}",
            after_counts.0 - before_counts.0,
            after_counts.1 - before_counts.1
        );
        println!("}}");
    }
    println!("]}}");
    Ok(())
}
