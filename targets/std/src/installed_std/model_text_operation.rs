//! Installed validated projection from a model-derived envelope to bounded text.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::PlannedGear;
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    OperationAction, OperationInput, PortId, RequestId,
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

impl ModelTextOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none() && (self.flow || !self.emitted) => {
                let Ok(input) = BoundedValueRef::new(value, self.maximum_input_bytes) else {
                    return fail(FailureCode::InvalidInput, 1);
                };
                let request = RequestId(self.next_request);
                self.next_request = self.next_request.saturating_add(1);
                self.pending = Some(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input,
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request) =>
            {
                self.pending = None;
                match (outcome.disposition, outcome.output, outcome.failure) {
                    (HostOperationDisposition::Completed, Some(output), None) => {
                        self.emitted = true;
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    (HostOperationDisposition::Denied, _, _) => {
                        fail(FailureCode::HostOperationDenied, 2)
                    }
                    (HostOperationDisposition::Cancelled, _, _) => fail(FailureCode::Cancelled, 3),
                    (HostOperationDisposition::Failed, _, _) => {
                        fail(FailureCode::HostOperationFailed, 4)
                    }
                    _ => fail(FailureCode::InvalidLifecycle, 5),
                }
            }
            OperationInput::Closed { port: PortId(0) } if self.pending.is_none() && self.flow => {
                OperationAction::Complete
            }
            _ => fail(FailureCode::InvalidLifecycle, 6),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        if self.emitted && !self.flow {
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
    }
}

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
        || placement.host_operations != offer.host_operations
        || !placement.configuration.is_empty()
    {
        return Err("planned model-text projection identity does not match installation".into());
    }
    Ok(())
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    let offer = selected_offer(placement);
    let input = offer.host_operations[0].maximum_input_bytes;
    let output = offer.host_operations[0].maximum_output_bytes;
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
        maximum_input_bytes: offer.host_operations[0].maximum_input_bytes,
    }))
}

fn fail(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}
