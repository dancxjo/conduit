//! Hosted std realization offers for deterministic education, vision, and robotics specimens.

use conduit_core::{
    ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
    HostOperationContractId, HostOperationRequirement, ImplementationId, ImplementationOffer,
    KindContractRevision, KindId, PortDescriptor, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};

pub const EDUCATION_PROFILE: &str = "std/education-assessment-hosted@1";
pub const EDUCATION_ARTIFACT: &str = "conduit-std-host/education-assessment@1";
pub const EDUCATION_HOST_OPERATION: &str = "conduit.host/education-deterministic@1";
pub const VISION_PROFILE: &str = "std/vision-metadata-hosted@1";
pub const VISION_ARTIFACT: &str = "conduit-std-host/vision-metadata@1";
pub const VISION_HOST_OPERATION: &str = "conduit.host/vision-deterministic@1";
pub const ROBOTICS_STRUCTURED_PROFILE: &str = "std/robotics-structured-deterministic@1";
pub const ROBOTICS_STRUCTURED_ARTIFACT: &str = "conduit-std-host/robotics-structured@1";
pub const ROBOTICS_STRUCTURED_HOST_OPERATION: &str =
    "conduit.host/robotics-structured-deterministic@1";

pub fn education_std_offers() -> Vec<CapabilityOffer> {
    conduit_std_catalog::education_kind_contracts()
        .into_iter()
        .map(|(kind, inputs, outputs)| {
            offer(
                kind,
                inputs,
                outputs,
                conduit_std_catalog::EDUCATION_REVISION,
                EDUCATION_PROFILE,
                EDUCATION_ARTIFACT,
                EDUCATION_HOST_OPERATION,
                4,
                4,
            )
        })
        .collect()
}

pub fn vision_std_offers() -> Vec<CapabilityOffer> {
    conduit_std_catalog::vision_kind_contracts()
        .into_iter()
        .map(|(kind, inputs, outputs)| {
            offer(
                kind,
                inputs,
                outputs,
                conduit_std_catalog::VISION_REVISION,
                VISION_PROFILE,
                VISION_ARTIFACT,
                VISION_HOST_OPERATION,
                4,
                conduit_std_catalog::MAXIMUM_VISION_DETECTIONS,
            )
        })
        .collect()
}

pub fn robotics_structured_deterministic_offers() -> Vec<CapabilityOffer> {
    conduit_std_catalog::robotics_structured_kind_contracts()
        .into_iter()
        .filter(|(kind, _, _)| kind.as_str() != conduit_std_catalog::ROBOTICS_EXECUTE_MOTION_KIND)
        .map(|(kind, inputs, outputs)| {
            offer(
                kind,
                inputs,
                outputs,
                conduit_std_catalog::ROBOTICS_STRUCTURED_REVISION,
                ROBOTICS_STRUCTURED_PROFILE,
                ROBOTICS_STRUCTURED_ARTIFACT,
                ROBOTICS_STRUCTURED_HOST_OPERATION,
                1,
                1,
            )
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn offer(
    kind: KindId,
    inputs: Vec<PortDescriptor>,
    outputs: Vec<PortDescriptor>,
    revision: &str,
    profile: &str,
    artifact: &str,
    host_operation: &str,
    max_active_instances: u16,
    max_queue_items: u16,
) -> CapabilityOffer {
    let maximum_input_bytes = if inputs.is_empty() {
        0
    } else {
        MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32
    };
    let maximum_output_bytes = if outputs.is_empty() {
        0
    } else {
        MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32
    };
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from(format!("{profile}/{}", kind.as_str())),
        kind_id: kind.clone(),
        kind_contract_revision: KindContractRevision::from(revision),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(profile),
            implementation_id: ImplementationId::from(format!("{profile}/{}", kind.as_str())),
            artifact_id: ArtifactId::from(artifact),
        },
        inputs,
        outputs,
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(host_operation),
            target_kind: Some(kind),
            maximum_in_flight: 1,
            maximum_input_bytes,
            maximum_output_bytes,
        }],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances,
            max_queue_items,
            max_queue_bytes: u32::from(max_queue_items)
                .saturating_mul(MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosted_offers_preserve_portable_faces_without_exporting_motion_authority() {
        let education = education_std_offers();
        let vision = vision_std_offers();
        let robotics = robotics_structured_deterministic_offers();
        assert_eq!(education.len(), 3);
        assert_eq!(vision.len(), 2);
        assert_eq!(robotics.len(), 2);
        assert!(robotics.iter().all(|offer| {
            offer.kind_id.as_str() != conduit_std_catalog::ROBOTICS_EXECUTE_MOTION_KIND
                && offer.authority_requirements.is_empty()
        }));
    }
}
