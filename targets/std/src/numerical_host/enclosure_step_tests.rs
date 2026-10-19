use super::*;
use conduit_kernel::ValueRef;
#[cfg(target_os = "linux")]
#[path = "enclosure_cost_tests.rs"]
mod cost;
fn requirements() -> MathematicalEnclosureRequirements {
    let mut w = ExactNumericWork::new(ENCLOSURE_WORK);
    MathematicalEnclosureRequirements {
        maximum_width: ExactRational::ratio(1, 100_000_000_000_000_000, &mut w).unwrap(),
        maximum_leaf_width: ExactRational::ratio(1, 100_000_000_000_000_000_000, &mut w).unwrap(),
        maximum_constant_terms: 32,
        maximum_root_refinements: 64,
        maximum_scratch_payload_bytes: 131072,
        maximum_retained_payload_bytes: 131072,
    }
}
fn pi() -> Vec<u8> {
    let value =
        MathematicalValue::parse_literal("pi", &mut ExactNumericWork::new(ENCLOSURE_WORK)).unwrap();
    let mut bytes = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let n = value.encode(&mut bytes).unwrap();
    bytes.truncate(n);
    bytes
}
fn frame(bytes: &[u8], ready: bool, fuel: u16) -> StepIo<1> {
    StepIo::test_frame(
        [Some(ValueRef {
            slot: 0,
            generation: 1,
            byte_len: bytes.len() as u32,
        })],
        [false],
        [ready.then_some(conduit_semantic_catalog::MATHEMATICAL_ENCLOSE_MAX_BYTES)],
        None,
        fuel,
    )
}
#[test]
fn pressure_fuel_cancellation_and_closed_input_preserve_transactional_output_custody() {
    let bytes = pi();
    let input = StepInputBytes::test_frame([Some(bytes.as_slice())], None);
    let mut back = MathematicalEnclosureBack::new(requirements()).unwrap();
    let capacity = back.retained_capacity_bytes();
    let mut blocked = frame(&bytes, false, 8);
    assert_eq!(back.step(&mut blocked, &input), StepOutcome::Await);
    assert!(!blocked.test_consumed(PortId(0)));
    let mut unfueled = frame(&bytes, true, 0);
    assert_eq!(back.step(&mut unfueled, &input), fail(3));
    assert!(!unfueled.test_consumed(PortId(0)));
    let guard = crate::allocation_probe::begin();
    let mut ready = frame(&bytes, true, 8);
    assert_eq!(back.step(&mut ready, &input), StepOutcome::Progress);
    assert!(ready.test_consumed(PortId(0)));
    assert!(
        <MathematicalEnclosureBack as StepBack<1>>::prepared_output(&back, PortId(0)).is_some()
    );
    <MathematicalEnclosureBack as StepBack<1>>::cancel(&mut back);
    assert!(
        <MathematicalEnclosureBack as StepBack<1>>::prepared_output(&back, PortId(0)).is_none()
    );
    assert_eq!(
        back.step(&mut frame(&bytes, true, 8), &input),
        StepOutcome::Complete
    );
    assert_eq!(guard.finish(), 0);
    assert_eq!(back.retained_capacity_bytes(), capacity);
    let mut empty = MathematicalEnclosureBack::new(requirements()).unwrap();
    let mut unfueled = StepIo::test_frame([None], [true], [None], None, 0);
    assert_eq!(
        empty.step(&mut unfueled, &StepInputBytes::test_frame([None], None)),
        fail(3)
    );
    assert!(!unfueled.test_consumed_closed(PortId(0)));
    let mut closed = StepIo::test_frame([None], [true], [None], None, 8);
    assert_eq!(
        empty.step(&mut closed, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Complete
    );
    assert!(closed.test_consumed_closed(PortId(0)));
}
#[test]
fn repeated_evaluation_and_malformed_refusal_recover_without_growth_or_stale_emission() {
    let mut back = MathematicalEnclosureBack::new(requirements()).unwrap();
    let bytes = pi();
    let capacity = back.retained_capacity_bytes();
    let guard = crate::allocation_probe::begin();
    for _ in 0..4 {
        back.evaluate(&bytes).unwrap();
        assert!(back.evaluate(&[255, 0, 0, 0]).is_err());
        assert!(
            <MathematicalEnclosureBack as StepBack<1>>::prepared_output(&back, PortId(0)).is_none()
        );
    }
    back.evaluate(&bytes).unwrap();
    assert_eq!(guard.finish(), 0);
    assert_eq!(back.retained_capacity_bytes(), capacity);
}
