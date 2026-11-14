//! Immutable native Back definition; no live provider, readiness or authority.
use super::*;
/// Fabrication may retain this definition in flash. Publishing it requires an
/// independently initialized provider with the exact artifact/implementation.
pub fn capability_offer(
    artifact: &ArtifactId,
    implementation: &ImplementationId,
) -> Result<CapabilityOffer, WaitInstallationRefusal> {
    if artifact.as_str().is_empty() || implementation.as_str().is_empty() {
        return Err(WaitInstallationRefusal::InvalidIdentity);
    }
    let kind = conduit_time::tick_semantic_contract();
    Ok(BackOfferBuilder::new(
        kind.clone(),
        Back {
            capability_id: WAIT_CAPABILITY.into(),
            execution_profile_id: WAIT_EXECUTION_PROFILE.into(),
            implementation_id: implementation.clone(),
            artifact_id: artifact.clone(),
            host_calls: vec![HostCallRequirement {
                contract_id: WAIT_HOST_CALL_CONTRACT.into(),
                target_kind: Some(kind.kind_id.clone()),
                maximum_in_flight: 1,
                maximum_input_bytes: PARAMETER_BYTES,
                maximum_output_bytes: 0,
            }],
            resource_requirements: vec![resource_requirement(WAIT_ATTACHMENT_CLASS, 1)],
            authority_requirements: vec![AuthorityRequirement {
                contract_id: WAIT_AUTHORITY.into(),
                host_call_contract_id: WAIT_HOST_CALL_CONTRACT.into(),
                subject_kind: kind.kind_id.clone(),
            }],
        },
    )
    .build())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn immutable_definition_retains_exact_contract_and_refuses_empty_identity() {
        let artifact: ArtifactId = "test/native-image".into();
        let implementation: ImplementationId = "test/native-implementation".into();
        let offer = capability_offer(&artifact, &implementation).unwrap();
        offer.validate_constructed_semantic_contract().unwrap();
        assert_eq!(offer.implementation.artifact_id, artifact);
        assert_eq!(offer.implementation.implementation_id, implementation);
        assert_eq!(offer.host_calls.len(), 1);
        assert!(capability_offer(&"".into(), &implementation).is_err());
        assert!(capability_offer(&artifact, &"".into()).is_err());
    }
}
