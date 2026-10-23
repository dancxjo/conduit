//! Counts all allocator effects only during the production kernel Play phase.
use conduit_core::ConnectionTrack;
use conduit_std_host::{
    ExternalForeDelivery, ExternalForeOutputAdapter, KernelTimingPhase, TimerAdapter,
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    time::Duration,
};

thread_local! {
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
    static EVENTS: Cell<[usize;3]> = const { Cell::new([0;3]) };
}
fn record(index: usize) {
    let _ = ACTIVE.try_with(|active| {
        if active.get() {
            let _ = EVENTS.try_with(|events| {
                let mut counts = events.get();
                counts[index] = counts[index].saturating_add(1);
                events.set(counts);
            });
        }
    });
}
struct Counted;
unsafe impl GlobalAlloc for Counted {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record(0);
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record(0);
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record(1);
        unsafe { System.realloc(pointer, layout, size) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        record(2);
        unsafe { System.dealloc(pointer, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: Counted = Counted;

#[derive(Default)]
pub struct Probe {
    pub phases: u8,
    pub counts: [usize; 3],
}
impl TimerAdapter for Probe {
    fn wait(&mut self, _: Duration) {
        panic!("no timer needed")
    }
    fn observe_kernel_phase(&mut self, phase: KernelTimingPhase) {
        match phase {
            KernelTimingPhase::PreparationStarted => {}
            KernelTimingPhase::PlayStarted => {
                assert_eq!(self.phases, 0);
                self.phases = 1;
                EVENTS.with(|events| events.set([0; 3]));
                ACTIVE.with(|active| active.set(true));
            }
            KernelTimingPhase::PlayFinished => {
                ACTIVE.with(|active| active.set(false));
                self.counts = EVENTS.with(Cell::get);
                self.phases = 2;
            }
        }
    }
}
impl Drop for Probe {
    fn drop(&mut self) {
        ACTIVE.with(|active| active.set(false));
    }
}

pub struct Sink {
    pub bytes: Vec<u8>,
    pub calls: usize,
}
impl Sink {
    pub fn prepare() -> Self {
        Self {
            bytes: Vec::with_capacity(262144),
            calls: 0,
        }
    }
}
impl ExternalForeOutputAdapter for Sink {
    fn deliver(&mut self, delivery: ExternalForeDelivery) -> Result<(), String> {
        if delivery.track == ConnectionTrack::Payload {
            assert!(delivery.bytes.len() <= self.bytes.capacity());
            self.bytes.clear();
            self.bytes.extend_from_slice(&delivery.bytes);
            self.calls += 1;
        }
        Ok(())
    }
}
