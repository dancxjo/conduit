//! Hosted std realizations of the transport-neutral typed-record codecs.

use conduit_core::{
    ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
    HostOperationContractId, HostOperationRequirement, ImplementationId, ImplementationOffer,
    KindContractRevision,
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
        conduit_net::TYPED_RECORD_CONTRACT_REVISION,
        "std-typed-record-frame-v1",
        TYPED_RECORD_FRAME_STD_IMPLEMENTATION,
        TYPED_RECORD_FRAME_HOST_OPERATION,
    )
}

pub fn typed_record_deframe_std_offer() -> CapabilityOffer {
    codec_offer(
        conduit_net::TYPED_RECORD_DEFRAME_KIND,
        conduit_net::TYPED_RECORD_CONTRACT_REVISION,
        "std-typed-record-deframe-v1",
        TYPED_RECORD_DEFRAME_STD_IMPLEMENTATION,
        TYPED_RECORD_DEFRAME_HOST_OPERATION,
    )
}

pub fn text_to_typed_record_std_offer() -> CapabilityOffer {
    codec_offer(
        conduit_net::TEXT_TO_TYPED_RECORD_KIND,
        conduit_net::TEXT_RECORD_CONTRACT_REVISION,
        "std-text-to-typed-record-v1",
        TEXT_TO_TYPED_RECORD_STD_IMPLEMENTATION,
        TEXT_TO_TYPED_RECORD_HOST_OPERATION,
    )
}

pub fn typed_record_to_text_std_offer() -> CapabilityOffer {
    codec_offer(
        conduit_net::TYPED_RECORD_TO_TEXT_KIND,
        conduit_net::TEXT_RECORD_CONTRACT_REVISION,
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
    revision: &str,
    capability: &str,
    implementation: &str,
    operation: &str,
) -> CapabilityOffer {
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_net::install_typed_record_catalogs(&mut startup, &mut profile)
        .expect("typed-record catalog is exact");
    let definition = profile
        .get(&conduit_core::kind_id(kind))
        .expect("typed-record codec definition exists");
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from(capability),
        kind_id: definition.kind_id.clone(),
        kind_contract_revision: KindContractRevision::from(revision),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("std/typed-record-codec-hosted@1"),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from("conduit-net/typed-record-codecs@1"),
        },
        inputs: definition.inputs.clone(),
        outputs: definition.outputs.clone(),
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(operation),
            target_kind: Some(definition.kind_id.clone()),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            maximum_output_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        }],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 4,
            max_queue_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32 * 4,
        },
    }
}
