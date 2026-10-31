//! Finite acquisition requests use the existing admitted Host Call lifecycle.
//! PCM is emitted port-specifically; no provider polling or sleeping in Step.
use super::back::{BackBudget, BackFactory, InstalledBack};
use crate::hosted_microphone::capture_flow::MicrophoneFlowLimits;
use conduit_core::{ConfigurationValue, PlannedGear};
use conduit_kernel::scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
    ValueRef, ValueStorage,
};
pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::MICROPHONE_FLOW_IMPLEMENTATION,
    budget,
    prepare,
};
pub(super) static CONVERTED_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::CONVERTED_MICROPHONE_FLOW_IMPLEMENTATION,
    budget,
    prepare,
};
pub(super) struct MicrophoneFlowBack {
    next: ValueRef,
    pending: bool,
    started: bool,
    complete: bool,
    blocks: u16,
    maximum_blocks: u16,
}
pub(super) fn configuration(placement: &PlannedGear) -> Result<(u32, u16), String> {
    let field = |key: &str, maximum: u64| -> Result<u64, String> {
        let mut fields = placement
            .configuration
            .iter()
            .filter(|field| field.key == key);
        let field = fields
            .next()
            .ok_or_else(|| format!("missing finite capture {key}"))?;
        if fields.next().is_some() {
            return Err("duplicate capture configuration".into());
        }
        match field.value {
            ConfigurationValue::U64(value) if value > 0 && value <= maximum => Ok(value),
            _ => Err("invalid capture configuration".into()),
        }
    };
    if placement.configuration.len() != 2 {
        return Err("unexpected capture configuration".into());
    }
    let millis = field(conduit_semantic_catalog::AUDIO_CAPTURE_MILLIS_KEY, 6000)? as u32;
    let frames = field(
        conduit_semantic_catalog::AUDIO_CAPTURE_BLOCK_FRAMES_KEY,
        2048,
    )? as u16;
    MicrophoneFlowLimits {
        total_frames: millis * 16,
        block_frames: frames,
    }
    .block_count()
    .map_err(|_| "capture block work bound".to_string())?;
    Ok((millis, frames))
}
fn offer(placement: &PlannedGear) -> Result<conduit_core::CapabilityOffer, String> {
    let converted = match placement.implementation_id.as_str() {
        conduit_std_offers::MICROPHONE_FLOW_IMPLEMENTATION => false,
        conduit_std_offers::CONVERTED_MICROPHONE_FLOW_IMPLEMENTATION => true,
        _ => return Err("unknown finite microphone Back".into()),
    };
    Ok(conduit_std_offers::microphone_flow_offer(converted))
}
fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offered = offer(placement)?;
    if placement.kind_id != offered.kind_id
        || placement.kind_contract_revision != offered.kind_contract_revision
        || placement.execution_profile_id != offered.implementation.execution_profile_id
        || placement.artifact_id != offered.implementation.artifact_id
        || placement.inputs != offered.inputs
        || placement.outputs != offered.outputs
        || placement.host_calls != offered.host_calls
        || placement.resources.len() != 1
        || placement.authority.len() != 1
    {
        return Err("finite capture differs from installed Back".into());
    }
    configuration(placement)?;
    Ok(())
}
fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate(placement)?;
    let (millis, frames) = configuration(placement)?;
    let blocks = MicrophoneFlowLimits {
        total_frames: millis * 16,
        block_frames: frames,
    }
    .block_count()
    .map_err(|_| "capture work bound")?;
    Ok(BackBudget {
        value_items: 3,
        value_bytes: 2 * conduit_semantic_catalog::AUDIO_CAPTURE_FINITE_BLOCK_BYTES + 16,
        maximum_value_bytes: conduit_semantic_catalog::AUDIO_CAPTURE_FINITE_BLOCK_BYTES,
        host_requests: usize::from(blocks) + 1,
        sign_items: blocks * 16 + 16,
    })
}
fn prepare(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    validate(placement)?;
    let (millis, frames) = configuration(placement)?;
    let blocks = MicrophoneFlowLimits {
        total_frames: millis * 16,
        block_frames: frames,
    }
    .block_count()
    .map_err(|_| "capture work bound")?;
    let next = values
        .store(b"next")
        .map_err(|error| format!("prepare capture token: {error:?}"))?;
    Ok(InstalledBack::MicrophoneFlow(MicrophoneFlowBack {
        next,
        pending: false,
        started: false,
        complete: false,
        blocks: 0,
        maximum_blocks: blocks,
    }))
}
fn fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}
impl<const PORTS: usize> StepBack<PORTS> for MicrophoneFlowBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.complete {
            return StepOutcome::Complete;
        }
        if let Some((request, outcome)) = io.host_completion() {
            if !self.pending || request != RequestId(u32::from(self.blocks)) {
                return fail(FailureCode::InvalidLifecycle, 20);
            }
            match (outcome.disposition, outcome.output, outcome.failure) {
                (HostCallDisposition::Completed, Some(output), None) => {
                    if self.blocks == self.maximum_blocks {
                        return fail(FailureCode::WorkBudgetExhausted, 20);
                    }
                    if !io.output_ready(PortId(0)) {
                        return fail(FailureCode::StorageExhausted, 19);
                    }
                    io.consume_host_completion()
                        .expect("observed finite capture completion");
                    io.send(PortId(0), output.value)
                        .expect("ready finite capture output");
                    self.blocks += 1;
                    self.pending = false;
                    StepOutcome::Progress
                }
                (HostCallDisposition::Completed, None, None) => {
                    io.consume_host_completion()
                        .expect("observed finite capture close");
                    if self.blocks != self.maximum_blocks {
                        return fail(FailureCode::InvalidInput, 20);
                    }
                    self.complete = true;
                    self.pending = false;
                    StepOutcome::Progress
                }
                (HostCallDisposition::Cancelled, _, _) => fail(FailureCode::Cancelled, 20),
                (_, _, Some(failure)) => StepOutcome::Fail(failure),
                _ => fail(FailureCode::HostCallFailed, 20),
            }
        } else if self.started {
            if self.pending {
                return StepOutcome::Await;
            }
            io.request_host_call(
                RequestId(u32::from(self.blocks)),
                HostCallId(0),
                BoundedValueRef::new(self.next, 16).expect("prepared next token"),
            )
            .expect("finite capture request");
            self.pending = true;
            StepOutcome::Progress
        } else if let Some(request) = io.input(PortId(0)) {
            let Ok(input) = BoundedValueRef::new(request, 16) else {
                return fail(FailureCode::InvalidInput, 20);
            };
            io.consume(PortId(0)).expect("capture trigger");
            io.request_host_call(RequestId(0), HostCallId(0), input)
                .expect("initial finite capture request");
            self.started = true;
            self.pending = true;
            StepOutcome::Progress
        } else if io.input_closed(PortId(0)) {
            io.consume_closed(PortId(0))
                .expect("unused finite capture trigger closed");
            io.discard(self.next)
                .expect("unused prepared capture token");
            self.complete = true;
            StepOutcome::Complete
        } else {
            StepOutcome::Await
        }
    }
    fn cancel(&mut self) {
        self.pending = false;
        self.complete = true;
    }
    fn retains_host_call_input(&self, _request: RequestId, value: ValueRef) -> bool {
        value == self.next && !self.complete
    }
}
