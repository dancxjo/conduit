use super::*;
use conduit_core::{ConfigurationEntry, ConfigurationValue};
use conduit_kernel::Operation;

fn placement(count: u64) -> PlannedGear {
    let offer = conduit_std_offers::pulse_observe_offer();
    PlannedGear {
        placement_id: "pulse-placement".into(),
        gear_id: "pulse".into(),
        kind_id: offer.kind_id,
        kind_contract_revision: offer.kind_contract_revision,
        execution_profile_id: offer.implementation.execution_profile_id,
        configuration: vec![
            ConfigurationEntry {
                key: "period-ms".into(),
                value: ConfigurationValue::U64(320),
            },
            ConfigurationEntry {
                key: "maximum-pulses".into(),
                value: ConfigurationValue::U64(count),
            },
        ],
        host_id: "pulse-host".into(),
        boot_id: "pulse-boot".into(),
        offer_generation: conduit_core::OfferGeneration(1),
        capability_id: offer.capability_id,
        implementation_id: offer.implementation.implementation_id,
        artifact_id: offer.implementation.artifact_id,
        realization_characteristics: vec![],
        limits: offer.limits,
        inputs: offer.inputs,
        outputs: offer.outputs,
        host_operations: offer.host_operations,
        resources: vec![],
        authority: vec![],
        pool_references: vec![],
    }
}
fn prepared(count: u64) -> (InstalledOperation, HostedValueStore) {
    let mut values = HostedValueStore::new(128, 8, 1024).unwrap();
    let operation = prepare(&placement(count), &mut values).unwrap();
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
fn factory_checks_exact_identity_and_admits_every_output_before_start() {
    let mut wrong = placement(2);
    wrong.artifact_id = "foreign".into();
    assert!(budget(&wrong).is_err());
    wrong = placement(2);
    wrong.inputs[0].temporal = conduit_core::PortTemporal::Current;
    assert!(budget(&wrong).is_err());
    assert!(budget(&placement(65)).is_err());
    let admitted = budget(&placement(2)).unwrap();
    assert_eq!(
        (
            admitted.value_items,
            admitted.value_bytes,
            admitted.host_requests
        ),
        (2, 12, 0)
    );
    let (mut operation, values) = prepared(2);
    let before = values.allocation_capacities();
    let operation_capacity = operation.allocation_capacity();
    assert_eq!(operation.start(), OperationAction::Await);
    for sequence in 0..2 {
        let OperationAction::Emit {
            port: PortId(0),
            value,
        } = operation.resume_value(PortId(0), tick(), &conduit_time::encode_tick(sequence))
        else {
            panic!("exact pulse output");
        };
        let pulse = conduit_time::decode_pulse_observation(values.get(value).unwrap()).unwrap();
        assert_eq!((pulse.sequence, pulse.period_ms), (sequence as u32, 320));
        assert_eq!(operation.advance(), OperationAction::Await);
    }
    assert_eq!(
        operation.resume(OperationInput::Closed { port: PortId(0) }),
        OperationAction::Complete
    );
    assert_eq!(values.allocation_capacities(), before);
    assert_eq!(operation.allocation_capacity(), operation_capacity);
}

#[test]
fn malformed_order_exhaustion_and_cancel_have_distinct_terminal_details() {
    let (mut operation, _) = prepared(1);
    operation.start();
    assert_eq!(
        operation.resume_value(PortId(1), tick(), &[0; 8]),
        failure(FailureCode::InvalidPort, 480)
    );
    assert_eq!(
        operation.resume_value(PortId(0), tick(), &[0; 7]),
        failure(FailureCode::InvalidInput, 481)
    );
    assert_eq!(
        operation.resume_value(PortId(0), tick(), &conduit_time::encode_tick(1)),
        failure(FailureCode::InvalidInput, 482)
    );
    assert!(matches!(
        operation.resume_value(PortId(0), tick(), &[0; 8]),
        OperationAction::Emit { .. }
    ));
    operation.advance();
    assert_eq!(
        operation.resume_value(PortId(0), tick(), &conduit_time::encode_tick(1)),
        failure(FailureCode::StorageExhausted, 483)
    );
    operation.cancel();
    assert_eq!(
        operation.resume_value(PortId(0), tick(), &[0; 8]),
        failure(FailureCode::Cancelled, 484)
    );
    assert_eq!(
        operation.resume(OperationInput::Closed { port: PortId(0) }),
        failure(FailureCode::Cancelled, 484)
    );
}

#[test]
fn closure_requires_no_minimum_count_and_refuses_late_values() {
    let (mut operation, _) = prepared(2);
    operation.start();
    assert_eq!(
        operation.resume(OperationInput::Closed { port: PortId(0) }),
        OperationAction::Complete
    );
    assert!(matches!(
        operation.resume_value(PortId(0), tick(), &[0; 8]),
        OperationAction::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            ..
        })
    ));
}

#[path = "pulse_observation_kernel_tests.rs"]
mod kernel;

#[path = "pulse_observation_form_tests.rs"]
mod form;
