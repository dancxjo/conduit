//! Count only the calling thread's ordinary Play; all output storage exists beforehand.
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
    static ENABLED: Cell<bool> = const { Cell::new(false) };
    static CALLS: Cell<usize> = const { Cell::new(0) };
    static BYTES: Cell<usize> = const { Cell::new(0) };
}
fn record(bytes: usize) {
    let _ = ENABLED.try_with(|enabled| {
        if enabled.get() {
            let _ = CALLS.try_with(|v| v.set(v.get().saturating_add(1)));
            let _ = BYTES.try_with(|v| v.set(v.get().saturating_add(bytes)));
        }
    });
}
struct Counted;
unsafe impl GlobalAlloc for Counted {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, p: *mut u8, layout: Layout, bytes: usize) -> *mut u8 {
        record(bytes);
        unsafe { System.realloc(p, layout, bytes) }
    }
    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        unsafe { System.dealloc(p, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: Counted = Counted;
#[derive(Default)]
pub struct Probe {
    pub phases: u8,
    pub calls: usize,
    pub bytes: usize,
}
impl TimerAdapter for Probe {
    fn wait(&mut self, _: Duration) {
        panic!("no timer work admitted");
    }
    fn observe_kernel_phase(&mut self, phase: KernelTimingPhase) {
        match phase {
            KernelTimingPhase::PreparationStarted => {}
            KernelTimingPhase::PlayStarted => {
                assert_eq!(self.phases, 0);
                self.phases = 1;
                CALLS.with(|v| v.set(0));
                BYTES.with(|v| v.set(0));
                ENABLED.with(|v| v.set(true));
            }
            KernelTimingPhase::PlayFinished => {
                ENABLED.with(|v| v.set(false));
                self.calls = CALLS.with(Cell::get);
                self.bytes = BYTES.with(Cell::get);
                self.phases = 2;
            }
        }
    }
}
pub fn stop() {
    ENABLED.with(|v| v.set(false));
}
pub struct Sink {
    pub bytes: Box<[u8]>,
    pub length: usize,
    pub calls: usize,
}
impl Sink {
    pub fn new() -> Self {
        Self {
            bytes: vec![0; 262144].into_boxed_slice(),
            length: 0,
            calls: 0,
        }
    }
}
impl ExternalForeOutputAdapter for Sink {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        if value.track == ConnectionTrack::Payload {
            if value.bytes.len() > self.bytes.len() {
                return Err("admitted output exhausted".into());
            }
            self.bytes[..value.bytes.len()].copy_from_slice(&value.bytes);
            self.length = value.bytes.len();
            self.calls += 1;
        }
        Ok(())
    }
}

pub fn measure<T>(operation: impl FnOnce() -> T) -> (T, usize, usize) {
    CALLS.with(|v| v.set(0));
    BYTES.with(|v| v.set(0));
    ENABLED.with(|v| v.set(true));
    let result = operation();
    stop();
    (result, CALLS.with(Cell::get), BYTES.with(Cell::get))
}
