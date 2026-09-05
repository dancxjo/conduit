//! Exact bounded image-plus-text composition offered by the hosted std Host.

use conduit_core::{
    ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
    HostOperationContractId, HostOperationRequirement, ImplementationId, ImplementationOffer,
    KindContractRevision, PortDescriptor,
};

pub const IMAGE_TEXT_STD_PROFILE: &str = "std/image-text-kernel-hosted@1";
pub const IMAGE_TEXT_STD_IMPLEMENTATION: &str = "std/kernel-image-text@1";
pub const IMAGE_TEXT_STD_ARTIFACT: &str = "conduit-human/image-text@1";
pub const IMAGE_TEXT_IMAGE_OPERATION: &str = "conduit.host/image-text-image@1";
pub const IMAGE_TEXT_CAPTION_OPERATION: &str = "conduit.host/image-text-caption@1";

pub fn image_text_std_offer() -> CapabilityOffer {
    let inputs = vec![
        structured_port(
            "image",
            &conduit_semantic_catalog::image_observation_reference_type(),
        ),
        PortDescriptor {
            port_id: conduit_core::port_id("caption"),
            value_kind: conduit_core::kind_id("value/text@1"),
            direction: conduit_core::PortDirection::Input,
            temporal: conduit_core::PortTemporal::Value,
        },
    ];
    let outputs = vec![PortDescriptor {
        port_id: conduit_core::port_id("record"),
        value_kind: conduit_semantic_catalog::image_text_record_type()
            .profile()
            .expect("image-text record has an exact profile")
            .value_kind()
            .clone(),
        direction: conduit_core::PortDirection::Output,
        temporal: conduit_core::PortTemporal::Value,
    }];
    let operation = |contract, maximum_input_bytes| HostOperationRequirement {
        contract_id: HostOperationContractId::from(contract),
        target_kind: Some(conduit_core::kind_id(
            conduit_semantic_catalog::IMAGE_TEXT_COMPOSE_KIND,
        )),
        maximum_in_flight: 1,
        maximum_input_bytes,
        maximum_output_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
    };
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from("std-image-text-v1"),
        kind_id: conduit_core::kind_id(conduit_semantic_catalog::IMAGE_TEXT_COMPOSE_KIND),
        kind_contract_revision: KindContractRevision::from(
            conduit_semantic_catalog::IMAGE_TEXT_COMPOSE_REVISION,
        ),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(IMAGE_TEXT_STD_PROFILE),
            implementation_id: ImplementationId::from(IMAGE_TEXT_STD_IMPLEMENTATION),
            artifact_id: ArtifactId::from(IMAGE_TEXT_STD_ARTIFACT),
        },
        inputs,
        outputs,
        host_operations: vec![
            operation(
                IMAGE_TEXT_IMAGE_OPERATION,
                conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            ),
            operation(
                IMAGE_TEXT_CAPTION_OPERATION,
                conduit_human::MAXIMUM_IMAGE_TEXT_CAPTION_BYTES as u32,
            ),
        ],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 2,
            max_queue_bytes: (conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
                + conduit_human::MAXIMUM_IMAGE_TEXT_CAPTION_BYTES)
                as u32,
        },
    }
}

fn structured_port(name: &str, value_type: &conduit_core::StructuredInfoType) -> PortDescriptor {
    PortDescriptor {
        port_id: conduit_core::port_id(name),
        value_kind: value_type
            .profile()
            .expect("image observation has an exact profile")
            .value_kind()
            .clone(),
        direction: conduit_core::PortDirection::Input,
        temporal: conduit_core::PortTemporal::Value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offer_preserves_the_portable_face_and_finite_bounds() {
        let offer = image_text_std_offer();
        assert_eq!(
            offer.kind_id.as_str(),
            conduit_semantic_catalog::IMAGE_TEXT_COMPOSE_KIND
        );
        assert_eq!(
            offer
                .inputs
                .iter()
                .map(|port| port.port_id.as_str())
                .collect::<Vec<_>>(),
            ["image", "caption"]
        );
        assert_eq!(offer.outputs[0].port_id.as_str(), "record");
        assert_eq!(offer.host_operations.len(), 2);
        assert_eq!(offer.limits.max_queue_items, 2);
        assert!(offer.authority_requirements.is_empty());
        assert!(offer.resource_requirements.is_empty());
    }
}
