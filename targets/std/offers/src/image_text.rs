//! Exact bounded image-plus-text composition offered by the hosted std Host.

use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    HostCallContractId, HostCallRequirement, ImplementationId, Kind,
};

pub const IMAGE_TEXT_STD_PROFILE: &str = "std/image-text-kernel-hosted@1";
pub const IMAGE_TEXT_STD_IMPLEMENTATION: &str = "std/kernel-image-text@1";
pub const IMAGE_TEXT_STD_ARTIFACT: &str = "conduit-human/image-text@1";
pub const IMAGE_TEXT_IMAGE_OPERATION: &str = "conduit.host/image-text-image@1";
pub const IMAGE_TEXT_CAPTION_OPERATION: &str = "conduit.host/image-text-caption@1";
pub const IMAGE_TEXT_RECORD_STD_PROFILE: &str = "std/image-text-record-kernel-hosted@1";
pub const IMAGE_TEXT_RECORD_STD_IMPLEMENTATION: &str = "std/kernel-image-text-record@1";
pub const IMAGE_TEXT_RECORD_STD_ARTIFACT: &str = "conduit-net/typed-record@1";
pub const IMAGE_TEXT_RECORD_OPERATION: &str = "conduit.host/image-text-record@1";

pub fn image_text_std_offer() -> CapabilityOffer {
    let operation = |contract, maximum_input_bytes| HostCallRequirement {
        contract_id: HostCallContractId::from(contract),
        target_kind: Some(conduit_core::kind_id(
            conduit_semantic_catalog::IMAGE_TEXT_COMPOSE_KIND,
        )),
        maximum_in_flight: 1,
        maximum_input_bytes,
        maximum_output_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
    };
    offer(
        conduit_semantic_catalog::image_text_compose_semantic_contract(),
        "std-image-text-v1",
        IMAGE_TEXT_STD_PROFILE,
        IMAGE_TEXT_STD_IMPLEMENTATION,
        IMAGE_TEXT_STD_ARTIFACT,
        vec![
            operation(
                IMAGE_TEXT_CAPTION_OPERATION,
                conduit_human::MAXIMUM_IMAGE_TEXT_CAPTION_BYTES as u32,
            ),
            operation(
                IMAGE_TEXT_IMAGE_OPERATION,
                conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            ),
        ],
    )
}

pub fn image_text_record_std_offer() -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::image_text_typed_record_semantic_contract(),
        "std-image-text-record-v1",
        IMAGE_TEXT_RECORD_STD_PROFILE,
        IMAGE_TEXT_RECORD_STD_IMPLEMENTATION,
        IMAGE_TEXT_RECORD_STD_ARTIFACT,
        vec![HostCallRequirement {
            contract_id: HostCallContractId::from(IMAGE_TEXT_RECORD_OPERATION),
            target_kind: Some(conduit_core::kind_id(
                conduit_semantic_catalog::IMAGE_TEXT_TYPED_RECORD_KIND,
            )),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_net::MAXIMUM_TYPED_RECORD_PAYLOAD_BYTES as u32,
            maximum_output_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        }],
    )
}

fn offer(
    contract: Kind,
    capability: &str,
    profile: &str,
    implementation: &str,
    artifact: &str,
    host_calls: Vec<HostCallRequirement>,
) -> CapabilityOffer {
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(capability),
            execution_profile_id: ExecutionProfileId::from(profile),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(artifact),
            host_calls,
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offer_preserves_the_portable_front_and_finite_bounds() {
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
        assert_eq!(offer.host_calls.len(), 2);
        assert_eq!(offer.limits.max_queue_items, 2);
        assert!(offer.authority_requirements.is_empty());
        assert!(offer.resource_requirements.is_empty());
        let adapter = image_text_record_std_offer();
        assert_eq!(adapter.inputs[0].port_id.as_str(), "record");
        assert_eq!(adapter.outputs[0].port_id.as_str(), "typed");
    }
}
