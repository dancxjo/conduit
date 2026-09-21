//! Installed validated projection from a model-derived envelope to bounded text.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::PlannedGear;
use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
};

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::MODEL_RESULT_TO_TEXT_STD_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct ModelTextOperation {
    pending: Option<RequestId>,
    next_request: u32,
    emitted: bool,
    flow: bool,
    maximum_input_bytes: u32,
}

impl<const PORTS: usize> StepOperation<PORTS> for ModelTextOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.emitted && !self.flow {
            return StepOutcome::Complete;
        }
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request) {
                return step_fail(FailureCode::InvalidLifecycle, 6);
            }
            match (outcome.disposition, outcome.output, outcome.failure) {
                (HostCallDisposition::Completed, Some(output), None) => {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.consume_host_completion()
                        .expect("observed model-text Host Call completion");
                    io.send(PortId(0), output.value)
                        .expect("ready model-text output");
                    self.pending = None;
                    self.emitted = true;
                    StepOutcome::Progress
                }
                (HostCallDisposition::Denied, _, _) => step_fail(FailureCode::HostCallDenied, 2),
                (HostCallDisposition::Cancelled, _, _) => step_fail(FailureCode::Cancelled, 3),
                (HostCallDisposition::Failed, _, _) => {
                    StepOutcome::Fail(outcome.failure.unwrap_or(Failure {
                        code: FailureCode::HostCallFailed,
                        detail: 4,
                    }))
                }
                _ => step_fail(FailureCode::InvalidLifecycle, 5),
            }
        } else if let Some(value) = io.input(PortId(0)) {
            if self.pending.is_some() || (!self.flow && self.emitted) {
                return step_fail(FailureCode::InvalidLifecycle, 6);
            }
            let Ok(input) = BoundedValueRef::new(value, self.maximum_input_bytes) else {
                return step_fail(FailureCode::InvalidInput, 1);
            };
            let request = RequestId(self.next_request);
            let Some(next) = self.next_request.checked_add(1) else {
                return step_fail(FailureCode::StorageExhausted, 1);
            };
            io.consume(PortId(0)).expect("present model result input");
            io.request_host_call(request, HostCallId(0), input)
                .expect("model-text projection Host Call");
            self.next_request = next;
            self.pending = Some(request);
            StepOutcome::Progress
        } else if self.flow && io.input_closed(PortId(0)) && self.pending.is_none() {
            io.consume_closed(PortId(0))
                .expect("observed model result closure");
            StepOutcome::Complete
        } else {
            StepOutcome::Await
        }
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}

const fn step_fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}

impl ModelTextOperation {}

fn selected_offer(placement: &PlannedGear) -> conduit_core::CapabilityOffer {
    match placement.kind_id.as_str() {
        conduit_ai::MODEL_RESULT_FLOW_TO_TEXT_KIND => {
            conduit_std_offers::model_result_flow_to_text_std_offer()
        }
        conduit_ai::GENERATED_CHUNK_TO_TEXT_KIND => {
            conduit_std_offers::generated_chunk_to_text_std_offer()
        }
        _ => conduit_std_offers::model_result_to_text_std_offer(),
    }
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = selected_offer(placement);
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id.as_str()
            != conduit_std_offers::MODEL_RESULT_TO_TEXT_STD_PROFILE
        || placement.implementation_id.as_str()
            != conduit_std_offers::MODEL_RESULT_TO_TEXT_STD_IMPLEMENTATION
        || placement.artifact_id.as_str() != conduit_std_offers::MODEL_RESULT_TO_TEXT_STD_ARTIFACT
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_calls != offer.host_calls
        || !placement.configuration.is_empty()
    {
        return Err("planned model-text projection identity does not match installation".into());
    }
    Ok(())
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    let offer = selected_offer(placement);
    let input = offer.host_calls[0].maximum_input_bytes;
    let output = offer.host_calls[0].maximum_output_bytes;
    let streaming_chunks = placement.kind_id.as_str() == conduit_ai::GENERATED_CHUNK_TO_TEXT_KIND;
    Ok(OperationBudget {
        value_items: if streaming_chunks {
            conduit_ai::MAXIMUM_GENERATED_TEXT_IN_FLIGHT_ITEMS.saturating_mul(2)
        } else {
            2
        },
        value_bytes: input.saturating_add(output),
        host_requests: if streaming_chunks {
            conduit_ai::MAXIMUM_GENERATED_TEXT_CHUNKS as usize
        } else {
            1
        },
        sign_items: 32,
        maximum_value_bytes: input.max(output),
    })
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    let offer = selected_offer(placement);
    Ok(InstalledOperation::ModelText(ModelTextOperation {
        pending: None,
        next_request: 0,
        emitted: false,
        flow: matches!(
            placement.kind_id.as_str(),
            conduit_ai::MODEL_RESULT_FLOW_TO_TEXT_KIND | conduit_ai::GENERATED_CHUNK_TO_TEXT_KIND
        ),
        maximum_input_bytes: offer.host_calls[0].maximum_input_bytes,
    }))
}
