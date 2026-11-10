//! Immutable implementation metadata for fabrication, not provider readiness.
use super::*;

/// Construct the reviewed I2C implementation's ordinary offer definition.
///
/// A fabrication host may encode these bytes once into a Seed. This neither
/// owns nor publishes a Base, identifies a live Host/Boot, reserves a resource,
/// nor grants a Host Call capability. Runtime publication must independently
/// establish current initialized provider custody and exact artifact identity.
pub fn capability_offer(
    contract: &I2cContract,
    artifact: &ArtifactId,
) -> Result<CapabilityOffer, I2cInstallationRefusal> {
    if artifact.as_str().is_empty() {
        return Err(I2cInstallationRefusal::InvalidIdentity);
    }
    let kind = contract.kind();
    let capability = conduit_core::capability_offer_from_parts! {
        semantic_contract: kind.semantic_contract(),
        startup_parameters: kind.startup_parameters.clone(),
        shorthand: kind.shorthand.clone(),
        capability_id: CapabilityId::from("conduitos/i2c-transaction@1"),
        kind_id: kind.kind_id.clone(),
        kind_contract_revision: kind.kind_contract_revision.clone(),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(I2C_EXECUTION_PROFILE),
            implementation_id: ImplementationId::from(I2C_IMPLEMENTATION),
            artifact_id: artifact.clone(),
        },
        inputs: kind.inputs.clone(),
        outputs: kind.outputs.clone(),
        host_calls: vec![HostCallRequirement {
            contract_id: HostCallContractId::from(I2C_CALL),
            target_kind: Some(kind.kind_id.clone()),
            maximum_in_flight: 1,
            maximum_input_bytes: I2C_MAXIMUM_BYTES,
            maximum_output_bytes: I2C_MAXIMUM_BYTES,
        }],
        resource_requirements: vec![resource_requirement(I2C_ATTACHMENT_CLASS, 1)],
        authority_requirements: vec![AuthorityRequirement {
            contract_id: AuthorityContractId::from(I2C_AUTHORITY),
            host_call_contract_id: HostCallContractId::from(I2C_CALL),
            subject_kind: kind.kind_id.clone(),
        }],
        limits: kind.limits.clone(),
    };
    Ok(capability)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fabrication_definition_preserves_fore_and_requires_exact_artifact() {
        let contract = I2cContract::prepare().unwrap();
        assert_eq!(
            capability_offer(&contract, &ArtifactId::from("")),
            Err(I2cInstallationRefusal::InvalidIdentity)
        );
        let artifact = ArtifactId::from("fixture/checked-i2c-image");
        let offer = capability_offer(&contract, &artifact).unwrap();
        offer.validate_constructed_semantic_contract().unwrap();
        assert_eq!(offer.kind_id, contract.kind().kind_id);
        assert_eq!(offer.inputs, contract.kind().inputs);
        assert_eq!(offer.outputs, contract.kind().outputs);
        assert_eq!(offer.implementation.artifact_id, artifact);
        assert_eq!(offer.host_calls.len(), 1);
        assert_eq!(offer.host_calls[0].contract_id.as_str(), I2C_CALL);
        assert_eq!(
            offer.resource_requirements,
            vec![resource_requirement(I2C_ATTACHMENT_CLASS, 1)]
        );
        assert_eq!(offer.authority_requirements.len(), 1);
        assert_eq!(
            offer.authority_requirements[0].contract_id.as_str(),
            I2C_AUTHORITY
        );
        // Definition construction has no Host, Boot, provider, table or handle.
        // A different checked image changes only its implementation artifact.
        let other = capability_offer(&contract, &ArtifactId::from("fixture/other-image")).unwrap();
        assert_ne!(
            other.implementation.artifact_id,
            offer.implementation.artifact_id
        );
        assert_eq!(other.inputs, offer.inputs);
        assert_eq!(other.authority_requirements, offer.authority_requirements);
    }
}
