use super::*;
use conduit_core::{ConfigurationEntry, ConfigurationValue};
use conduit_kernel::scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome};

fn placement() -> PlannedGear {
    let offer = conduit_std_offers::pulse_observe_offer();
    PlannedGear {
        placement_id: "pulse-placement".into(),
        gear_id: "pulse".into(),
        kind_id: offer.kind_id,
        kind_contract_revision: offer.kind_contract_revision,
        execution_profile_id: offer.implementation.execution_profile_id,
        configuration: vec![ConfigurationEntry {
            key: "period-ms".into(),
            value: ConfigurationValue::U64(320),
        }],
        host_id: "pulse-host".into(),
        boot_id: "pulse-boot".into(),
        offer_generation: conduit_core::OfferGeneration(1),
        capability_id: offer.capability_id,
        implementation_id: offer.implementation.implementation_id,
        artifact_id: offer.implementation.artifact_id,
        base: None,
        realization_characteristics: vec![],
        limits: offer.limits,
        inputs: offer.inputs,
        outputs: offer.outputs,
        host_calls: offer.host_calls,
        resources: vec![],
        authority: vec![],
        pool_references: vec![],
    }
}
fn prepared() -> (conduit_time::PulseObservationOperation, HostedValueStore) {
    let mut values = HostedValueStore::new(128, 8, 1024).unwrap();
    let InstalledOperation::PulseObserve(operation) = prepare(&placement(), &mut values).unwrap()
    else {
        panic!("pulse preparation returned another Back");
    };
    (operation, values)
}
fn tick() -> ValueRef {
    ValueRef {
        slot: 127,
        generation: 1,
        byte_len: 8,
    }
}

#[test]
fn factory_checks_exact_identity_and_needs_no_lifetime_output_allocation() {
    let mut wrong = placement();
    wrong.artifact_id = "foreign".into();
    assert!(budget(&wrong).is_err());
    wrong = placement();
    wrong.inputs[0].temporal = conduit_core::PortTemporal::Current;
    assert!(budget(&wrong).is_err());
    let admitted = budget(&placement()).unwrap();
    assert_eq!(
        (
            admitted.value_items,
            admitted.value_bytes,
            admitted.host_requests
        ),
        (0, 0, 0)
    );
    let (mut operation, values) = prepared();
    let before = values.allocation_capacities();
    let operation_capacity = operation.allocation_capacity();
    for sequence in 0..2 {
        let encoded = conduit_time::encode_tick(sequence);
        let mut io = StepIo::test_frame([Some(tick())], [false], [Some(32)], None, 8);
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame([Some(&encoded)], None)),
            StepOutcome::Progress
        );
        let value = io
            .test_canonical_output()
            .map(|(_, value)| value)
            .expect("exact pulse output");
        let pulse = conduit_time::decode_pulse_observation(value.as_slice()).unwrap();
        assert_eq!((pulse.sequence, pulse.period_ms), (sequence as u32, 320));
    }
    let mut io = StepIo::test_frame([None], [true], [None], None, 8);
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Complete
    );
    assert_eq!(values.allocation_capacities(), before);
    assert_eq!(operation.allocation_capacity(), operation_capacity);
}

#[test]
fn malformed_order_and_cancel_have_distinct_terminal_details() {
    let (mut operation, _) = prepared();
    let mut io = StepIo::test_frame([Some(tick())], [false], [Some(32)], None, 8);
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([Some(&[0; 7])], None)),
        StepOutcome::Fail(Failure {
            code: FailureCode::InvalidInput,
            detail: 481,
        })
    );
    let out_of_order = conduit_time::encode_tick(1);
    let mut io = StepIo::test_frame([Some(tick())], [false], [Some(32)], None, 8);
    assert_eq!(
        operation.step(
            &mut io,
            &StepInputBytes::test_frame([Some(&out_of_order)], None)
        ),
        StepOutcome::Fail(Failure {
            code: FailureCode::InvalidInput,
            detail: 482,
        })
    );
    for sequence in 0..2 {
        let encoded = conduit_time::encode_tick(sequence);
        let mut io = StepIo::test_frame([Some(tick())], [false], [Some(32)], None, 8);
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame([Some(&encoded)], None)),
            StepOutcome::Progress
        );
    }
    StepBack::<1>::cancel(&mut operation);
    let mut io = StepIo::test_frame([Some(tick())], [false], [Some(32)], None, 8);
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([Some(&[0; 8])], None)),
        StepOutcome::Fail(Failure {
            code: FailureCode::Cancelled,
            detail: 484,
        })
    );
    let mut io = StepIo::test_frame([None], [true], [None], None, 8);
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Fail(Failure {
            code: FailureCode::Cancelled,
            detail: 484,
        })
    );
}

#[test]
fn closure_requires_no_minimum_count_and_refuses_late_values() {
    let (mut operation, _) = prepared();
    let mut io = StepIo::test_frame([None], [true], [None], None, 8);
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Complete
    );
    let mut io = StepIo::test_frame([Some(tick())], [false], [Some(32)], None, 8);
    assert!(matches!(
        operation.step(&mut io, &StepInputBytes::test_frame([Some(&[0; 8])], None)),
        StepOutcome::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            ..
        })
    ));
}

#[path = "pulse_observation_kernel_tests.rs"]
mod kernel;

#[path = "pulse_observation_form_tests.rs"]
mod form;
