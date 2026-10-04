use super::*;
use conduit_core::{
    kind_id, StructuredFieldType, StructuredFieldValue, StructuredInfoValue, BOOL_INFO_ID,
};
use conduit_kernel::ValueRef;

fn reference(slot: u16, bytes: &[u8]) -> ValueRef {
    ValueRef {
        slot,
        generation: 1,
        byte_len: bytes.len() as u32,
    }
}

fn boolean() -> SeededStateBack {
    SeededStateBack::prepare(
        &CheckedValueContract::new(kind_id(BOOL_INFO_ID), 1, vec![]).unwrap(),
        &StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap(),
    )
    .unwrap()
}

#[test]
fn seed_wins_simultaneous_replacement_and_only_committed_seed_opens_feedback() {
    let mut back = boolean();
    let seed = reference(1, &[0]);
    let next = reference(2, &[1]);
    let mut io = StepIo::test_frame(
        [Some(seed), Some(next)],
        [false; 2],
        [Some(1), None],
        None,
        16,
    );
    assert_eq!(
        back.step(
            &mut io,
            &StepInputBytes::test_frame([Some(&[0]), Some(&[1])], None)
        ),
        StepOutcome::Progress
    );
    assert!(io.test_consumed(PortId(0)));
    assert!(io.test_retained(PortId(0)));
    assert!(!io.test_consumed(PortId(1)));
    assert_eq!(io.test_output(PortId(0)), Some(seed));
    assert!(!back.seeded);
    <SeededStateBack as StepBack<2>>::step_committed(&mut back);
    let allocations = crate::test_support::allocation::allocations_during(|| {
        for _ in 0..1000 {
            let mut io = StepIo::test_frame([None, Some(next)], [false; 2], [None; 2], None, 16);
            assert_eq!(
                back.step(
                    &mut io,
                    &StepInputBytes::test_frame([None, Some(&[1])], None)
                ),
                StepOutcome::Await
            );
            assert!(!io.test_consumed(PortId(1)));
            assert!(io.test_discards().iter().all(Option::is_none));
            assert_eq!(back.held, Some(seed));
        }
    });
    assert_eq!(allocations, 0);
    let mut io = StepIo::test_frame([None, Some(next)], [false; 2], [Some(1), None], None, 16);
    assert_eq!(
        back.step(
            &mut io,
            &StepInputBytes::test_frame([None, Some(&[1])], None)
        ),
        StepOutcome::Progress
    );
    assert_eq!(io.test_output(PortId(0)), Some(next));
    assert!(io.test_discards().contains(&Some(seed)));
    <SeededStateBack as StepBack<2>>::step_committed(&mut back);
    let mut io = StepIo::test_frame([None, None], [true, true], [None; 2], None, 16);
    assert_eq!(
        back.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
        StepOutcome::Progress
    );
    <SeededStateBack as StepBack<2>>::step_committed(&mut back);
    let mut io = StepIo::test_frame([None, None], [false, true], [None; 2], None, 16);
    assert_eq!(
        back.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
        StepOutcome::Complete
    );
    assert!(io.test_discards().contains(&Some(next)));
}

#[test]
fn missing_duplicate_or_malformed_seed_is_a_failure_without_replacement_or_emission() {
    let mut back = boolean();
    let mut io = StepIo::test_frame(
        [None, Some(reference(2, &[1]))],
        [true, false],
        [Some(1), None],
        None,
        16,
    );
    assert_eq!(
        back.step(
            &mut io,
            &StepInputBytes::test_frame([None, Some(&[1])], None)
        ),
        fail(931)
    );
    assert!(!io.test_consumed(PortId(1)));
    assert_eq!(io.test_output(PortId(0)), None);
    let mut io = StepIo::test_frame(
        [Some(reference(1, &[2])), None],
        [false; 2],
        [Some(1), None],
        None,
        16,
    );
    assert_eq!(
        back.step(
            &mut io,
            &StepInputBytes::test_frame([Some(&[2]), None], None)
        ),
        fail(934)
    );
    assert!(!io.test_consumed(PortId(0)));
    back.seeded = true;
    let mut io = StepIo::test_frame(
        [Some(reference(1, &[0])), Some(reference(2, &[1]))],
        [false; 2],
        [Some(1), None],
        None,
        16,
    );
    assert_eq!(
        back.step(
            &mut io,
            &StepInputBytes::test_frame([Some(&[0]), Some(&[1])], None)
        ),
        fail(932)
    );
    assert!(!io.test_consumed(PortId(0)));
    assert!(!io.test_consumed(PortId(1)));
}

#[test]
fn exact_structured_seed_waits_under_pressure_without_allocation_or_schema_substitution() {
    let boolean = StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap();
    let schema = StructuredInfoType::record(
        kind_id("fixture/state@1"),
        vec![StructuredFieldType::new("on", boolean.clone()).unwrap()],
    )
    .unwrap();
    let seed = StructuredInfoValue::record(
        schema.clone(),
        vec![
            StructuredFieldValue::new("on", StructuredInfoValue::leaf(boolean, vec![1]).unwrap())
                .unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let contract =
        CheckedValueContract::new(schema.profile().unwrap().value_kind().clone(), 1024, vec![])
            .unwrap();
    let mut back = SeededStateBack::prepare(&contract, &schema).unwrap();
    let value = reference(1, &seed);
    let allocations = crate::test_support::allocation::allocations_during(|| {
        for _ in 0..1000 {
            let mut io = StepIo::test_frame([Some(value), None], [false; 2], [None; 2], None, 16);
            assert_eq!(
                back.step(
                    &mut io,
                    &StepInputBytes::test_frame([Some(&seed), None], None)
                ),
                StepOutcome::Await
            );
            assert!(!io.test_consumed(PortId(0)));
            assert!(!back.seeded);
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
                &StepInputBytes::test_frame([Some(&seed), None], None)
            ),
            StepOutcome::Progress
        );
        assert_eq!(io.test_output(PortId(0)), Some(value));
        <SeededStateBack as StepBack<2>>::step_committed(&mut back);
    });
    assert_eq!(allocations, 0);
    assert!(SeededStateBack::prepare(
        &contract,
        &StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap()
    )
    .is_err());
    <SeededStateBack as StepBack<2>>::cancel(&mut back);
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
            &StepInputBytes::test_frame([Some(&seed), None], None)
        ),
        StepOutcome::Complete
    );
    assert_eq!(io.test_output(PortId(0)), None);
}
