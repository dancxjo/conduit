//! Atomic one-shot Value pairing uses existing canonical framing and no heap growth.
use conduit_composite::ValuePairBack;
use conduit_core::{
    kind_id, validate_canonical_structured_value, StructuredInfoType, StructuredInfoValue,
};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    PortId, ValueRef,
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
thread_local! {static ENABLED:Cell<bool>=const{Cell::new(false)};static CALLS:Cell<usize>=const{Cell::new(0)};}
struct Counted;
fn record() {
    let _ = ENABLED.try_with(|v| {
        if v.get() {
            let _ = CALLS.try_with(|v| v.set(v.get() + 1));
        }
    });
}
unsafe impl GlobalAlloc for Counted {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        record();
        unsafe { System.alloc(l) }
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        record();
        unsafe { System.alloc_zeroed(l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        record();
        unsafe { System.realloc(p, l, n) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
}
#[global_allocator]
static ALLOCATOR: Counted = Counted;
fn measured(f: impl FnOnce()) {
    CALLS.with(|v| v.set(0));
    ENABLED.with(|v| v.set(true));
    f();
    ENABLED.with(|v| v.set(false));
    assert_eq!(CALLS.with(Cell::get), 0);
}
fn frame<'a>(
    left: Option<&'a [u8]>,
    right: Option<&'a [u8]>,
    closed: [bool; 2],
    maximum: Option<u32>,
) -> (StepIo<2>, StepInputBytes<'a, 2>) {
    let reference = |slot, bytes: &[u8]| ValueRef {
        slot,
        generation: 1,
        byte_len: bytes.len() as u32,
    };
    (
        StepIo::test_frame(
            [
                left.map(|b| reference(1, b)),
                right.map(|b| reference(2, b)),
            ],
            closed,
            [maximum, None],
            None,
            8,
        ),
        StepInputBytes::test_frame([left, right], None),
    )
}
#[test]
fn both_values_pair_once_even_when_producers_are_closed_and_output_was_pressured() {
    let left_type = StructuredInfoType::leaf(kind_id("value/i32")).unwrap();
    let right_type = StructuredInfoType::leaf(kind_id("value/u8")).unwrap();
    let mut back = ValuePairBack::prepare(left_type, 4, right_type, 1).unwrap();
    let maximum = back.output_maximum();
    let left = (-7_i32).to_le_bytes();
    let right = [42];
    measured(|| {
        let (mut io, bytes) = frame(Some(&left), None, [false, false], Some(maximum));
        assert_eq!(back.step(&mut io, &bytes), StepOutcome::Await);
        assert!(!io.test_consumed(PortId(0)));
        let (mut io, bytes) = frame(Some(&left), Some(&right), [true, true], None);
        assert_eq!(back.step(&mut io, &bytes), StepOutcome::Await);
        assert!(!io.test_consumed(PortId(0)));
        assert!(!io.test_consumed(PortId(1)));
        let (mut io, bytes) = frame(Some(&left), Some(&right), [true, true], Some(maximum));
        assert_eq!(back.step(&mut io, &bytes), StepOutcome::Progress);
        assert!(io.test_consumed(PortId(0)));
        assert!(io.test_consumed(PortId(1)));
        let encoded = <ValuePairBack as StepBack<2>>::prepared_output(&back, PortId(0)).unwrap();
        let view = validate_canonical_structured_value(encoded).unwrap();
        assert_eq!(
            view.record_field("item-00000")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/i32")
                .unwrap(),
            left
        );
        assert_eq!(
            view.record_field("item-00001")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/u8")
                .unwrap(),
            right
        );
        // A rejected transaction must not complete the operation.
        let (mut io, bytes) = frame(Some(&left), Some(&right), [true, true], None);
        assert_eq!(back.step(&mut io, &bytes), StepOutcome::Await);
        assert!(<ValuePairBack as StepBack<2>>::prepared_output(&back, PortId(0)).is_none());
        let (mut io, bytes) = frame(Some(&left), Some(&right), [true, true], Some(maximum));
        assert_eq!(back.step(&mut io, &bytes), StepOutcome::Progress);
        <ValuePairBack as StepBack<2>>::step_committed(&mut back);
        let (mut io, bytes) = frame(Some(&left), Some(&right), [true, true], Some(maximum));
        assert_eq!(back.step(&mut io, &bytes), StepOutcome::Complete);
        assert!(!io.test_consumed(PortId(0)));
        assert!(!io.test_consumed(PortId(1)));
    });
}
#[test]
fn exact_nominal_type_is_retained_and_foreign_nominal_or_missing_value_refuses() {
    let scalar = StructuredInfoType::leaf(kind_id("value/i32")).unwrap();
    let expected =
        StructuredInfoType::nominal(kind_id("fixture/expected"), scalar.clone()).unwrap();
    let foreign = StructuredInfoType::nominal(kind_id("fixture/foreign"), scalar.clone()).unwrap();
    let value = |ty| {
        StructuredInfoValue::nominal(
            ty,
            StructuredInfoValue::leaf(scalar.clone(), 5_i32.to_le_bytes().to_vec()).unwrap(),
        )
        .unwrap()
        .canonical_bytes()
        .unwrap()
    };
    let good = value(expected.clone());
    let bad = value(foreign);
    let expected_header = expected.canonical_bytes().unwrap();
    let right = [9];
    let mut back = ValuePairBack::prepare(
        expected,
        good.len() as u32,
        StructuredInfoType::leaf(kind_id("value/u8")).unwrap(),
        1,
    )
    .unwrap();
    let maximum = back.output_maximum();
    measured(|| {
        let (mut io, bytes) = frame(Some(&bad), Some(&right), [false, false], Some(maximum));
        assert!(matches!(back.step(&mut io, &bytes), StepOutcome::Fail(_)));
        assert!(!io.test_consumed(PortId(0)));
        let (mut io, bytes) = frame(Some(&good), None, [false, true], Some(maximum));
        assert!(matches!(back.step(&mut io, &bytes), StepOutcome::Fail(_)));
        let (mut io, bytes) = frame(Some(&good), Some(&right), [true, true], Some(maximum));
        assert_eq!(back.step(&mut io, &bytes), StepOutcome::Progress);
        let view = validate_canonical_structured_value(
            <ValuePairBack as StepBack<2>>::prepared_output(&back, PortId(0)).unwrap(),
        )
        .unwrap();
        assert_eq!(
            view.record_field("item-00000")
                .unwrap()
                .unwrap()
                .type_bytes(),
            expected_header
        );
    });
    let mut cancelled = ValuePairBack::prepare(
        StructuredInfoType::leaf(kind_id("value/i32")).unwrap(),
        4,
        StructuredInfoType::leaf(kind_id("value/u8")).unwrap(),
        1,
    )
    .unwrap();
    <ValuePairBack as StepBack<2>>::cancel(&mut cancelled);
    let cancelled_input = 5_i32.to_le_bytes();
    let (mut io, bytes) = frame(
        Some(&cancelled_input),
        Some(&right),
        [false, false],
        Some(maximum),
    );
    assert_eq!(cancelled.step(&mut io, &bytes), StepOutcome::Complete);
    assert!(!io.test_consumed(PortId(0)));
}
