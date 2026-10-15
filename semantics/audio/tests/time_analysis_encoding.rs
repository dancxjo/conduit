use conduit_audio::*;
use conduit_plot::rust_binding::NativeRustBinding;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
thread_local! { static ALLOCATIONS: Cell<(bool, usize)> = const { Cell::new((false, 0)) }; }
struct Counted;
fn count() {
    let _ = ALLOCATIONS.try_with(|c| {
        let (enabled, n) = c.get();
        if enabled {
            c.set((true, n + 1));
        }
    });
}
unsafe impl GlobalAlloc for Counted {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count();
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count();
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count();
        unsafe { System.realloc(ptr, layout, size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: Counted = Counted;
fn input(frames: u16, clock: u64, rate: u32, start: u64) -> Vec<u8> {
    let payload: Vec<_> = (0..frames)
        .flat_map(|i| (if i % 2 == 0 { 0.25_f32 } else { -0.5 }).to_le_bytes())
        .collect();
    PcmFrameHeader::new(
        PcmSampleRepresentation::Float32LittleEndian,
        rate,
        PcmChannelLayout::Mono,
        frames,
        clock,
        start,
        false,
    )
    .unwrap()
    .encode_frame(&payload)
    .unwrap()
}
#[test]
fn prepared_native_slots_match_generated_binding_without_runtime_allocations() {
    for frames in [2, 1024, 1025, 2048] {
        let mut encoder = PreparedPcmTimeObservationEncoder::prepare(frames).unwrap();
        let heap = encoder.prepared_heap_bytes();
        assert_eq!(
            heap,
            encoder.maximum_output_bytes() + usize::from(frames) * core::mem::size_of::<usize>()
        );
        for (clock, rate, start) in [(1, 8000, 0), (77, 48000, 1234567)] {
            let input = input(frames, clock, rate, start);
            let analysis =
                PreparedPcmTimeAnalysis::prepare(&input, clock, rate, 0, frames).unwrap();
            let expected = analysis.native_observation().unwrap().encode().unwrap();
            ALLOCATIONS.with(|c| c.set((true, 0)));
            let analysis =
                PreparedPcmTimeAnalysis::prepare(&input, clock, rate, 0, frames).unwrap();
            let actual = encoder.encode(&analysis).unwrap();
            let allocations = ALLOCATIONS.with(|c| {
                let (_, n) = c.replace((false, 0));
                n
            });
            assert_eq!(
                allocations, 0,
                "{frames}-sample preparation and output patch must be finite"
            );
            assert_eq!(actual, expected);
            assert_eq!(
                AudioTimeAnalysisObservation::decode(actual)
                    .unwrap()
                    .source_digest()
                    .get(),
                &analysis.source_digest()
            );
            assert_eq!(heap, encoder.prepared_heap_bytes());
            assert_eq!(encoder.maximum_output_bytes(), expected.len());
        }
    }
}
#[test]
fn different_window_shape_refuses_without_overwriting_prior_output() {
    let input = input(3, 1, 8000, 0);
    let mut encoder = PreparedPcmTimeObservationEncoder::prepare(2).unwrap();
    let original = encoder.encoded().to_vec();
    let wrong = PreparedPcmTimeAnalysis::prepare(&input, 1, 8000, 0, 3).unwrap();
    assert!(matches!(
        encoder.encode(&wrong),
        Err(PcmTimeEncodingRefusal::WrongFrameCount)
    ));
    assert_eq!(encoder.encoded(), original);
}
