//! Hosted std realization offers for deterministic education, vision, and robotics specimens.

use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    HostCallContractId, HostCallRequirement, ImplementationId, Kind,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};

pub const EDUCATION_PROFILE: &str = "std/education-assessment-hosted@1";
pub const EDUCATION_ARTIFACT: &str = "conduit-std-host/education-assessment@1";
pub const EDUCATION_HOST_CALL: &str = "conduit.host/education-deterministic@1";
pub const VISION_PROFILE: &str = "std/vision-metadata-hosted@1";
pub const VISION_ARTIFACT: &str = "conduit-std-host/vision-metadata@1";
pub const VISION_HOST_CALL: &str = "conduit.host/vision-deterministic@1";
pub const ROBOTICS_STRUCTURED_PROFILE: &str = "std/robotics-structured-deterministic@1";
pub const ROBOTICS_STRUCTURED_ARTIFACT: &str = "conduit-std-host/robotics-structured@1";
pub const ROBOTICS_STRUCTURED_HOST_CALL: &str = "conduit.host/robotics-structured-deterministic@1";

pub fn education_std_offers() -> Vec<CapabilityOffer> {
    conduit_semantic_catalog::education_semantic_contracts()
        .into_iter()
        .map(|contract| {
            offer(
                contract,
                EDUCATION_PROFILE,
                EDUCATION_ARTIFACT,
                EDUCATION_HOST_CALL,
            )
        })
        .collect()
}

pub fn vision_std_offers() -> Vec<CapabilityOffer> {
    conduit_semantic_catalog::vision_semantic_contracts()
        .into_iter()
        .map(|contract| offer(contract, VISION_PROFILE, VISION_ARTIFACT, VISION_HOST_CALL))
        .collect()
}

pub fn robotics_structured_deterministic_offers() -> Vec<CapabilityOffer> {
    conduit_robotics::robotics_structured_semantic_contracts()
        .into_iter()
        .filter(|contract| {
            contract.kind_id.as_str() != conduit_robotics::ROBOTICS_EXECUTE_MOTION_KIND
        })
        .map(|contract| {
            offer(
                contract,
                ROBOTICS_STRUCTURED_PROFILE,
                ROBOTICS_STRUCTURED_ARTIFACT,
                ROBOTICS_STRUCTURED_HOST_CALL,
            )
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn offer(contract: Kind, profile: &str, artifact: &str, host_call: &str) -> CapabilityOffer {
    let maximum_input_bytes = if contract.inputs.is_empty() {
        0
    } else {
        MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32
    };
    let maximum_output_bytes = if contract.outputs.is_empty() {
        0
    } else {
        MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32
    };
    let kind = contract.kind_id.clone();
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(format!("{profile}/{}", kind.as_str())),
            execution_profile_id: ExecutionProfileId::from(profile),
            implementation_id: ImplementationId::from(format!("{profile}/{}", kind.as_str())),
            artifact_id: ArtifactId::from(artifact),
            host_calls: vec![HostCallRequirement {
                contract_id: HostCallContractId::from(host_call),
                target_kind: Some(kind),
                maximum_in_flight: 1,
                maximum_input_bytes,
                maximum_output_bytes,
            }],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_exact_semantics(offers: &[CapabilityOffer], contracts: &[Kind]) {
        for offer in offers {
            let contract = contracts
                .iter()
                .find(|contract| contract.kind_id == offer.kind_id)
                .expect("every hosted specimen has a portable contract");
            assert_eq!(offer.startup_parameters, contract.startup_parameters);
            assert_eq!(offer.shorthand, contract.shorthand);
            assert_eq!(
                offer.kind_contract_revision,
                contract.kind_contract_revision
            );
            assert_eq!(offer.inputs, contract.inputs);
            assert_eq!(offer.outputs, contract.outputs);
            assert_eq!(offer.limits, contract.limits);
        }
    }

    #[test]
    fn hosted_offers_preserve_portable_fronts_without_exporting_motion_authority() {
        let education = education_std_offers();
        let vision = vision_std_offers();
        let robotics = robotics_structured_deterministic_offers();
        assert_eq!(education.len(), 3);
        assert_eq!(vision.len(), 9);
        assert_eq!(robotics.len(), 2);
        assert_exact_semantics(
            &education,
            &conduit_semantic_catalog::education_semantic_contracts(),
        );
        assert_exact_semantics(
            &vision,
            &conduit_semantic_catalog::vision_semantic_contracts(),
        );
        assert_exact_semantics(
            &robotics,
            &conduit_robotics::robotics_structured_semantic_contracts(),
        );
        assert!(robotics.iter().all(|offer| {
            offer.kind_id.as_str() != conduit_robotics::ROBOTICS_EXECUTE_MOTION_KIND
                && offer.authority_requirements.is_empty()
        }));
        assert!(vision.iter().all(|offer| {
            offer.limits.max_active_instances == 1 && offer.limits.max_queue_items == 1
        }));
    }
}
