//! Exact native canonical document state over the protected retained owner.
use super::WorksetRefusal;
use alloc::{format, vec};
use conduit_core::{
    Back, BackOfferBuilder, CapabilityOffer, HostCallRequirement, PlannedGear, resource_requirement,
};
use conduit_kernel::FailureCode;
use conduit_semantic_catalog::TextEditRefusal;

pub(crate) const IMPLEMENTATION: &str = "conduitos/kernel-text-edit-document@1";
pub(crate) const HOST_CALL: &str = "conduit.host/conduitos-text-edit-document@1";

pub(super) fn offer(build_id: &str) -> CapabilityOffer {
    BackOfferBuilder::new(
        conduit_semantic_catalog::text_edit_document_semantic_contract(),
        Back {
            capability_id: IMPLEMENTATION.into(),
            execution_profile_id: "conduitos/bounded-text-document@1".into(),
            implementation_id: IMPLEMENTATION.into(),
            artifact_id: format!("conduitos-build/{build_id}").into(),
            host_calls: vec![HostCallRequirement {
                contract_id: HOST_CALL.into(),
                target_kind: Some("text/bounded-document-state-output@1".into()),
                maximum_in_flight: 1,
                maximum_input_bytes: conduit_semantic_catalog::MAXIMUM_TEXT_EDIT_BYTES,
                maximum_output_bytes: conduit_semantic_catalog::MAXIMUM_TEXT_DOCUMENT_BYTES,
            }],
            resource_requirements: vec![resource_requirement(
                "conduit.resource/runtime-memory@1",
                super::text_state::reserved_memory_bytes(),
            )],
            authority_requirements: vec![],
        },
    )
    .build()
}

pub(super) fn validate(placement: &PlannedGear) -> Result<usize, WorksetRefusal> {
    let expected = offer("");
    if placement.kind_id.as_str() != conduit_semantic_catalog::TEXT_EDIT_DOCUMENT_KIND
        || placement.capability_id.as_str() != IMPLEMENTATION
        || placement.kind_contract_revision != expected.kind_contract_revision
        || placement.execution_profile_id != expected.implementation.execution_profile_id
        || placement.implementation_id != expected.implementation.implementation_id
        || placement.inputs != expected.inputs
        || placement.outputs != expected.outputs
        || placement.limits != expected.limits
        || placement.semantic_contract != expected.semantic_contract
        || placement.host_calls != expected.host_calls
        || !placement.authority.is_empty()
        || placement.configuration.len() != 1
    {
        return Err(WorksetRefusal::Plan);
    }
    placement
        .configuration
        .iter()
        .find_map(|field| match (&*field.key, &field.value) {
            ("maximum-bytes", conduit_core::ConfigurationValue::U64(maximum))
                if (1..=256).contains(maximum) =>
            {
                usize::try_from(*maximum).ok()
            }
            _ => None,
        })
        .ok_or(WorksetRefusal::Plan)
}

pub(super) fn failure(refusal: TextEditRefusal) -> (FailureCode, u16) {
    use TextEditRefusal::*;
    let code = match refusal {
        CapacityExhausted => FailureCode::StateCapacityExhausted,
        RevisionExhausted => FailureCode::IdentityCapacityExhausted,
        InvalidStorage | UnsupportedMode => FailureCode::InvalidLifecycle,
        _ => FailureCode::InvalidInput,
    };
    let detail = match refusal {
        StaleRevision => 90,
        InvalidEncoding => 91,
        InvalidRange => 92,
        InvalidUtf8 => 93,
        CapacityExhausted => 94,
        RevisionExhausted => 95,
        InvalidStorage => 96,
        UnsupportedMode => 97,
    };
    (code, detail)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refusal_categories_and_details_keep_revision_range_and_capacity_distinct() {
        assert_eq!(
            failure(TextEditRefusal::StaleRevision),
            (FailureCode::InvalidInput, 90)
        );
        assert_eq!(
            failure(TextEditRefusal::CapacityExhausted),
            (FailureCode::StateCapacityExhausted, 94)
        );
        assert_eq!(
            failure(TextEditRefusal::RevisionExhausted),
            (FailureCode::IdentityCapacityExhausted, 95)
        );
        assert_ne!(
            failure(TextEditRefusal::InvalidRange),
            failure(TextEditRefusal::InvalidUtf8)
        );
    }
}
