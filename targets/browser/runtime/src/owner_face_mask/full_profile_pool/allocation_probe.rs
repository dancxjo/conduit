//! Test-only per-thread requested-byte census. Fixed bookkeeping is excluded
//! from measurements; this observes the ordinary System allocator, not a pool.
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::RefCell;
// The ordinary catalog planner transiently owns more than 8192 allocations.
// This fixed test-only table is excluded from the measured product storage.
const SLOTS: usize = 65536;
#[derive(Clone, Copy)]
struct Entry {
    address: usize,
    bytes: usize,
}
struct Probe {
    active: bool,
    entries: [Entry; SLOTS],
    live: usize,
    peak: usize,
    allocations: usize,
    overflow: bool,
}
impl Probe {
    const fn new() -> Self {
        Self {
            active: false,
            entries: [Entry {
                address: 0,
                bytes: 0,
            }; SLOTS],
            live: 0,
            peak: 0,
            allocations: 0,
            overflow: false,
        }
    }
    fn insert(&mut self, address: usize, bytes: usize) {
        if address == 0 {
            return;
        }
        let start = (address >> 4) % SLOTS;
        for offset in 0..SLOTS {
            let e = &mut self.entries[(start + offset) % SLOTS];
            if e.address <= 1 {
                *e = Entry { address, bytes };
                self.live += bytes;
                self.peak = self.peak.max(self.live);
                self.allocations += 1;
                return;
            }
        }
        self.overflow = true;
    }
    fn remove(&mut self, address: usize) {
        let start = (address >> 4) % SLOTS;
        for offset in 0..SLOTS {
            let e = &mut self.entries[(start + offset) % SLOTS];
            if e.address == 0 {
                return;
            }
            if e.address == address {
                self.live -= e.bytes;
                *e = Entry {
                    address: 1,
                    bytes: 0,
                };
                return;
            }
        }
    }
}
thread_local! {static PROBE:RefCell<Probe>=const{RefCell::new(Probe::new())};}
fn record(f: impl FnOnce(&mut Probe)) {
    let _ = PROBE.try_with(|p| {
        if let Ok(mut p) = p.try_borrow_mut() {
            if p.active {
                f(&mut p);
            }
        }
    });
}
struct Observed;
#[global_allocator]
static ALLOCATOR: Observed = Observed;
// SAFETY: every operation is forwarded unchanged to System. Bookkeeping only
// observes successful pointers and never dereferences or frees those pointers.
unsafe impl GlobalAlloc for Observed {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller supplies the allocator's valid Layout.
        let p = unsafe { System.alloc(layout) };
        record(|s| s.insert(p as usize, layout.size()));
        p
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller supplies the allocator's valid Layout.
        let p = unsafe { System.alloc_zeroed(layout) };
        record(|s| s.insert(p as usize, layout.size()));
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        record(|s| s.remove(p as usize));
        // SAFETY: the pointer and Layout are forwarded from the caller.
        unsafe { System.dealloc(p, layout) }
    }
    unsafe fn realloc(&self, p: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: the allocation and new extent are forwarded unchanged.
        let next = unsafe { System.realloc(p, layout, new_size) };
        if !next.is_null() {
            record(|s| {
                s.remove(p as usize);
                s.insert(next as usize, new_size);
            });
        }
        next
    }
}
pub(super) fn measure<T>(f: impl FnOnce() -> T) -> (T, usize, usize) {
    PROBE.with(|p| {
        let mut p = p.borrow_mut();
        assert!(!p.active);
        p.entries.fill(Entry {
            address: 0,
            bytes: 0,
        });
        p.live = 0;
        p.peak = 0;
        p.allocations = 0;
        p.overflow = false;
        p.active = true;
    });
    let result = f();
    PROBE.with(|p| {
        let mut p = p.borrow_mut();
        p.active = false;
        assert!(!p.overflow, "allocation census table exhausted");
        (result, p.peak, p.allocations)
    })
}
