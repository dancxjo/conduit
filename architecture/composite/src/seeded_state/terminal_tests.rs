use super::*;
use conduit_core::{
    kind_id, StructuredFieldType, StructuredFieldValue, StructuredInfoValue, BOOL_INFO_ID,
};
use conduit_kernel::ValueRef;

#[test]
fn terminal_frame_waits_without_allocation_then_emits_once_before_normal_completion() {
    let boolean = StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap();
    let schema = StructuredInfoType::record(
        kind_id("fixture/terminal-state@1"),
        vec![StructuredFieldType::new("terminal", boolean.clone()).unwrap()],
    )
    .unwrap();
    let bytes = StructuredInfoValue::record(
        schema.clone(),
        vec![StructuredFieldValue::new(
            "terminal",
            StructuredInfoValue::leaf(boolean, vec![1]).unwrap(),
        )
        .unwrap()],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let contract =
        CheckedValueContract::new(schema.profile().unwrap().value_kind().clone(), 1024, vec![])
            .unwrap();
    let mut back = SeededStateBack::prepare_until(&contract, &schema).unwrap();
    let value = ValueRef {
        slot: 1,
        generation: 1,
        byte_len: bytes.len() as u32,
    };
    let allocations = crate::test_support::allocation::allocations_during(|| {
        for _ in 0..1000 {
            let mut io = StepIo::test_frame([Some(value), None], [false; 2], [None; 2], None, 16);
            assert_eq!(
                back.step(
                    &mut io,
                    &StepInputBytes::test_frame([Some(&bytes), None], None)
                ),
                StepOutcome::Await
            );
            assert!(!io.test_consumed(PortId(0)));
            assert!(!back.terminal);
        }
        let mut io = StepIo::test_frame(
            [Some(value), None],
            [false; 2],
            [Some(1024), None],
            None,
            16,
        );
        assert_eq!(
            back.step(
                &mut io,
                &StepInputBytes::test_frame([Some(&bytes), None], None)
            ),
            StepOutcome::Progress
        );
        assert_eq!(io.test_output(PortId(0)), Some(value));
        assert!(!back.terminal);
        <SeededStateBack as StepBack<2>>::step_committed(&mut back);
        let mut io = StepIo::test_frame([None; 2], [false; 2], [None; 2], None, 16);
        assert_eq!(
            back.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
            StepOutcome::Complete
        );
        assert!(io.test_discards().contains(&Some(value)));
        assert_eq!(io.test_output(PortId(0)), None);
    });
    assert_eq!(allocations, 0);
}

#[test]
fn terminal_state_requires_an_exact_record_boolean() {
    let schema = StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap();
    let contract = CheckedValueContract::new(kind_id(BOOL_INFO_ID), 1, vec![]).unwrap();
    assert!(SeededStateBack::prepare_until(&contract, &schema).is_err());
}
