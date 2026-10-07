#![cfg(all(feature = "kernel", feature = "semantic-bindings"))]
#[path = "common/playback_fixture.rs"]
mod fixture;
#[path = "common/playback_graph.rs"]
#[allow(dead_code)]
mod graph;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    rc::Rc,
};
struct CountingAllocator;
std::thread_local! {
    static COUNT: Cell<Option<usize>> = const { Cell::new(None) };
}
fn allocated() {
    COUNT.with(|count| {
        if let Some(value) = count.get() {
            count.set(Some(value + 1));
        }
    });
}
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        allocated();
        // SAFETY: pass the caller's allocation contract unchanged to System.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: GlobalAlloc receives the matching pointer/layout pair.
        unsafe { System.dealloc(pointer, layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        allocated();
        // SAFETY: delegate the caller's reallocation contract unchanged.
        unsafe { System.realloc(pointer, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;
#[test]
fn admitted_native_tape_plan_play_has_no_heap_growth() {
    let fixture = fixture::fixture("Hello Travis", "r1");
    let linguistic = fixture.linguistic();
    let pitch = conduit_speech::pitch_trajectory::prepare_utterance_pitch(
        &fixture.source,
        &[conduit_speech::pitch_trajectory::OfferedSegmentPitch {
            event: 0,
            admission: linguistic.accepted().pitch(),
        }],
    )
    .unwrap();
    let realized = fixture.realized();
    let tape = conduit_speech::playback_basis::prepare_speech_playback_tape(
        &realized,
        &pitch,
        &[conduit_speech::playback_basis::PlaybackLinguisticBinding {
            event: 0,
            admitted: &linguistic,
        }],
        7,
    )
    .unwrap();
    let mut scheduler = graph::scheduler(&tape, Rc::new(Cell::new(false)));
    let capacities = scheduler.values().allocation_capacities();
    COUNT.with(|count| count.set(Some(0)));
    let result = scheduler.run(20000);
    let allocations = COUNT.with(|count| count.replace(None).unwrap());
    result.unwrap();
    assert_eq!(allocations, 0, "only preparation may allocate");
    assert_eq!(scheduler.values().allocation_capacities(), capacities);
}
