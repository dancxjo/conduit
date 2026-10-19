use super::*;
use conduit_kernel::ValueRef;
fn frame(bytes: &[u8], evidence: bool, result: bool, fuel: u16) -> StepIo<2> {
    StepIo::test_frame(
        [
            Some(ValueRef {
                slot: 0,
                generation: 1,
                byte_len: bytes.len() as u32,
            }),
            None,
        ],
        [false; 2],
        [
            evidence.then_some(131500),
            result.then_some(MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES as u32),
        ],
        None,
        fuel,
    )
}
fn prepared(back: &QuantityCoordinateBack, port: u16) -> Option<&[u8]> {
    <QuantityCoordinateBack as StepBack<2>>::prepared_output(back, PortId(port))
}
#[test]
fn either_output_pressure_and_exhausted_fuel_leave_input_and_encoder_untouched() {
    let bytes = Quantity::parse_plot_literal("1m").unwrap().encode();
    let input = StepInputBytes::test_frame([Some(bytes.as_slice()), None], None);
    let mut back = QuantityCoordinateBack::new(Unit::Centimeter).unwrap();
    let retained = back.retained_capacity_bytes();
    for (evidence, result) in [(false, true), (true, false), (false, false)] {
        let mut blocked = frame(&bytes, evidence, result, 8);
        assert_eq!(back.step(&mut blocked, &input), StepOutcome::Await);
        assert!(!blocked.test_consumed(PortId(0)));
        assert_eq!(back.state.output_len, 0);
        assert!(back.state.encoding.encoded().is_empty());
    }
    let mut exhausted = frame(&bytes, true, true, 0);
    assert_eq!(back.step(&mut exhausted, &input), fail(3));
    assert!(!exhausted.test_consumed(PortId(0)));
    let guard = crate::allocation_probe::begin();
    let mut ready = frame(&bytes, true, true, 8);
    assert_eq!(back.step(&mut ready, &input), StepOutcome::Progress);
    assert!(ready.test_consumed(PortId(0)));
    assert!(prepared(&back, 0).is_some());
    assert_eq!(
        Quantity::decode(prepared(&back, 1).unwrap()).unwrap(),
        Quantity::new(100, Unit::Centimeter)
    );
    assert_eq!(guard.finish(), 0);
    <QuantityCoordinateBack as StepBack<2>>::step_committed(&mut back);
    assert!(prepared(&back, 0).is_none());
    assert!(prepared(&back, 1).is_none());
    assert_eq!(
        back.step(&mut frame(&bytes, true, true, 8), &input),
        StepOutcome::Complete
    );
    assert_eq!(back.retained_capacity_bytes(), retained);
}
#[test]
fn cancellation_discards_both_outputs_and_normal_close_has_no_value() {
    let bytes = Quantity::new(1, Unit::Meter).encode();
    let input = StepInputBytes::test_frame([Some(bytes.as_slice()), None], None);
    let mut back = QuantityCoordinateBack::new(Unit::Centimeter).unwrap();
    assert_eq!(
        back.step(&mut frame(&bytes, true, true, 8), &input),
        StepOutcome::Progress
    );
    <QuantityCoordinateBack as StepBack<2>>::cancel(&mut back);
    assert!(prepared(&back, 0).is_none());
    assert!(prepared(&back, 1).is_none());
    let mut empty = QuantityCoordinateBack::new(Unit::Centimeter).unwrap();
    let mut closed = StepIo::test_frame([None; 2], [true, false], [None; 2], None, 8);
    assert_eq!(
        empty.step(&mut closed, &StepInputBytes::test_frame([None; 2], None)),
        StepOutcome::Complete
    );
    assert!(closed.test_consumed_closed(PortId(0)));
    assert!(prepared(&empty, 0).is_none());
    assert!(prepared(&empty, 1).is_none());
}

#[test]
fn physical_refusal_is_semantic_and_close_work_exhaustion_is_mechanical() {
    let bytes = Quantity::new(1, Unit::Kelvin).encode();
    let mut back = QuantityCoordinateBack::new(Unit::Centimeter).unwrap();
    let mut io = frame(&bytes, true, true, 8);
    let StepOutcome::Abnormal { port, terminal } = back.step(
        &mut io,
        &StepInputBytes::test_frame([Some(bytes.as_slice()), None], None),
    ) else {
        panic!("semantic refusal required")
    };
    assert_eq!(port, PortId(0));
    assert_eq!(
        TerminalInfo::decode(terminal.as_slice())
            .unwrap()
            .category(),
        TerminalCategory::RefusalOrAdmission
    );
    assert!(io.test_consumed(PortId(0)));
    assert!(prepared(&back, 0).is_none());
    assert!(prepared(&back, 1).is_none());
    let mut back = QuantityCoordinateBack::new(Unit::Centimeter).unwrap();
    let mut closed = StepIo::test_frame([None; 2], [true, false], [None; 2], None, 0);
    assert_eq!(
        back.step(&mut closed, &StepInputBytes::test_frame([None; 2], None)),
        fail(3)
    );
    assert!(!closed.test_consumed_closed(PortId(0)));
}
#[test]
fn original_input_target_and_complete_frame_replay_without_allocations() {
    let input = Quantity::parse_plot_literal("1m").unwrap().encode();
    let mut back = QuantityCoordinateBack::new(Unit::Centimeter).unwrap();
    let mut verifier = PreparedQuantityCoordinateConversion::new(500000, COORDINATE_WORK).unwrap();
    let mut scratch = vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
    let mut cpe = vec![0; QUANTITY_COORDINATE_CONVERSION_EVIDENCE_LIMITS.total_bytes];
    let retained = back.retained_capacity_bytes();
    let guard = crate::allocation_probe::begin();
    back.evaluate(&input).unwrap();
    let record = validate_canonical_structured_value(back.state.encoding.encoded()).unwrap();
    let head = record
        .record_field("projection-evidence-head")
        .unwrap()
        .unwrap()
        .primitive_representation_bytes("value/bytes")
        .unwrap();
    let tail = record
        .record_field("projection-evidence-tail")
        .unwrap()
        .unwrap()
        .primitive_representation_bytes("value/bytes")
        .unwrap();
    assert_eq!(head.len(), (head.len() + tail.len()).min(65536));
    cpe[..head.len()].copy_from_slice(head);
    cpe[head.len()..head.len() + tail.len()].copy_from_slice(tail);
    let target = &back.state.output[..back.state.output_len];
    let ids = QuantityCoordinateContentIds::new(&input, target, Unit::Centimeter);
    verifier
        .verify_evidence(
            &input,
            target,
            QuantityCoordinateConversionPolicy {
                target_unit: Unit::Centimeter,
            },
            COORDINATE_WORK,
            ids.basis(),
            &cpe[..head.len() + tail.len()],
            &mut scratch,
            &mut ExactNumericWork::new(300_000_000),
        )
        .unwrap();
    assert_eq!(guard.finish(), 0);
    assert_eq!(back.retained_capacity_bytes(), retained);
}
