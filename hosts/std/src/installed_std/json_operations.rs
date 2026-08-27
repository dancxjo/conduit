use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{JsonRefusal, JsonValue, PlannedGear};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    OperationAction, OperationInput, PortId, RequestId,
};
use std::vec::Vec;

pub(super) struct JsonHost {
    output: Vec<u8>,
}

impl JsonHost {
    pub(super) fn prepare() -> Self {
        Self {
            output: Vec::with_capacity(conduit_core::JSON_MAXIMUM_ENCODED_BYTES),
        }
    }

    pub(super) fn execute<'a>(
        &'a mut self,
        contract: &str,
        input: &[u8],
    ) -> Result<&'a [u8], JsonRefusal> {
        let encoded = transform(contract, input)?;
        self.output.clear();
        self.output.extend_from_slice(&encoded);
        Ok(&self.output)
    }
}

pub(super) static JSON_ENCODE_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::JSON_ENCODE_STD_IMPLEMENTATION,
    budget: encode_budget,
    prepare: prepare_encode,
};
pub(super) static JSON_DECODE_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::JSON_DECODE_STD_IMPLEMENTATION,
    budget: decode_budget,
    prepare: prepare_decode,
};

pub(super) struct JsonOperation {
    pending: Option<RequestId>,
    next: u32,
}

impl JsonOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none() && self.next < 4 => {
                let request = RequestId(self.next);
                self.pending = Some(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input: match BoundedValueRef::new(
                        value,
                        conduit_core::JSON_MAXIMUM_ENCODED_BYTES as u32,
                    ) {
                        Ok(input) => input,
                        Err(_) => return InstalledOperation::fail(101),
                    },
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request) =>
            {
                self.pending = None;
                match (outcome.disposition, outcome.output, outcome.failure) {
                    (HostOperationDisposition::Completed, Some(output), None) => {
                        self.next += 1;
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    (HostOperationDisposition::Cancelled, _, _) => OperationAction::Fail(Failure {
                        code: FailureCode::Cancelled,
                        detail: 0,
                    }),
                    (HostOperationDisposition::Failed, None, Some(failure)) => {
                        OperationAction::Fail(failure)
                    }
                    _ => InstalledOperation::fail(102),
                }
            }
            OperationInput::Closed { port: PortId(0) } if self.pending.is_none() => {
                OperationAction::Complete
            }
            _ => InstalledOperation::fail(103),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
    }
}

pub(super) fn transform(contract: &str, input: &[u8]) -> Result<Vec<u8>, JsonRefusal> {
    match contract {
        conduit_std_offers::JSON_ENCODE_HOST_OPERATION => {
            JsonValue::decode_info(input)?.encode_text()
        }
        conduit_std_offers::JSON_DECODE_HOST_OPERATION => {
            JsonValue::decode_text(input)?.encode_info()
        }
        _ => Err(JsonRefusal::NonCanonicalValue),
    }
}

fn budget(
    placement: &PlannedGear,
    offer: conduit_core::CapabilityOffer,
) -> Result<OperationBudget, String> {
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
    {
        return Err("planned JSON identity differs from installed realization".into());
    }
    Ok(OperationBudget {
        value_items: 4,
        value_bytes: (conduit_core::JSON_MAXIMUM_ENCODED_BYTES * 4) as u32,
        host_requests: 4,
        sign_items: 64,
        maximum_value_bytes: conduit_core::JSON_MAXIMUM_ENCODED_BYTES as u32,
    })
}

fn encode_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    budget(placement, conduit_std_offers::json_encode_std_offer())
}
fn decode_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    budget(placement, conduit_std_offers::json_decode_std_offer())
}
fn prepare_encode(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    encode_budget(placement)?;
    Ok(InstalledOperation::JsonEncode(JsonOperation {
        pending: None,
        next: 0,
    }))
}
fn prepare_decode(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    decode_budget(placement)?;
    Ok(InstalledOperation::JsonDecode(JsonOperation {
        pending: None,
        next: 0,
    }))
}
