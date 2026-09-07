use super::*;
use conduit_core::{
    BoundedResourceRef, ConfigurationEntry, ConfigurationValue, OfferGeneration, ResourceClassId,
    ResourceExtent, ResourceLifetime, ResourceSemanticIdentity, ResourceVersionIdentity,
    TemporalInstant, TemporalScale,
};

fn value(slot: u16) -> ValueRef {
    ValueRef {
        slot,
        generation: 1,
        byte_len: 32,
    }
}

fn completed(value: Option<ValueRef>) -> HostOperationOutcome {
    HostOperationOutcome {
        disposition: HostOperationDisposition::Completed,
        output: value.map(|value| BoundedValueRef::new(value, 4096).unwrap()),
        failure: None,
    }
}

fn placement() -> PlannedGear {
    let offer = offer();
    PlannedGear {
        placement_id: "replay-placement".into(),
        gear_id: "replay".into(),
        kind_id: offer.kind_id,
        kind_contract_revision: offer.kind_contract_revision,
        execution_profile_id: offer.implementation.execution_profile_id,
        configuration: vec![
            configuration("mode", ConfigurationValue::Text("step".into())),
            configuration("rate-numerator", ConfigurationValue::U64(1)),
            configuration("rate-denominator", ConfigurationValue::U64(1)),
            configuration("maximum-duration-seconds", ConfigurationValue::U64(60)),
        ],
        host_id: "browser/replay".into(),
        boot_id: "browser-boot/replay".into(),
        offer_generation: OfferGeneration(1),
        capability_id: offer.capability_id,
        implementation_id: offer.implementation.implementation_id,
        artifact_id: offer.implementation.artifact_id,
        realization_characteristics: Vec::new(),
        limits: offer.limits,
        inputs: offer.inputs,
        outputs: offer.outputs,
        host_operations: offer.host_operations,
        resources: Vec::new(),
        authority: Vec::new(),
        pool_references: Vec::new(),
    }
}

fn configuration(key: &str, value: ConfigurationValue) -> ConfigurationEntry {
    ConfigurationEntry {
        key: key.into(),
        value,
    }
}

fn leaf(identity: &str, payload: Vec<u8>) -> Vec<u8> {
    conduit_core::StructuredInfoValue::leaf(
        StructuredInfoType::leaf(conduit_core::kind_id(identity)).unwrap(),
        payload,
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}

fn replay_timeline() -> Vec<u8> {
    let entry = conduit_time::HistoricalReplayEntry {
        sequence: 7,
        identity: "memory/amber".into(),
        event_time: TemporalInstant {
            ticks: 1_000,
            scale: TemporalScale::Milliseconds,
            clock_basis: "memory/event-clock".into(),
            resolution_ticks: 1,
            uncertainty_ticks: 0,
        },
        origin: conduit_time::HistoricalEntryOrigin::OperatorAuthored,
        value: BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest([1; 32]),
            content_profile: conduit_core::kind_id("memory/color@1"),
            access_class: ResourceClassId::from("conduit.resource/history-value@1"),
            extent: ResourceExtent {
                bytes: 4,
                items: Some(1),
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest([2; 32]),
                expires_at: None,
            },
        },
    };
    let mut output = vec![0; conduit_time::MAXIMUM_REPLAY_TIMELINE_BYTES];
    let length = conduit_time::encode_replay_timeline_into(&[entry], &mut output).unwrap();
    output.truncate(length);
    output
}

fn replay_command(command: conduit_time::ReplayCommand) -> Vec<u8> {
    let mut output = vec![0; conduit_time::MAXIMUM_REPLAY_COMMAND_BYTES];
    let length = conduit_time::encode_replay_command_into(command, &mut output).unwrap();
    output.truncate(length);
    output
}

fn payload<'a>(canonical: &'a [u8], identity: &str) -> &'a [u8] {
    let value_type = StructuredInfoType::leaf(conduit_core::kind_id(identity))
        .unwrap()
        .canonical_bytes()
        .unwrap();
    exact_leaf(canonical, &value_type).unwrap()
}

#[test]
fn prepared_step_preserves_historical_and_playback_time_as_distinct_values() {
    let mut prepared = PreparedReplayControl::for_placement(&placement())
        .unwrap()
        .unwrap();
    let timeline = leaf("history/replay-timeline@1", replay_timeline());
    let initial = prepared
        .execute(HOST_OPERATION, &timeline)
        .unwrap()
        .unwrap()
        .to_vec();
    assert_eq!(
        conduit_time::decode_replay_state(payload(&initial, "history/replay-state@1")),
        Ok(conduit_time::ReplayState::Stopped)
    );
    assert!(prepared
        .execute(HOST_OPERATION, &initial)
        .unwrap()
        .is_none());

    let start = leaf(
        "history/replay-control@1",
        replay_command(conduit_time::ReplayCommand::Start),
    );
    let started = prepared
        .execute(HOST_OPERATION, &start)
        .unwrap()
        .unwrap()
        .to_vec();
    assert!(prepared
        .execute(HOST_OPERATION, &started)
        .unwrap()
        .is_none());
    let step = leaf(
        "history/replay-control@1",
        replay_command(conduit_time::ReplayCommand::Step),
    );
    let state = prepared
        .execute(HOST_OPERATION, &step)
        .unwrap()
        .unwrap()
        .to_vec();
    let event = prepared.execute(HOST_OPERATION, &state).unwrap().unwrap();
    let event =
        conduit_time::decode_replay_event(payload(event, "history/replay-event@1")).unwrap();
    assert_eq!(event.historical_identity, "memory/amber");
    assert_eq!(event.historical_event_time.ticks, 1_000);
    assert_eq!(event.playback_ticks, 0);
}

#[test]
fn state_output_becomes_the_owned_event_request_token() {
    let mut operation = ReplayControlOperation::new();
    let OperationAction::RequestHostOperation {
        request,
        operation: host_operation,
        ..
    } = operation.resume(OperationInput::Value {
        port: PortId(0),
        value: value(1),
    })
    else {
        panic!("timeline input must request its exact host operation");
    };
    assert_eq!(host_operation, HostOperationId(0));
    let state = value(2);
    assert_eq!(
        operation.resume(OperationInput::HostOperationCompleted {
            request,
            outcome: completed(Some(state)),
        }),
        OperationAction::Emit {
            port: PortId(1),
            value: state,
        }
    );
    let OperationAction::RequestHostOperation {
        request,
        operation: host_operation,
        input,
    } = operation.advance()
    else {
        panic!("state emission must request the correlated event");
    };
    assert_eq!(host_operation, HostOperationId(0));
    assert_eq!(input.value, state);
    let event = value(3);
    assert_eq!(
        operation.resume(OperationInput::HostOperationCompleted {
            request,
            outcome: completed(Some(event)),
        }),
        OperationAction::Emit {
            port: PortId(0),
            value: event,
        }
    );
    assert_eq!(operation.advance(), OperationAction::Await);
}

#[test]
fn absent_event_and_closed_inputs_remain_explicit() {
    let mut operation = ReplayControlOperation::new();
    let OperationAction::RequestHostOperation { request, .. } =
        operation.resume(OperationInput::Value {
            port: PortId(2),
            value: value(1),
        })
    else {
        panic!("clock input must request its exact host operation");
    };
    assert_eq!(
        operation.resume(OperationInput::HostOperationCompleted {
            request,
            outcome: completed(None),
        }),
        OperationAction::Await
    );
    assert_eq!(
        operation.resume(OperationInput::Closed { port: PortId(0) }),
        OperationAction::Await
    );
    assert_eq!(
        operation.resume(OperationInput::Closed { port: PortId(1) }),
        OperationAction::Await
    );
    assert_eq!(
        operation.resume(OperationInput::Closed { port: PortId(2) }),
        OperationAction::Complete
    );
}
