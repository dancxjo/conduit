//! Transactional pressure, cancellation and terminal input custody.
use super::*;
use conduit_kernel::ValueRef;
fn operation() -> NearQuantityBack {
    NearQuantityBack::new(
        Quantity::parse_plot_literal("0.0001m").unwrap(),
        NearQuantityAlgorithm::Binary32,
    )
    .unwrap()
}
fn frame(bytes: &[u8], ready: bool, fuel: u16) -> StepIo<1> {
    StepIo::test_frame(
        [Some(ValueRef {
            slot: 0,
            generation: 1,
            byte_len: bytes.len() as u32,
        })],
        [false],
        [ready.then_some(NUMERICAL_NEAR_MAX_BYTES)],
        None,
        fuel,
    )
}
#[test]
fn output_pressure_and_exhausted_fuel_preserve_input_before_projection() {
    let bytes = Quantity::parse_plot_literal("1m").unwrap().encode();
    let mut back = operation();
    let capacity = back.retained_capacity_bytes();
    let before = back.state.encoding.encoded().to_vec();
    let input = StepInputBytes::test_frame([Some(bytes.as_slice())], None);
    let mut blocked = frame(&bytes, false, 8);
    assert_eq!(back.step(&mut blocked, &input), StepOutcome::Await);
    assert!(!blocked.test_consumed(PortId(0)));
    assert_eq!(back.state.encoding.encoded(), before);
    let mut no_fuel = frame(&bytes, true, 0);
    assert_eq!(back.step(&mut no_fuel, &input), fail(NearRefusal::Work));
    assert!(!no_fuel.test_consumed(PortId(0)));
    assert_eq!(back.state.encoding.encoded(), before);
    let mut ready = frame(&bytes, true, 8);
    assert_eq!(back.step(&mut ready, &input), StepOutcome::Progress);
    assert!(ready.test_consumed(PortId(0)));
    assert!(<NearQuantityBack as StepBack<1>>::prepared_output(&back, PortId(0)).is_some());
    <NearQuantityBack as StepBack<1>>::step_committed(&mut back);
    let mut again = frame(&bytes, true, 8);
    assert_eq!(back.step(&mut again, &input), StepOutcome::Complete);
    assert!(!again.test_consumed(PortId(0)));
    assert!(<NearQuantityBack as StepBack<1>>::prepared_output(&back, PortId(0)).is_none());
    assert_eq!(back.retained_capacity_bytes(), capacity);
}
#[test]
fn cancellation_discards_staged_output_and_empty_closed_input_completes() {
    let bytes = Quantity::parse_plot_literal("1m").unwrap().encode();
    let input = StepInputBytes::test_frame([Some(bytes.as_slice())], None);
    let mut back = operation();
    assert_eq!(
        back.step(&mut frame(&bytes, true, 8), &input),
        StepOutcome::Progress
    );
    <NearQuantityBack as StepBack<1>>::cancel(&mut back);
    assert!(<NearQuantityBack as StepBack<1>>::prepared_output(&back, PortId(0)).is_none());
    let mut again = frame(&bytes, true, 8);
    assert_eq!(back.step(&mut again, &input), StepOutcome::Complete);
    assert!(!again.test_consumed(PortId(0)));
    let mut empty = operation();
    let mut no_fuel = StepIo::test_frame([None], [true], [None], None, 0);
    assert_eq!(
        empty.step(&mut no_fuel, &StepInputBytes::test_frame([None], None)),
        fail(NearRefusal::Work)
    );
    assert!(!no_fuel.test_consumed_closed(PortId(0)));
    let mut closed = StepIo::test_frame([None], [true], [None], None, 8);
    assert_eq!(
        empty.step(&mut closed, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Complete
    );
    assert!(closed.test_consumed_closed(PortId(0)));
    assert!(<NearQuantityBack as StepBack<1>>::prepared_output(&empty, PortId(0)).is_none());
}
