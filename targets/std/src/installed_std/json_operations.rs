use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::PlannedGear;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
};
use conduit_web::{JsonRefusal, JsonValue};
use std::vec::Vec;

#[cfg(test)]
#[path = "json_collection_tests.rs"]
mod collection_tests;
#[cfg(test)]
#[path = "json_summary_tests.rs"]
mod summary_tests;

pub(super) struct JsonHost {
    output: Vec<u8>,
    summary_fields: Vec<Option<String>>,
}

impl JsonHost {
    pub(super) fn prepare(fragment: &conduit_core::PlanFragment) -> Self {
        Self {
            output: Vec::with_capacity(conduit_web::JSON_MAXIMUM_ENCODED_BYTES),
            summary_fields: fragment
                .placements
                .iter()
                .map(|placement| {
                    if placement.kind_id.as_str() == conduit_web::JSON_BOOLEAN_SUMMARY_KIND {
                        super::json_summary_operation::field_configuration(placement)
                            .ok()
                            .map(str::to_owned)
                    } else {
                        None
                    }
                })
                .collect(),
        }
    }

    pub(super) fn execute<'a>(
        &'a mut self,
        node: usize,
        contract: &str,
        input: &[u8],
    ) -> Result<&'a [u8], u16> {
        let encoded = if contract == conduit_std_offers::JSON_COLLECTION_STEP_HOST_CALL {
            conduit_web::json_collection_step_bytes(input).map_err(collection_failure_detail)?
        } else if contract == conduit_std_offers::JSON_BOOLEAN_SUMMARY_HOST_CALL {
            let field = self
                .summary_fields
                .get(node)
                .and_then(Option::as_deref)
                .ok_or(120_u16)?;
            let value = JsonValue::decode_info(input).map_err(|error| error as u16)?;
            conduit_web::json_boolean_summary(&value, field)
                .map_err(|error| error.detail())?
                .encode_info()
                .map_err(|error| error as u16)?
        } else {
            transform(contract, input).map_err(|error| error as u16)?
        };
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

impl<const PORTS: usize> StepBack<PORTS> for JsonOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request) {
                return StepOutcome::Fail(step_failure(FailureCode::InvalidLifecycle, 103));
            }
            match (outcome.disposition, outcome.output, outcome.failure) {
                (HostCallDisposition::Completed, Some(output), None) => {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.consume_host_completion()
                        .expect("observed JSON Host Call completion");
                    io.send(PortId(0), output.value).expect("ready JSON output");
                    self.pending = None;
                    self.next += 1;
                    StepOutcome::Progress
                }
                (HostCallDisposition::Cancelled, _, _) => {
                    io.consume_host_completion()
                        .expect("observed cancelled JSON Host Call");
                    self.pending = None;
                    StepOutcome::Fail(Failure {
                        code: FailureCode::Cancelled,
                        detail: 0,
                    })
                }
                (HostCallDisposition::Failed, None, Some(failure)) => {
                    io.consume_host_completion()
                        .expect("observed failed JSON Host Call");
                    self.pending = None;
                    StepOutcome::Fail(failure)
                }
                _ => StepOutcome::Fail(step_failure(FailureCode::InvalidLifecycle, 102)),
            }
        } else if let Some(value) = io.input(PortId(0)) {
            if self.pending.is_some() || self.next >= 4 {
                return StepOutcome::Fail(step_failure(FailureCode::InvalidLifecycle, 103));
            }
            let Ok(input) =
                BoundedValueRef::new(value, conduit_web::JSON_MAXIMUM_ENCODED_BYTES as u32)
            else {
                return StepOutcome::Fail(step_failure(FailureCode::InvalidInput, 101));
            };
            let request = RequestId(self.next);
            io.consume(PortId(0)).expect("present JSON input");
            io.request_host_call(request, HostCallId(0), input)
                .expect("JSON Host Call");
            self.pending = Some(request);
            StepOutcome::Progress
        } else if io.input_closed(PortId(0)) && self.pending.is_none() {
            io.consume_closed(PortId(0))
                .expect("observed JSON input closure");
            StepOutcome::Complete
        } else {
            StepOutcome::Await
        }
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}

const fn step_failure(code: FailureCode, detail: u16) -> Failure {
    Failure { code, detail }
}

impl JsonOperation {
    pub(super) fn new() -> Self {
        Self {
            pending: None,
            next: 0,
        }
    }
}

pub(super) fn transform(contract: &str, input: &[u8]) -> Result<Vec<u8>, JsonRefusal> {
    match contract {
        conduit_std_offers::JSON_ENCODE_HOST_CALL => JsonValue::decode_info(input)?.encode_text(),
        conduit_std_offers::JSON_DECODE_HOST_CALL => JsonValue::decode_text(input)?.encode_info(),
        _ => Err(JsonRefusal::NonCanonicalValue),
    }
}

pub(super) fn budget(
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
        || placement.host_calls != offer.host_calls
    {
        return Err("planned JSON identity differs from installed realization".into());
    }
    Ok(OperationBudget {
        value_items: 4,
        value_bytes: (conduit_web::JSON_MAXIMUM_ENCODED_BYTES * 4) as u32,
        host_requests: 4,
        sign_items: 64,
        maximum_value_bytes: conduit_web::JSON_MAXIMUM_ENCODED_BYTES as u32,
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
    Ok(InstalledOperation::Json(JsonOperation {
        pending: None,
        next: 0,
    }))
}
fn prepare_decode(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    decode_budget(placement)?;
    Ok(InstalledOperation::Json(JsonOperation {
        pending: None,
        next: 0,
    }))
}

fn collection_failure_detail(error: conduit_web::JsonCollectionRefusal) -> u16 {
    use conduit_web::JsonCollectionRefusal::*;
    match error {
        InvalidRequest => 100,
        InvalidCollection => 101,
        InvalidCommand => 102,
        UnknownOperation => 103,
        InvalidIndex => 104,
        MissingIndex => 105,
        MissingField => 106,
        NotBoolean => 107,
        CollectionFull => 108,
        InvalidValue(error) => error as u16,
    }
}

pub(super) fn matches(contract: &str) -> bool {
    matches!(
        contract,
        conduit_std_offers::JSON_ENCODE_HOST_CALL
            | conduit_std_offers::JSON_DECODE_HOST_CALL
            | conduit_std_offers::JSON_COLLECTION_STEP_HOST_CALL
            | conduit_std_offers::JSON_BOOLEAN_SUMMARY_HOST_CALL
    )
}

pub(super) static JSON_COLLECTION_STEP_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::JSON_COLLECTION_STEP_STD_IMPLEMENTATION,
    budget: collection_budget,
    prepare: prepare_collection,
};
fn collection_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    budget(
        placement,
        conduit_std_offers::json_collection_step_std_offer(),
    )
}
fn prepare_collection(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    collection_budget(placement)?;
    Ok(InstalledOperation::Json(JsonOperation {
        pending: None,
        next: 0,
    }))
}
