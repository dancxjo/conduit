//! Finite preparation and ordinary-kernel emission for recurrence occurrences.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use super::recurrence_codec;
use super::recurrence_encoding;
use conduit_core::{ConfigurationValue, PlannedGear, StructuredInfoValue};
use conduit_kernel::{OperationAction, OperationInput, PortId, ValueRef, ValueStorage};
use conduit_time::RecurrenceRule;

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::RECURRENCE_STD_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct RecurrenceOperation {
    result: ValueRef,
    emitted: bool,
}

impl RecurrenceOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Emit {
            port: PortId(0),
            value: self.result,
        }
    }

    pub(super) fn resume(&mut self, _input: OperationInput) -> OperationAction {
        InstalledOperation::fail(220)
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        if self.emitted {
            InstalledOperation::fail(221)
        } else {
            self.emitted = true;
            OperationAction::Complete
        }
    }
}

fn request(placement: &PlannedGear) -> Result<recurrence_codec::DecodedRecurrence, String> {
    let [entry] = placement.configuration.as_slice() else {
        return Err("recurrence requires one exact planned request".into());
    };
    let ("request", ConfigurationValue::Structured(configuration)) =
        (entry.key.as_str(), &entry.value)
    else {
        return Err("recurrence planned request is malformed".into());
    };
    let value = StructuredInfoValue::from_canonical_bytes(configuration.canonical_value())
        .map_err(|error| format!("decode planned recurrence request: {error:?}"))?;
    let expected = conduit_semantic_catalog::recurrence_request_type();
    if value.value_type() != &expected
        || configuration.profile()
            != expected
                .profile()
                .map_err(|error| format!("profile recurrence request: {error:?}"))?
                .value_kind()
    {
        return Err("recurrence planned request type/profile mismatch".into());
    }
    recurrence_codec::decode(&value)
}

fn validate(placement: &PlannedGear) -> Result<recurrence_codec::DecodedRecurrence, String> {
    let offer = conduit_std_offers::recurrence_std_offer();
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
    {
        return Err("planned recurrence differs from installed realization".into());
    }
    let request = request(placement)?;
    if request.expansion.maximum_results
        > u32::from(conduit_semantic_catalog::RECURRENCE_MAXIMUM_RESULTS)
    {
        return Err("recurrence result bound exceeds the installed profile".into());
    }
    Ok(request)
}

fn expand(
    request: &recurrence_codec::DecodedRecurrence,
) -> Result<Vec<conduit_time::RecurrenceOccurrence>, String> {
    let result = match request.definition.rule {
        RecurrenceRule::CivilWeekdays { .. } => request.definition.expand_civil(
            &request.expansion,
            &request.resolutions,
            request.policy,
        ),
        _ if request.resolutions.is_empty() => request.definition.expand(&request.expansion),
        _ => return Err("non-civil recurrence cannot carry civil resolution truth".into()),
    };
    result.map_err(|error| format!("recurrence expansion refusal: {error:?}"))
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    let request = validate(placement)?;
    let occurrences = expand(&request)?;
    let encoded = recurrence_encoding::encode_batch(&occurrences)?;
    Ok(OperationBudget {
        value_items: 1,
        value_bytes: encoded
            .len()
            .try_into()
            .map_err(|_| "recurrence byte budget overflow")?,
        host_requests: 0,
        sign_items: 32,
        maximum_value_bytes: encoded
            .len()
            .try_into()
            .map_err(|_| "recurrence value bound overflow")?,
    })
}

fn prepare(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    let request = validate(placement)?;
    let occurrences = expand(&request)?;
    let encoded = recurrence_encoding::encode_batch(&occurrences)?;
    let result = values
        .store(&encoded)
        .map_err(|error| format!("store recurrence result: {error:?}"))?;
    Ok(InstalledOperation::Recurrence(RecurrenceOperation {
        result,
        emitted: false,
    }))
}
