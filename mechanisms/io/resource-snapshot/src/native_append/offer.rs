//! Immutable definition only; publishing requires the actual recovered owner.
use super::*;
use alloc::vec;
use conduit_core::*;
pub fn capability_offer(
    artifact: &ArtifactId,
    implementation: &ImplementationId,
) -> Result<CapabilityOffer, AppendCallRefusal> {
    if artifact.as_str().is_empty() || implementation.as_str().is_empty() {
        return Err(AppendCallRefusal::InvalidIdentity);
    }
    let kind = crate::append_contract::definition();
    Ok(BackOfferBuilder::new(
        kind.clone(),
        Back {
            capability_id: APPEND_CAPABILITY.into(),
            execution_profile_id: APPEND_PROFILE.into(),
            implementation_id: implementation.clone(),
            artifact_id: artifact.clone(),
            host_calls: vec![HostCallRequirement {
                contract_id: APPEND_OPERATION.into(),
                target_kind: Some(kind.kind_id.clone()),
                maximum_in_flight: 1,
                maximum_input_bytes: MAXIMUM_RECORD_PAYLOAD_BYTES as u32,
                maximum_output_bytes: APPEND_RESULT_BYTES,
            }],
            resource_requirements: vec![resource_requirement(APPEND_CLASS, 1)],
            authority_requirements: vec![AuthorityRequirement {
                contract_id: APPEND_AUTHORITY.into(),
                host_call_contract_id: APPEND_OPERATION.into(),
                subject_kind: kind.kind_id.clone(),
            }],
        },
    )
    .build())
}
