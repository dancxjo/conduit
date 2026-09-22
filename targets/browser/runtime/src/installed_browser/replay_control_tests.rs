use super::*;
use conduit_core::{
    BoundedResourceRef, ConfigurationEntry, ConfigurationValue, OfferGeneration, ResourceClassId,
    ResourceExtent, ResourceLifetime, ResourceSemanticIdentity, ResourceVersionIdentity,
    TemporalInstant, TemporalScale,
};

#[test]
fn browser_replay_control_preserves_semantics_and_explicitly_narrows_capacity() {
    let offer = offer();
    let semantic = conduit_time::replay_control_semantic_contract();
    assert_eq!(offer.startup_parameters, semantic.startup_parameters);
    assert_eq!(offer.kind_id, semantic.kind_id);
    assert_eq!(
        offer.kind_contract_revision,
        semantic.kind_contract_revision
    );
    assert_eq!(offer.inputs, semantic.inputs);
    assert_eq!(offer.outputs, semantic.outputs);
    assert_eq!(offer.limits.max_active_instances, 1);
    assert_eq!(offer.limits.max_queue_items, MAXIMUM_INPUTS as u16);
    assert_eq!(
        offer.limits.max_queue_bytes,
        super::super::MAXIMUM_BROWSER_VALUE_BYTES as u32 * MAXIMUM_INPUTS
    );
    assert!(offer.limits.max_active_instances < semantic.limits.max_active_instances);
    assert!(offer.limits.max_queue_bytes < semantic.limits.max_queue_bytes);
}

fn value(slot: u16) -> ValueRef {
    ValueRef {
        slot,
        generation: 1,
        byte_len: 32,
    }
}

fn completed(value: Option<ValueRef>) -> HostCallOutcome {
    HostCallOutcome {
        disposition: HostCallDisposition::Completed,
        output: value.map(|value| BoundedValueRef::new(value, 4096).unwrap()),
        failure: None,
    }
}

fn input_step(
    operation: &mut ReplayControlBack,
    port: PortId,
    value: ValueRef,
) -> (StepOutcome, StepIo<3>) {
    let mut inputs = [None; 3];
    inputs[usize::from(port.0)] = Some(value);
    let mut io = StepIo::test_frame(
        inputs,
        [false; 3],
        [
            Some(super::super::MAXIMUM_BROWSER_VALUE_BYTES as u32),
            Some(super::super::MAXIMUM_BROWSER_VALUE_BYTES as u32),
            None,
        ],
        None,
        5,
    );
    let outcome = operation.step(
        &mut io,
        &StepInputBytes::test_frame([None, None, None], None),
    );
    (outcome, io)
}

fn completion_step(
    operation: &mut ReplayControlBack,
    request: RequestId,
    outcome: HostCallOutcome,
) -> (StepOutcome, StepIo<3>) {
    let mut io = StepIo::test_frame(
        [None; 3],
        [false; 3],
        [
            Some(super::super::MAXIMUM_BROWSER_VALUE_BYTES as u32),
            Some(super::super::MAXIMUM_BROWSER_VALUE_BYTES as u32),
            None,
        ],
        Some((request, outcome)),
        5,
    );
    let result = operation.step(
        &mut io,
        &StepInputBytes::test_frame([None, None, None], None),
    );
    (result, io)
}

fn close_step(operation: &mut ReplayControlBack, closed: [bool; 3]) -> (StepOutcome, StepIo<3>) {
    let mut io = StepIo::test_frame([None; 3], closed, [None; 3], None, 3);
    let result = operation.step(
        &mut io,
        &StepInputBytes::test_frame([None, None, None], None),
    );
    (result, io)
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
        base: None,
        realization_characteristics: Vec::new(),
        limits: offer.limits,
        inputs: offer.inputs,
        outputs: offer.outputs,
        host_calls: offer.host_calls,
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
        .execute(HOST_CALL, &timeline)
        .unwrap()
        .unwrap()
        .to_vec();
    assert_eq!(
        conduit_time::decode_replay_state(payload(&initial, "history/replay-state@1")),
        Ok(conduit_time::ReplayState::Stopped)
    );
    assert!(prepared.execute(HOST_CALL, &initial).unwrap().is_none());

    let start = leaf(
        "history/replay-control@1",
        replay_command(conduit_time::ReplayCommand::Start),
    );
    let started = prepared
        .execute(HOST_CALL, &start)
        .unwrap()
        .unwrap()
        .to_vec();
    assert!(prepared.execute(HOST_CALL, &started).unwrap().is_none());
    let step = leaf(
        "history/replay-control@1",
        replay_command(conduit_time::ReplayCommand::Step),
    );
    let state = prepared
        .execute(HOST_CALL, &step)
        .unwrap()
        .unwrap()
        .to_vec();
    let event = prepared.execute(HOST_CALL, &state).unwrap().unwrap();
    let event =
        conduit_time::decode_replay_event(payload(event, "history/replay-event@1")).unwrap();
    assert_eq!(event.historical_identity, "memory/amber");
    assert_eq!(event.historical_event_time.ticks, 1_000);
    assert_eq!(event.playback_ticks, 0);
}

#[test]
fn state_output_becomes_the_owned_event_request_token() {
    let mut operation = ReplayControlBack::new();
    let (result, input) = input_step(&mut operation, PortId(0), value(1));
    assert_eq!(result, StepOutcome::Progress);
    let (request, host_call, _) = input.test_host_request().unwrap();
    assert_eq!(host_call, HostCallId(0));
    let state = value(2);
    let (result, state_io) = completion_step(&mut operation, request, completed(Some(state)));
    assert_eq!(result, StepOutcome::Progress);
    assert_eq!(state_io.test_output(PortId(1)), Some(state));
    let (request, host_call, input) = state_io.test_host_request().unwrap();
    assert_eq!(host_call, HostCallId(0));
    assert_eq!(input.value, state);
    let event = value(3);
    let (result, event_io) = completion_step(&mut operation, request, completed(Some(event)));
    assert_eq!(result, StepOutcome::Progress);
    assert_eq!(event_io.test_output(PortId(0)), Some(event));
}

#[test]
fn absent_event_and_closed_inputs_remain_explicit() {
    let mut operation = ReplayControlBack::new();
    let (result, input) = input_step(&mut operation, PortId(2), value(1));
    assert_eq!(result, StepOutcome::Progress);
    let request = input.test_host_request().unwrap().0;
    assert_eq!(
        completion_step(&mut operation, request, completed(None)).0,
        StepOutcome::Progress
    );
    assert_eq!(
        close_step(&mut operation, [true, false, false]).0,
        StepOutcome::Progress
    );
    assert_eq!(
        close_step(&mut operation, [true, true, false]).0,
        StepOutcome::Progress
    );
    assert_eq!(
        close_step(&mut operation, [true; 3]).0,
        StepOutcome::Complete
    );
}

#[test]
fn completed_requests_use_monotonic_identities_for_long_lived_replay() {
    let mut operation = ReplayControlBack::new();
    for sequence in 0..100_000 {
        let (result, input) = input_step(&mut operation, PortId(2), value(1));
        assert_eq!(result, StepOutcome::Progress);
        let request = input.test_host_request().unwrap().0;
        assert_eq!(
            request,
            RequestId(sequence * 2),
            "processing request {sequence}"
        );
        let (result, state) = completion_step(&mut operation, request, completed(Some(value(2))));
        assert_eq!(result, StepOutcome::Progress);
        assert_eq!(state.test_output(PortId(1)), Some(value(2)));
        let request = state.test_host_request().unwrap().0;
        assert_eq!(
            request,
            RequestId(sequence * 2 + 1),
            "event request {sequence}"
        );
        assert_eq!(
            completion_step(&mut operation, request, completed(None)).0,
            StepOutcome::Progress
        );
    }
    assert_eq!(operation.next_request, Some(200_000));
}

#[test]
fn a_noncurrent_identity_cannot_complete_the_pending_stage() {
    let mut operation = ReplayControlBack::new();
    let (result, input) = input_step(&mut operation, PortId(0), value(1));
    assert_eq!(result, StepOutcome::Progress);
    let request = input.test_host_request().unwrap().0;
    assert_eq!(request, RequestId(0));
    assert_eq!(
        completion_step(&mut operation, RequestId(1), completed(None)).0,
        fail(14)
    );
    assert_eq!(
        completion_step(&mut operation, request, completed(None)).0,
        StepOutcome::Progress
    );
}

#[test]
fn request_identity_exhaustion_is_explicit_before_the_event_stage() {
    let mut operation = ReplayControlBack::new();
    operation.next_request = Some(u32::MAX);
    let (result, input) = input_step(&mut operation, PortId(0), value(1));
    assert_eq!(result, StepOutcome::Progress);
    let request = input.test_host_request().unwrap().0;
    assert_eq!(request, RequestId(u32::MAX));
    assert_eq!(
        completion_step(&mut operation, request, completed(Some(value(2)))).0,
        StepOutcome::Fail(failure(FailureCode::IdentityCapacityExhausted, 15))
    );
}
