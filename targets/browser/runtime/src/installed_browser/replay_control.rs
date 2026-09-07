//! Browser realization of finite replay timing and explicit control.

use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserOperation;
use conduit_core::{
    ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
    FaceStartupParameter, HostOperationContractId, HostOperationRequirement, ImplementationId,
    ImplementationOffer, KindContractRevision, PlannedGear, StructuredInfoType,
};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    HostOperationOutcome, HostedValueStore, Operation, OperationAction, OperationInput, PortId,
    RequestId, ValueRef,
};

pub(crate) const HOST_OPERATION: &str = "conduit.host/replay@1";
const IMPLEMENTATION: &str = "browser/replay@1";
const MAXIMUM_INPUTS: u32 = 64;

pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: None,
};

pub(crate) struct PreparedReplayControl {
    operation: conduit_time::BoundedReplayOperation,
    timeline_type: Vec<u8>,
    control_type: Vec<u8>,
    clock_type: Vec<u8>,
    event_type: Vec<u8>,
    state_type: Vec<u8>,
    event: Vec<u8>,
    state: Vec<u8>,
    event_canonical: Vec<u8>,
    state_canonical: Vec<u8>,
    last_playback_ticks: u64,
    event_pending: bool,
}

impl PreparedReplayControl {
    pub(crate) fn for_placement(placement: &PlannedGear) -> Result<Option<Self>, String> {
        if placement.implementation_id.as_str() != IMPLEMENTATION {
            return Ok(None);
        }
        validate_placement(placement, &offer())?;
        let policy = conduit_time::replay_policy_from_configuration(&placement.configuration)
            .map_err(|error| format!("prepare replay policy: {error:?}"))?;
        let maximum_duration =
            conduit_time::replay_maximum_duration_from_configuration(&placement.configuration)
                .map_err(|error| format!("prepare replay duration: {error:?}"))?;
        let operation = conduit_time::BoundedReplayOperation::new_with_maximum_duration(
            policy,
            maximum_duration,
        )
        .map_err(|error| format!("prepare replay operation: {error:?}"))?;
        Ok(Some(Self {
            operation,
            timeline_type: leaf_type("history/replay-timeline@1")?,
            control_type: leaf_type("history/replay-control@1")?,
            clock_type: leaf_type(conduit_time::PLAYBACK_TICK_INFO_ID)?,
            event_type: leaf_type("history/replay-event@1")?,
            state_type: leaf_type("history/replay-state@1")?,
            event: vec![0; conduit_time::MAXIMUM_REPLAY_EVENT_BYTES],
            state: vec![0; conduit_time::MAXIMUM_REPLAY_STATE_BYTES],
            event_canonical: Vec::with_capacity(super::MAXIMUM_BROWSER_VALUE_BYTES),
            state_canonical: Vec::with_capacity(super::MAXIMUM_BROWSER_VALUE_BYTES),
            last_playback_ticks: 0,
            event_pending: false,
        }))
    }

    pub(crate) fn execute<'a>(
        &'a mut self,
        _contract: &str,
        canonical: &[u8],
    ) -> Result<Option<&'a [u8]>, Failure> {
        if canonical == self.state_canonical && !canonical.is_empty() {
            let output = self
                .event_pending
                .then_some(self.event_canonical.as_slice());
            self.event_pending = false;
            return Ok(output);
        }

        self.event_pending = false;
        let output = if let Some(timeline) = exact_leaf(canonical, &self.timeline_type) {
            self.operation
                .load_timeline(timeline)
                .map_err(|_| failure(FailureCode::InvalidInput, 2))?;
            let state = self
                .operation
                .state()
                .ok_or_else(|| failure(FailureCode::InvalidInput, 3))?;
            let state_bytes = conduit_time::encode_replay_state_into(state, &mut self.state)
                .map_err(|_| failure(FailureCode::StorageExhausted, 4))?;
            Some((None, state_bytes))
        } else if let Some(control) = exact_leaf(canonical, &self.control_type) {
            let output = self
                .operation
                .apply_command(
                    control,
                    self.last_playback_ticks,
                    &mut self.event,
                    &mut self.state,
                )
                .map_err(|_| failure(FailureCode::InvalidInput, 6))?;
            output.state_bytes.map(|state| (output.event_bytes, state))
        } else if let Some(clock) = exact_leaf(canonical, &self.clock_type) {
            let ticks = conduit_time::decode_playback_tick(clock)
                .map_err(|_| failure(FailureCode::InvalidInput, 9))?;
            let output = self
                .operation
                .poll(ticks, &mut self.event, &mut self.state)
                .map_err(|_| failure(FailureCode::InvalidInput, 10))?;
            self.last_playback_ticks = ticks;
            output.state_bytes.map(|state| (output.event_bytes, state))
        } else {
            return Err(failure(FailureCode::InvalidInput, 11));
        };

        let Some((event_bytes, state_bytes)) = output else {
            return Ok(None);
        };
        wrap_leaf(
            &self.state_type,
            &self.state[..state_bytes],
            &mut self.state_canonical,
        )?;
        if let Some(event_bytes) = event_bytes {
            wrap_leaf(
                &self.event_type,
                &self.event[..event_bytes],
                &mut self.event_canonical,
            )?;
            self.event_pending = true;
        }
        Ok(Some(&self.state_canonical))
    }
}

fn offer() -> CapabilityOffer {
    let definition = conduit_time::replay_control_kind_definition();
    CapabilityOffer {
        startup_parameters: [
            ("mode", "Text"),
            ("rate-numerator", "Count"),
            ("rate-denominator", "Count"),
            ("maximum-duration-seconds", "Count"),
        ]
        .map(|(name, value_type)| FaceStartupParameter {
            name: name.into(),
            value_type: value_type.into(),
            has_default: true,
        })
        .into(),
        shorthand: None,
        capability_id: CapabilityId::from(IMPLEMENTATION),
        kind_id: definition.kind_id.clone(),
        kind_contract_revision: KindContractRevision::from(
            conduit_time::REPLAY_CONTROL_CONTRACT_REVISION,
        ),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(IMPLEMENTATION),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from("time/replay@1"),
        },
        inputs: definition.inputs,
        outputs: definition.outputs,
        host_operations: vec![host_operation(HOST_OPERATION, &definition.kind_id)],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: MAXIMUM_INPUTS as u16,
            max_queue_bytes: super::MAXIMUM_BROWSER_VALUE_BYTES as u32 * MAXIMUM_INPUTS,
        },
    }
}

fn host_operation(contract: &str, kind: &conduit_core::KindId) -> HostOperationRequirement {
    HostOperationRequirement {
        contract_id: HostOperationContractId::from(contract),
        target_kind: Some(kind.clone()),
        maximum_in_flight: 1,
        maximum_input_bytes: super::MAXIMUM_BROWSER_VALUE_BYTES as u32,
        maximum_output_bytes: super::MAXIMUM_BROWSER_VALUE_BYTES as u32,
    }
}

fn prepare(placement: &PlannedGear, _: &mut HostedValueStore) -> Result<BrowserOperation, String> {
    PreparedReplayControl::for_placement(placement)?
        .ok_or_else(|| "replay control placement selected another implementation".to_string())?;
    Ok(BrowserOperation::installed(ReplayControlOperation::new()))
}

struct ReplayControlOperation {
    next_request: u32,
    stage: Stage,
    closed: [bool; 3],
}

#[derive(Copy, Clone)]
enum Stage {
    Awaiting,
    Processing(RequestId),
    StateEmitted(ValueRef),
    EventPending(RequestId),
    EventEmitted,
}

impl ReplayControlOperation {
    const fn new() -> Self {
        Self {
            next_request: 0,
            stage: Stage::Awaiting,
            closed: [false; 3],
        }
    }

    fn next(&mut self) -> Result<RequestId, Failure> {
        if self.next_request >= MAXIMUM_INPUTS.saturating_mul(2) {
            return Err(failure(FailureCode::StorageExhausted, 12));
        }
        let request = RequestId(self.next_request);
        self.next_request += 1;
        Ok(request)
    }
}

impl Operation for ReplayControlOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value { port, value }
                if matches!(self.stage, Stage::Awaiting)
                    && usize::from(port.0) < self.closed.len()
                    && !self.closed[usize::from(port.0)]
                    && value.byte_len <= super::MAXIMUM_BROWSER_VALUE_BYTES as u32 =>
            {
                let Ok(request) = self.next() else {
                    return fail(12);
                };
                if port.0 > 2 {
                    return fail(13);
                }
                self.stage = Stage::Processing(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input: BoundedValueRef::new(value, super::MAXIMUM_BROWSER_VALUE_BYTES as u32)
                        .expect("replay input bound was checked"),
                }
            }
            OperationInput::Closed { port }
                if matches!(self.stage, Stage::Awaiting)
                    && usize::from(port.0) < self.closed.len() =>
            {
                self.closed[usize::from(port.0)] = true;
                if self.closed.into_iter().all(|closed| closed) {
                    OperationAction::Complete
                } else {
                    OperationAction::Await
                }
            }
            OperationInput::HostOperationCompleted { request, outcome } if matches!(self.stage, Stage::Processing(expected) if expected == request) => {
                match completed_output(outcome) {
                    Ok(Some(value)) => {
                        self.stage = Stage::StateEmitted(value);
                        OperationAction::Emit {
                            port: PortId(1),
                            value,
                        }
                    }
                    Ok(None) => {
                        self.stage = Stage::Awaiting;
                        OperationAction::Await
                    }
                    Err(failure) => OperationAction::Fail(failure),
                }
            }
            OperationInput::HostOperationCompleted { request, outcome } if matches!(self.stage, Stage::EventPending(expected) if expected == request) => {
                match completed_output(outcome) {
                    Ok(Some(value)) => {
                        self.stage = Stage::EventEmitted;
                        OperationAction::Emit {
                            port: PortId(0),
                            value,
                        }
                    }
                    Ok(None) => {
                        self.stage = Stage::Awaiting;
                        OperationAction::Await
                    }
                    Err(failure) => OperationAction::Fail(failure),
                }
            }
            _ => fail(14),
        }
    }

    fn advance(&mut self) -> OperationAction {
        match self.stage {
            Stage::StateEmitted(value) => {
                let Ok(request) = self.next() else {
                    return fail(15);
                };
                self.stage = Stage::EventPending(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input: BoundedValueRef::new(value, super::MAXIMUM_BROWSER_VALUE_BYTES as u32)
                        .expect("replay state output is browser bounded"),
                }
            }
            Stage::EventEmitted => {
                self.stage = Stage::Awaiting;
                OperationAction::Await
            }
            _ => fail(16),
        }
    }
}

fn completed_output(outcome: HostOperationOutcome) -> Result<Option<ValueRef>, Failure> {
    match (outcome.disposition, outcome.output, outcome.failure) {
        (HostOperationDisposition::Completed, output, None) => Ok(output.map(|value| value.value)),
        (HostOperationDisposition::Failed, None, Some(failure)) => Err(failure),
        (HostOperationDisposition::Cancelled, None, None) => {
            Err(failure(FailureCode::Cancelled, 0))
        }
        _ => Err(failure(FailureCode::InvalidInput, 17)),
    }
}

fn leaf_type(identity: &str) -> Result<Vec<u8>, String> {
    StructuredInfoType::leaf(conduit_core::kind_id(identity))
        .map_err(|error| format!("replay control type: {error:?}"))?
        .canonical_bytes()
        .map_err(|error| format!("replay control type bytes: {error:?}"))
}

fn exact_leaf<'a>(canonical: &'a [u8], value_type: &[u8]) -> Option<&'a [u8]> {
    let node = canonical.strip_prefix(value_type)?;
    if node.first() != Some(&0) || node.len() < 5 {
        return None;
    }
    let length = usize::try_from(u32::from_le_bytes(node[1..5].try_into().ok()?)).ok()?;
    (node.len() == 5 + length).then_some(&node[5..])
}

fn wrap_leaf(value_type: &[u8], payload: &[u8], output: &mut Vec<u8>) -> Result<(), Failure> {
    let length =
        u32::try_from(payload.len()).map_err(|_| failure(FailureCode::StorageExhausted, 18))?;
    let total = value_type
        .len()
        .checked_add(5)
        .and_then(|value| value.checked_add(payload.len()))
        .ok_or_else(|| failure(FailureCode::StorageExhausted, 19))?;
    if total > super::MAXIMUM_BROWSER_VALUE_BYTES {
        return Err(failure(FailureCode::StorageExhausted, 20));
    }
    output.clear();
    output.extend_from_slice(value_type);
    output.push(0);
    output.extend_from_slice(&length.to_le_bytes());
    output.extend_from_slice(payload);
    Ok(())
}

fn fail(detail: u16) -> OperationAction {
    OperationAction::Fail(failure(FailureCode::InvalidInput, detail))
}

fn failure(code: FailureCode, detail: u16) -> Failure {
    Failure { code, detail }
}

#[cfg(test)]
#[path = "replay_control_tests.rs"]
mod tests;
