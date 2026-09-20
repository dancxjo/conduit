//! Hosted std realizations of the transport-neutral typed-record codecs.

use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    HostOperationContractId, HostOperationRequirement, ImplementationId,
};

pub const TYPED_RECORD_FRAME_STD_IMPLEMENTATION: &str = "std/typed-record-frame@1";
pub const TYPED_RECORD_DEFRAME_STD_IMPLEMENTATION: &str = "std/typed-record-deframe@1";
pub const TEXT_TO_TYPED_RECORD_STD_IMPLEMENTATION: &str = "std/text-to-typed-record@1";
pub const TYPED_RECORD_TO_TEXT_STD_IMPLEMENTATION: &str = "std/typed-record-to-text@1";

pub const TYPED_RECORD_FRAME_HOST_OPERATION: &str = "conduit.host/typed-record-frame@1";
pub const TYPED_RECORD_DEFRAME_HOST_OPERATION: &str = "conduit.host/typed-record-deframe@1";
pub const TEXT_TO_TYPED_RECORD_HOST_OPERATION: &str = "conduit.host/text-to-typed-record@1";
pub const TYPED_RECORD_TO_TEXT_HOST_OPERATION: &str = "conduit.host/typed-record-to-text@1";

pub fn typed_record_frame_std_offer() -> CapabilityOffer {
    codec_offer(
        conduit_net::TYPED_RECORD_FRAME_KIND,
        "std-typed-record-frame-v1",
        TYPED_RECORD_FRAME_STD_IMPLEMENTATION,
        TYPED_RECORD_FRAME_HOST_OPERATION,
    )
}

pub fn typed_record_deframe_std_offer() -> CapabilityOffer {
    codec_offer(
        conduit_net::TYPED_RECORD_DEFRAME_KIND,
        "std-typed-record-deframe-v1",
        TYPED_RECORD_DEFRAME_STD_IMPLEMENTATION,
        TYPED_RECORD_DEFRAME_HOST_OPERATION,
    )
}

pub fn text_to_typed_record_std_offer() -> CapabilityOffer {
    codec_offer(
        conduit_net::TEXT_TO_TYPED_RECORD_KIND,
        "std-text-to-typed-record-v1",
        TEXT_TO_TYPED_RECORD_STD_IMPLEMENTATION,
        TEXT_TO_TYPED_RECORD_HOST_OPERATION,
    )
}

pub fn typed_record_to_text_std_offer() -> CapabilityOffer {
    codec_offer(
        conduit_net::TYPED_RECORD_TO_TEXT_KIND,
        "std-typed-record-to-text-v1",
        TYPED_RECORD_TO_TEXT_STD_IMPLEMENTATION,
        TYPED_RECORD_TO_TEXT_HOST_OPERATION,
    )
}

pub fn typed_record_codec_offers() -> [CapabilityOffer; 4] {
    [
        typed_record_frame_std_offer(),
        typed_record_deframe_std_offer(),
        text_to_typed_record_std_offer(),
        typed_record_to_text_std_offer(),
    ]
}

fn codec_offer(
    kind: &str,
    capability: &str,
    implementation: &str,
    operation: &str,
) -> CapabilityOffer {
    let contract = conduit_net::typed_record_semantic_contract(kind)
        .expect("typed-record codec semantic contract exists");
    let target_kind = contract.kind_id.clone();
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(capability),
            execution_profile_id: ExecutionProfileId::from("std/typed-record-codec-hosted@1"),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from("conduit-net/typed-record-codecs@1"),
            host_operations: vec![HostOperationRequirement {
                contract_id: HostOperationContractId::from(operation),
                target_kind: Some(target_kind),
                maximum_in_flight: 1,
                maximum_input_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
                maximum_output_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            }],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}
