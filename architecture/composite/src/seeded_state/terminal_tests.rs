use super::*;
use alloc::boxed::Box;
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
    let canonical = Box::leak(schema.canonical_bytes().unwrap().into_boxed_slice());
    for mut back in [
        SeededStateBack::prepare_until(&contract, &schema).unwrap(),
        SeededStateBack::prepare_until_static(&contract, &schema, canonical).unwrap(),
    ] {
        let value = ValueRef {
            slot: 1,
            generation: 1,
            byte_len: bytes.len() as u32,
        };
        let allocations = crate::test_support::allocation::allocations_during(|| {
            for _ in 0..1000 {
                let mut io =
                    StepIo::test_frame([Some(value), None], [false; 2], [None; 2], None, 16);
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
}

#[test]
fn terminal_state_requires_an_exact_record_boolean() {
    let schema = StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap();
    let contract = CheckedValueContract::new(kind_id(BOOL_INFO_ID), 1, vec![]).unwrap();
    assert!(SeededStateBack::prepare_until(&contract, &schema).is_err());
}

#[test]
fn linked_state_retains_exact_schema_and_rejects_invalid_unselected_leaves() {
    let boolean = StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap();
    let schema = StructuredInfoType::record(
        kind_id("test/linked-state"),
        vec![
            StructuredFieldType::new("terminal", boolean.clone()).unwrap(),
            StructuredFieldType::new("hidden", boolean.clone()).unwrap(),
        ],
    )
    .unwrap();
    let canonical = Box::leak(schema.canonical_bytes().unwrap().into_boxed_slice());
    let contract =
        CheckedValueContract::new(schema.profile().unwrap().value_kind().clone(), 1024, vec![])
            .unwrap();
    let owned = SeededStateBack::prepare_until(&contract, &schema).unwrap();
    let linked = SeededStateBack::prepare_until_static(&contract, &schema, canonical).unwrap();
    let wrong = Box::leak(boolean.canonical_bytes().unwrap().into_boxed_slice());
    assert!(SeededStateBack::prepare_until_static(&contract, &schema, wrong).is_err());
    for terminal in [0, 1] {
        let value = StructuredInfoValue::record(
            schema.clone(),
            vec![
                StructuredFieldValue::new(
                    "terminal",
                    StructuredInfoValue::leaf(boolean.clone(), vec![terminal]).unwrap(),
                )
                .unwrap(),
                StructuredFieldValue::new(
                    "hidden",
                    StructuredInfoValue::leaf(boolean.clone(), vec![1]).unwrap(),
                )
                .unwrap(),
            ],
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();
        let mut malformed = value.clone();
        *malformed.last_mut().unwrap() = 2;
        let mut trailing = value.clone();
        trailing.push(0);
        assert!(linked.valid(&value));
        for bytes in [
            &value[..],
            &malformed,
            &trailing,
            &value[..value.len() - 1],
            &vec![0; 1025],
        ] {
            assert_eq!(owned.valid(bytes), linked.valid(bytes));
        }
        assert!(!linked.valid(&malformed));
    }
}
