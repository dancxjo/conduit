//! Immutable reviewed clock implementation metadata; no live provider or authority.
use super::contract::{MonotonicClockContract, CLOCK_CALL, CLOCK_MAXIMUM_BYTES};
use alloc::vec;
use conduit_core::*;

pub const CLOCK_IMPLEMENTATION: &str = "conduitos/monotonic-clock-at@1";
pub const CLOCK_EXECUTION_PROFILE: &str = "conduitos/monotonic-clock-bounded@1";
pub const CLOCK_RESOURCE_CLASS: &str = "machine/monotonic-clock";
pub const CLOCK_AUTHORITY: &str = "conduitos.authority/monotonic-clock-at@1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockOfferRefusal {
    InvalidArtifact,
}

/// Fabrication metadata only. A retained provider and original admission must
/// independently establish current Host/Boot, resources and effect authority.
pub fn capability_offer(
    contract: &MonotonicClockContract,
    artifact: &ArtifactId,
) -> Result<CapabilityOffer, ClockOfferRefusal> {
    if artifact.as_str().is_empty() {
        return Err(ClockOfferRefusal::InvalidArtifact);
    }
    let kind = contract.kind();
    let capability = conduit_core::capability_offer_from_parts! {
        semantic_contract: kind.semantic_contract(),
        startup_parameters: kind.startup_parameters.clone(),
        shorthand: kind.shorthand.clone(),
        capability_id: CapabilityId::from("conduitos/monotonic-clock-at@1"),
        kind_id: kind.kind_id.clone(),
        kind_contract_revision: kind.kind_contract_revision.clone(),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(CLOCK_EXECUTION_PROFILE),
            implementation_id: ImplementationId::from(CLOCK_IMPLEMENTATION),
            artifact_id: artifact.clone(),
        },
        inputs: kind.inputs.clone(),
        outputs: kind.outputs.clone(),
        host_calls: vec![HostCallRequirement {
            contract_id: HostCallContractId::from(CLOCK_CALL),
            target_kind: Some(kind.kind_id.clone()),
            maximum_in_flight: 1,
            maximum_input_bytes: CLOCK_MAXIMUM_BYTES,
            maximum_output_bytes: CLOCK_MAXIMUM_BYTES,
        }],
        resource_requirements: vec![resource_requirement(CLOCK_RESOURCE_CLASS, 1)],
        authority_requirements: vec![AuthorityRequirement {
            contract_id: AuthorityContractId::from(CLOCK_AUTHORITY),
            host_call_contract_id: HostCallContractId::from(CLOCK_CALL),
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
    fn definition_preserves_exact_contract_and_artifact_without_ready_custody() {
        let contract = MonotonicClockContract::prepare().unwrap();
        assert_eq!(
            capability_offer(&contract, &ArtifactId::from("")),
            Err(ClockOfferRefusal::InvalidArtifact)
        );
        let artifact = ArtifactId::from("fixture/clock-image");
        let offer = capability_offer(&contract, &artifact).unwrap();
        offer.validate_constructed_semantic_contract().unwrap();
        assert_eq!(offer.inputs, contract.kind().inputs);
        assert_eq!(offer.outputs, contract.kind().outputs);
        assert_eq!(offer.limits, contract.kind().limits);
        assert_eq!(offer.implementation.artifact_id, artifact);
        assert_eq!(
            offer.implementation.implementation_id.as_str(),
            CLOCK_IMPLEMENTATION
        );
        assert_eq!(offer.host_calls[0].contract_id.as_str(), CLOCK_CALL);
        assert_eq!(
            offer.resource_requirements,
            vec![resource_requirement(CLOCK_RESOURCE_CLASS, 1)]
        );
        assert_eq!(
            offer.authority_requirements[0].contract_id.as_str(),
            CLOCK_AUTHORITY
        );
        let other_artifact = ArtifactId::from("fixture/other-clock-image");
        let other = capability_offer(&contract, &other_artifact).unwrap();
        let mut expected = offer;
        expected.implementation.artifact_id = other_artifact;
        assert_eq!(other, expected);
    }
}
