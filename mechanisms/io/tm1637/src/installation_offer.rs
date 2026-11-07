//! Immutable native Back definition; no live provider, readiness or authority.
use super::*;
use alloc::vec;
/// Fabrication may retain this definition in flash. Publishing it requires an
/// independently initialized provider with the exact artifact/implementation.
pub fn capability_offer(
    contract: &WriteContract,
    artifact: &ArtifactId,
    implementation: &ImplementationId,
) -> Result<CapabilityOffer, InstallationRefusal> {
    if artifact.as_str().is_empty() || implementation.as_str().is_empty() {
        return Err(InstallationRefusal::InvalidIdentity);
    }
    let kind = contract.kind();
    Ok(BackOfferBuilder::new(
        kind.clone(),
        Back {
            capability_id: WRITE_CAPABILITY.into(),
            execution_profile_id: WRITE_EXECUTION_PROFILE.into(),
            implementation_id: implementation.clone(),
            artifact_id: artifact.clone(),
            host_calls: vec![HostCallRequirement {
                contract_id: WRITE_CALL.into(),
                target_kind: Some(kind.kind_id.clone()),
                maximum_in_flight: 1,
                maximum_input_bytes: MAXIMUM_BYTES,
                maximum_output_bytes: 0,
            }],
            resource_requirements: vec![resource_requirement(WRITE_ATTACHMENT_CLASS, 1)],
            authority_requirements: vec![AuthorityRequirement {
                contract_id: WRITE_AUTHORITY.into(),
                host_call_contract_id: WRITE_CALL.into(),
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
        let contract = WriteContract::prepare().unwrap();
        let artifact: ArtifactId = "test/native-image".into();
        let implementation: ImplementationId = "test/native-implementation".into();
        let offer = capability_offer(&contract, &artifact, &implementation).unwrap();
        offer.validate_constructed_semantic_contract().unwrap();
        assert_eq!(offer.implementation.artifact_id, artifact);
        assert_eq!(offer.implementation.implementation_id, implementation);
        assert_eq!(offer.host_calls.len(), 1);
        let values = offer.semantic_contract.value_contracts();
        assert_eq!(values.len(), 1);
        assert_eq!(
            values[0].location,
            conduit_core::FrontValueLocation::Input(conduit_core::port_id("packets"))
        );
        assert!(values[0].contract.maximum_bytes <= MAXIMUM_BYTES);
        assert!(values[0].contract.maximum_bytes > 64);
        assert_eq!(offer.host_calls[0].maximum_output_bytes, 0);

        assert!(capability_offer(&contract, &"".into(), &implementation).is_err());
        assert!(capability_offer(&contract, &artifact, &"".into()).is_err());
    }
}
