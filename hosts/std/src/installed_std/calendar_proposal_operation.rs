//! Preparation-time calendar evaluation and one ordinary-kernel emission.

use super::calendar_proposal_codec;
use super::calendar_proposal_encoding;
use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{ConfigurationValue, PlannedGear, StructuredInfoValue};
use conduit_kernel::{OperationAction, OperationInput, PortId, ValueRef, ValueStorage};

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_catalog::CALENDAR_PROPOSAL_STD_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct CalendarProposalOperation {
    result: ValueRef,
    emitted: bool,
}

impl CalendarProposalOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Emit {
            port: PortId(0),
            value: self.result,
        }
    }

    pub(super) fn resume(&mut self, _input: OperationInput) -> OperationAction {
        InstalledOperation::fail(230)
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        if self.emitted {
            InstalledOperation::fail(231)
        } else {
            self.emitted = true;
            OperationAction::Complete
        }
    }
}

fn request(
    placement: &PlannedGear,
) -> Result<calendar_proposal_codec::DecodedCalendarProposal, String> {
    let [entry] = placement.configuration.as_slice() else {
        return Err("calendar proposal requires one exact planned request".into());
    };
    let ("request", ConfigurationValue::Structured(configuration)) =
        (entry.key.as_str(), &entry.value)
    else {
        return Err("calendar proposal planned request is malformed".into());
    };
    let value = StructuredInfoValue::from_canonical_bytes(configuration.canonical_value())
        .map_err(|error| format!("decode planned calendar request: {error:?}"))?;
    let expected = conduit_std_catalog::calendar_proposal_request_type();
    if value.value_type() != &expected
        || configuration.profile()
            != expected
                .profile()
                .map_err(|error| format!("profile calendar request: {error:?}"))?
                .value_kind()
    {
        return Err("calendar proposal planned request type/profile mismatch".into());
    }
    calendar_proposal_codec::decode(&value)
}

fn validate(
    placement: &PlannedGear,
) -> Result<calendar_proposal_codec::DecodedCalendarProposal, String> {
    let offer = conduit_std_catalog::calendar_proposal_std_offer();
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
        || !offer.resource_requirements.is_empty()
        || !offer.authority_requirements.is_empty()
    {
        return Err("planned calendar proposal differs from installed realization".into());
    }
    let decoded = request(placement)?;
    if decoded.request.participant_identities.len()
        > usize::from(conduit_std_catalog::CALENDAR_PROPOSAL_MAXIMUM_PARTICIPANTS)
        || decoded.request.candidates.len()
            > usize::from(conduit_std_catalog::CALENDAR_PROPOSAL_MAXIMUM_CANDIDATES)
        || decoded.availability.len()
            > usize::from(conduit_std_catalog::CALENDAR_PROPOSAL_MAXIMUM_PARTICIPANTS)
        || decoded.availability.iter().any(|participant| {
            participant.intervals.len()
                > usize::from(conduit_std_catalog::CALENDAR_PROPOSAL_MAXIMUM_INTERVALS)
        })
        || decoded.request.maximum_results > conduit_std_catalog::CALENDAR_PROPOSAL_MAXIMUM_RESULTS
    {
        return Err("calendar proposal exceeds installed profile".into());
    }
    Ok(decoded)
}

fn evaluate(placement: &PlannedGear) -> Result<Vec<u8>, String> {
    let decoded = validate(placement)?;
    let proposal = decoded
        .request
        .propose(&decoded.availability)
        .map_err(|error| format!("calendar proposal refusal: {error:?}"))?;
    calendar_proposal_encoding::encode(&proposal)
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    let encoded = evaluate(placement)?;
    let bytes = encoded
        .len()
        .try_into()
        .map_err(|_| "calendar proposal byte budget overflow")?;
    Ok(OperationBudget {
        value_items: 1,
        value_bytes: bytes,
        host_requests: 0,
        sign_items: 16,
        maximum_value_bytes: bytes,
    })
}

fn prepare(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    let encoded = evaluate(placement)?;
    let result = values
        .store(&encoded)
        .map_err(|error| format!("store calendar proposal: {error:?}"))?;
    Ok(InstalledOperation::CalendarProposal(
        CalendarProposalOperation {
            result,
            emitted: false,
        },
    ))
}
