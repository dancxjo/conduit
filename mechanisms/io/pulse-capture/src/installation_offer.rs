//! Immutable pulse Back definition, independent of a live pin or provider.
use super::*;

/// Fabrication may encode this definition once in flash. This does not publish
/// a Ready Base, create a Host/Boot, reserve storage or grant a Host Call.
/// Publication still requires the actual initialized provider's exact identity.
pub fn capability_offer(
    contract: &PulseReadContract,
    artifact: &ArtifactId,
    implementation: &ImplementationId,
) -> Result<CapabilityOffer, PulseInstallationRefusal> {
    if artifact.as_str().is_empty() || implementation.as_str().is_empty() {
        return Err(PulseInstallationRefusal::InvalidIdentity);
    }
    let kind = contract.kind();
    Ok(BackOfferBuilder::new(
        kind.clone(),
        Back {
            capability_id: PULSE_CAPABILITY.into(),
            execution_profile_id: PULSE_EXECUTION_PROFILE.into(),
            implementation_id: implementation.clone(),
            artifact_id: artifact.clone(),
            host_calls: vec![HostCallRequirement {
                contract_id: PULSE_READ_CALL.into(),
                target_kind: Some(kind.kind_id.clone()),
                maximum_in_flight: 1,
                maximum_input_bytes: PULSE_READ_MAXIMUM_BYTES,
                maximum_output_bytes: PULSE_READ_MAXIMUM_BYTES,
            }],
            resource_requirements: vec![resource_requirement(PULSE_ATTACHMENT_CLASS, 1)],
            authority_requirements: vec![AuthorityRequirement {
                contract_id: PULSE_AUTHORITY.into(),
                host_call_contract_id: PULSE_READ_CALL.into(),
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
    fn definition_keeps_typed_fore_and_exact_implementation_without_a_provider() {
        let contract = PulseReadContract::prepare().unwrap();
        let artifact = ArtifactId::from("fixture/pulse-image");
        let implementation = ImplementationId::from("fixture/pulse-implementation");
        let offer = capability_offer(&contract, &artifact, &implementation).unwrap();
        offer.validate_constructed_semantic_contract().unwrap();
        assert_eq!(offer.inputs, contract.kind().inputs);
        assert_eq!(offer.outputs, contract.kind().outputs);
        assert_eq!(offer.semantic_contract.value_contracts().len(), 2);
        for value in offer.semantic_contract.value_contracts() {
            assert!(value.contract.maximum_bytes <= PULSE_READ_MAXIMUM_BYTES);
            assert!(value.contract.maximum_bytes > 64);
        }

        assert_eq!(offer.implementation.artifact_id, artifact);
        assert_eq!(offer.implementation.implementation_id, implementation);
        assert_eq!(offer.host_calls.len(), 1);
        assert_eq!(offer.host_calls[0].contract_id.as_str(), PULSE_READ_CALL);
        assert_eq!(offer.host_calls[0].maximum_in_flight, 1);
        assert_eq!(
            offer.resource_requirements,
            vec![resource_requirement(PULSE_ATTACHMENT_CLASS, 1)]
        );
        assert_eq!(
            offer.authority_requirements[0].contract_id.as_str(),
            PULSE_AUTHORITY
        );
        assert_eq!(
            capability_offer(&contract, &"".into(), &implementation),
            Err(PulseInstallationRefusal::InvalidIdentity)
        );
        assert_eq!(
            capability_offer(&contract, &artifact, &"".into()),
            Err(PulseInstallationRefusal::InvalidIdentity)
        );
        let other =
            capability_offer(&contract, &artifact, &"fixture/other-provider".into()).unwrap();
        assert_ne!(other.implementation, offer.implementation);
        assert_eq!(other.inputs, offer.inputs);
        assert_eq!(other.authority_requirements, offer.authority_requirements);
    }
}
