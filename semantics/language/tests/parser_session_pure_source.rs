//! Actual fixed Source component gates; public Session acceptance is additional.
extern crate alloc;
#[path = "../src/parser_session_pure_source.rs"]
mod custody;
use conduit_language::*;
use conduit_plot::rust_binding::*;
use custody::*;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::RefCell,
    rc::Rc,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
struct Probe;
static TRACK: AtomicBool = AtomicBool::new(false);
static REQUESTS: AtomicUsize = AtomicUsize::new(0);
static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
unsafe impl GlobalAlloc for Probe {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if TRACK.load(Ordering::Relaxed) {
            REQUESTS.fetch_add(layout.size(), Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, old: Layout, size: usize) -> *mut u8 {
        if TRACK.load(Ordering::Relaxed) {
            REQUESTS.fetch_add(size, Ordering::Relaxed);
        }
        unsafe { System.realloc(ptr, old, size) }
    }
}
#[global_allocator]
static ALLOCATOR: Probe = Probe;
const PROGRAM: &str = include_str!(concat!(env!("OUT_DIR"), "/parser_session_complete.hex"));
fn limits() -> PureSourceLimits {
    PureSourceLimits {
        maximum_invocations: 2,
        maximum_history_retained_bytes: 2 * 1024 * 1024,
        maximum_input_bytes: 262144,
        maximum_output_bytes: 262144,
        maximum_program_bytes: 16 * 1024 * 1024,
        maximum_program_decode_bytes: 64 * 1024 * 1024,
        maximum_type_encoding_bytes: 262144,
        maximum_evaluator_retained_bytes: 64 * 1024 * 1024,
        maximum_evaluator_preparation_bytes: 128 * 1024 * 1024,
        maximum_active_native_bytes: 1024 * 1024 * 1024,
        other_existing_bytes: 0,
        maximum_combined_preparation_bytes: 2 * 1024 * 1024 * 1024,
    }
}
fn family() -> Rc<RefCell<PreparedNativeFamily>> {
    Rc::new(RefCell::new(
        PreparedNativeFamily::prepare(
            &[
                LanguageParserState::PREPARED_DESCRIPTOR,
                LanguageParserCompletionObservation::PREPARED_DESCRIPTOR,
            ],
            PreparedNativeFamilyLimits {
                maximum_types: 64,
                maximum_laws_per_type: 128,
                maximum_input_bytes: 262144,
                maximum_retained_bytes: 1024 * 1024 * 1024,
                maximum_preparation_peak_bytes: 1024 * 1024 * 1024,
                maximum_conversion_requested_bytes: 1024 * 1024 * 1024,
            },
        )
        .unwrap(),
    ))
}
fn state(complete: bool) -> Vec<u8> {
    let basis = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("fixture/analysis".into()).unwrap(),
        LanguageTextRevisionId::new("fixture/revision".into()).unwrap(),
        LanguageTextId::new("fixture/text".into()).unwrap(),
    )
    .unwrap();
    let dep = LanguageParserRelation::new(
        LanguageUniversalDependencyRelation::Dep,
        LanguageParserSubtype::new("".into()).unwrap(),
    )
    .unwrap();
    let root = LanguageParserRelation::new(
        LanguageUniversalDependencyRelation::Root,
        LanguageParserSubtype::new("".into()).unwrap(),
    )
    .unwrap();
    LanguageParserState::new(
        basis,
        0,
        1,
        [if complete { 4 } else { 5 }, 5, 5, 5, 4],
        if complete { root } else { dep.clone() },
        dep.clone(),
        dep.clone(),
        dep,
        [4; 5],
        1,
        u64::from(complete),
    )
    .unwrap()
    .encode()
    .unwrap()
}
fn frames() -> PureSourceFrames {
    PureSourceFrames {
        input: Vec::with_capacity(262144),
        output: Vec::with_capacity(262144),
        intermediates: (1..PROGRAM.lines().count())
            .map(|_| Vec::with_capacity(262144))
            .collect(),
    }
}
#[test]
fn fixed_source_preserves_whole_frames_and_replays_after_cancellation() {
    let _lock = LOCK.lock().unwrap();
    let family = family();
    let mut port = PreparedParserPureSource::prepare::<
        LanguageParserState,
        LanguageParserCompletionObservation,
    >(PROGRAM, family.clone(), family.clone(), limits())
    .unwrap();
    assert_eq!(port.receipt.original_program_artifact_bytes, PROGRAM.len());
    assert!(port.receipt.evaluator.retained_heap_bytes_bound > 0);
    assert!(port.receipt.active_native_bytes_bound > 0);
    eprintln!("pure Source receipt: {:?}", port.receipt);
    let reference: Vec<_> = PROGRAM
        .lines()
        .map(|line| conduit_plot::PortableExpressionProgram::from_canonical_hex(line).unwrap())
        .collect();
    let mut histories = Vec::new();
    for (index, complete) in [false, true].into_iter().enumerate() {
        let input = state(complete);
        let mut expected = input.clone();
        for program in &reference {
            expected = program.evaluate(&expected).unwrap();
        }
        let history = port.execute(&input, frames()).unwrap();
        assert_eq!(history.ordinal, index as u64);
        assert_eq!(history.input, input);
        assert_eq!(history.output, expected);
        assert_eq!(
            *family
                .borrow_mut()
                .decode::<LanguageParserCompletionObservation>(&history.output)
                .unwrap()
                .complete(),
            complete
        );
        port.replay(&history).unwrap();
        histories.push(history);
    }
    port.cancel();
    for history in &histories {
        port.replay(history).unwrap();
    }
    assert!(matches!(
        port.execute(&state(false), frames()),
        Err(PureSourceRefusal::Closed)
    ));
}
#[test]
fn native_valid_wrong_historical_output_refuses_and_closes_ingress() {
    let _lock = LOCK.lock().unwrap();
    let family = family();
    let mut port = PreparedParserPureSource::prepare::<
        LanguageParserState,
        LanguageParserCompletionObservation,
    >(PROGRAM, family.clone(), family.clone(), limits())
    .unwrap();
    let input = state(false);
    let mut history = port.execute(&input, frames()).unwrap();
    history.output = LanguageParserCompletionObservation::new(true)
        .unwrap()
        .encode()
        .unwrap();
    family
        .borrow_mut()
        .decode::<LanguageParserCompletionObservation>(&history.output)
        .unwrap();
    assert!(matches!(
        port.replay(&history),
        Err(PureSourceRefusal::Output)
    ));
    assert!(matches!(
        port.execute(&input, frames()),
        Err(PureSourceRefusal::Closed)
    ));
}
#[test]
fn whole_reservation_and_both_descriptors_refuse_before_first_allocation() {
    let _lock = LOCK.lock().unwrap();
    let family = family();
    for selected in [
        PureSourceLimits {
            maximum_combined_preparation_bytes: 0,
            ..limits()
        },
        PureSourceLimits {
            maximum_active_native_bytes: 0,
            ..limits()
        },
    ] {
        REQUESTS.store(0, Ordering::Relaxed);
        TRACK.store(true, Ordering::Relaxed);
        let result = PreparedParserPureSource::prepare::<
            LanguageParserState,
            LanguageParserCompletionObservation,
        >(PROGRAM, family.clone(), family.clone(), selected);
        TRACK.store(false, Ordering::Relaxed);
        assert!(matches!(result, Err(PureSourceRefusal::Pressure)));
        assert_eq!(REQUESTS.load(Ordering::Relaxed), 0);
    }
    let tiny = Rc::new(RefCell::new(
        PreparedNativeFamily::prepare(
            &[LanguageParserCompletionObservation::PREPARED_DESCRIPTOR],
            PreparedNativeFamilyLimits {
                maximum_types: 64,
                maximum_laws_per_type: 128,
                maximum_input_bytes: 262144,
                maximum_retained_bytes: 1024 * 1024 * 1024,
                maximum_preparation_peak_bytes: 1024 * 1024 * 1024,
                maximum_conversion_requested_bytes: 1024 * 1024 * 1024,
            },
        )
        .unwrap(),
    ));
    REQUESTS.store(0, Ordering::Relaxed);
    TRACK.store(true, Ordering::Relaxed);
    let result = PreparedParserPureSource::prepare::<
        LanguageParserState,
        LanguageParserCompletionObservation,
    >(PROGRAM, tiny.clone(), tiny, limits());
    TRACK.store(false, Ordering::Relaxed);
    assert!(matches!(result, Err(PureSourceRefusal::Descriptor)));
    assert_eq!(REQUESTS.load(Ordering::Relaxed), 0);
}

#[test]
fn complete_chain_preparation_is_bounded_and_one_under_is_preallocation_refusal() {
    let _lock = LOCK.lock().unwrap();
    let family = family();
    REQUESTS.store(0, Ordering::Relaxed);
    TRACK.store(true, Ordering::Relaxed);
    let reserved = PreparedParserPureSource::reservation::<
        LanguageParserState,
        LanguageParserCompletionObservation,
    >(PROGRAM, &family, &family, limits());
    TRACK.store(false, Ordering::Relaxed);
    let reserved = reserved.unwrap();
    assert_eq!(REQUESTS.load(Ordering::Relaxed), 0);
    assert_eq!(reserved.programs, 2);

    REQUESTS.store(0, Ordering::Relaxed);
    TRACK.store(true, Ordering::Relaxed);
    let result = PreparedParserPureSource::prepare::<
        LanguageParserState,
        LanguageParserCompletionObservation,
    >(PROGRAM, family.clone(), family.clone(), limits());
    TRACK.store(false, Ordering::Relaxed);
    let port = result.unwrap();
    let requested = REQUESTS.load(Ordering::Relaxed);
    assert_eq!(
        reserved.combined_preparation_bytes_bound,
        port.receipt.combined_preparation_bytes_bound
    );
    assert!(requested <= port.receipt.combined_preparation_bytes_bound);
    let mut one_under = limits();
    one_under.maximum_combined_preparation_bytes =
        port.receipt.combined_preparation_bytes_bound - 1;
    REQUESTS.store(0, Ordering::Relaxed);
    TRACK.store(true, Ordering::Relaxed);
    let refused = PreparedParserPureSource::prepare::<
        LanguageParserState,
        LanguageParserCompletionObservation,
    >(PROGRAM, family.clone(), family.clone(), one_under);
    TRACK.store(false, Ordering::Relaxed);
    assert!(matches!(refused, Err(PureSourceRefusal::Pressure)));
    assert_eq!(REQUESTS.load(Ordering::Relaxed), 0);
    eprintln!("complete chain preparation requested={requested}");
}
#[test]
fn original_intermediate_output_is_retained_and_wrong_frame_refuses() {
    let _lock = LOCK.lock().unwrap();
    let family = family();
    let mut port = PreparedParserPureSource::prepare::<
        LanguageParserState,
        LanguageParserCompletionObservation,
    >(PROGRAM, family.clone(), family, limits())
    .unwrap();
    let mut history = port.execute(&state(false), frames()).unwrap();
    assert_eq!(history.intermediates.len(), 1);
    let first = conduit_plot::PortableExpressionProgram::from_canonical_hex(
        PROGRAM.lines().next().unwrap(),
    )
    .unwrap();
    assert_eq!(
        history.intermediates[0],
        first.evaluate(&history.input).unwrap()
    );
    history.intermediates[0].clear();
    assert!(matches!(
        port.replay(&history),
        Err(PureSourceRefusal::Output)
    ));
    assert!(matches!(
        port.execute(&state(false), frames()),
        Err(PureSourceRefusal::Closed)
    ));
}
#[test]
fn malformed_native_ingress_and_history_pressure_close_the_consumed_port() {
    let _lock = LOCK.lock().unwrap();
    let family = family();
    let mut port = PreparedParserPureSource::prepare::<
        LanguageParserState,
        LanguageParserCompletionObservation,
    >(PROGRAM, family.clone(), family.clone(), limits())
    .unwrap();
    match port.execute(&[], frames()) {
        Err(PureSourceRefusal::Native(reason)) => {
            let _ = reason;
        }
        _ => panic!("malformed original Native ingress must refuse"),
    }
    assert!(matches!(
        port.execute(&state(false), frames()),
        Err(PureSourceRefusal::Closed)
    ));
    let mut small = limits();
    small.maximum_history_retained_bytes = 1;
    let mut port = PreparedParserPureSource::prepare::<
        LanguageParserState,
        LanguageParserCompletionObservation,
    >(PROGRAM, family.clone(), family, small)
    .unwrap();
    let query = state(false);
    let pool = frames();
    REQUESTS.store(0, Ordering::Relaxed);
    TRACK.store(true, Ordering::Relaxed);
    let result = port.execute(&query, pool);
    TRACK.store(false, Ordering::Relaxed);
    assert!(matches!(result, Err(PureSourceRefusal::Pressure)));
    assert_eq!(REQUESTS.load(Ordering::Relaxed), 0);
    assert!(matches!(
        port.execute(&query, frames()),
        Err(PureSourceRefusal::Closed)
    ));
}
