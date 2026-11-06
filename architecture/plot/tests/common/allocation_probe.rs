use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

#[derive(Clone, Copy, Default, Debug)]
pub struct Observation {
    pub allocations: usize,
    pub allocation_attempts: usize,
    pub refused_allocations: usize,
    pub reallocations: usize,
    pub deallocations: usize,
    pub peak_bytes: usize,
    pub requested_bytes: usize,
    live_bytes: usize,
}
thread_local! {
    static TRACKING: Cell<bool> = const { Cell::new(false) };
    static REFUSE_AT: Cell<usize> = const { Cell::new(0) };
    static OBSERVATION: Cell<Observation> = const { Cell::new(Observation { allocations: 0, allocation_attempts: 0, refused_allocations: 0, reallocations: 0, deallocations: 0, peak_bytes: 0, requested_bytes: 0, live_bytes: 0 }) };
}
pub struct Allocator;
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if refuse() {
            return core::ptr::null_mut();
        }
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            update(layout.size(), 0, false);
        }
        pointer
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if refuse() {
            return core::ptr::null_mut();
        }
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            update(layout.size(), 0, false);
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        update(0, layout.size(), false);
        unsafe {
            System.dealloc(pointer, layout);
        }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if refuse() {
            return core::ptr::null_mut();
        }
        let next = unsafe { System.realloc(pointer, layout, size) };
        if !next.is_null() {
            update(size, layout.size(), true);
        }
        next
    }
}
// Only this test thread's explicitly scoped request is refused. Reallocation
// refusal leaves the original pointer owned by its caller, as GlobalAlloc requires.
fn refuse() -> bool {
    TRACKING
        .try_with(|tracking| {
            if !tracking.get() {
                return false;
            }
            OBSERVATION.with(|observation| {
                let mut state = observation.get();
                state.allocation_attempts += 1;
                let refused = REFUSE_AT.with(|at| at.get() == state.allocation_attempts);
                state.refused_allocations += usize::from(refused);
                observation.set(state);
                refused
            })
        })
        .unwrap_or(false)
}
fn update(added: usize, removed: usize, reallocation: bool) {
    let _ = TRACKING.try_with(|tracking| {
        if tracking.get() {
            OBSERVATION.with(|observation| {
                let mut state = observation.get();
                if reallocation {
                    state.reallocations += 1;
                } else if added != 0 {
                    state.allocations += 1;
                } else if removed != 0 {
                    state.deallocations += 1;
                }
                state.live_bytes = state
                    .live_bytes
                    .saturating_sub(removed)
                    .saturating_add(added);
                state.requested_bytes = state.requested_bytes.saturating_add(added);
                state.peak_bytes = state.peak_bytes.max(state.live_bytes);
                observation.set(state);
            });
        }
    });
}
pub fn observe<T>(work: impl FnOnce() -> T) -> (T, Observation) {
    observe_with_refusal(0, work)
}
/// Keep a callback-delimited phase observable without counting its preparation.
/// The marker prevents ending another thread's interval; unwinding also stops it.
#[allow(dead_code)]
pub struct ObservationInterval {
    active: bool,
    thread: core::marker::PhantomData<std::rc::Rc<()>>,
}
#[allow(dead_code)]
impl ObservationInterval {
    pub fn begin() -> Self {
        TRACKING.with(|tracking| assert!(!tracking.replace(true)));
        OBSERVATION.with(|observation| observation.set(Observation::default()));
        REFUSE_AT.with(|at| at.set(0));
        Self {
            active: true,
            thread: core::marker::PhantomData,
        }
    }
    pub fn finish(mut self) -> Observation {
        self.stop();
        OBSERVATION.with(Cell::get)
    }
    fn stop(&mut self) {
        if self.active {
            TRACKING.with(|tracking| tracking.set(false));
            REFUSE_AT.with(|at| at.set(0));
            self.active = false;
        }
    }
}
impl Drop for ObservationInterval {
    fn drop(&mut self) {
        self.stop();
    }
}
/// Refuse exactly the nth allocation/reallocation, without changing other threads.
#[allow(dead_code)]
pub fn observe_refusing<T>(nth: usize, work: impl FnOnce() -> T) -> (T, Observation) {
    assert!(nth > 0);
    observe_with_refusal(nth, work)
}
fn observe_with_refusal<T>(nth: usize, work: impl FnOnce() -> T) -> (T, Observation) {
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            TRACKING.with(|tracking| tracking.set(false));
            REFUSE_AT.with(|at| at.set(0));
        }
    }
    TRACKING.with(|tracking| assert!(!tracking.replace(true)));
    OBSERVATION.with(|observation| observation.set(Observation::default()));
    REFUSE_AT.with(|at| at.set(nth));
    let guard = Guard;
    let result = work();
    drop(guard);
    let observation = OBSERVATION.with(Cell::get);
    (result, observation)
}
