//! Hosted std realization of the transport-neutral typed-record frame.

use conduit_core::{
    ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
    HostOperationContractId, HostOperationRequirement, ImplementationId, ImplementationOffer,
    KindContractRevision,
};

pub const TYPED_RECORD_FRAME_STD_PROFILE: &str = "std/typed-record-frame-hosted@1";
pub const TYPED_RECORD_FRAME_STD_IMPLEMENTATION: &str = "std/typed-record-frame@1";
pub const TYPED_RECORD_FRAME_STD_ARTIFACT: &str = "conduit-net/typed-record-frame@1";
pub const TYPED_RECORD_FRAME_HOST_OPERATION: &str = "conduit.host/typed-record-frame@1";

pub fn typed_record_frame_std_offer() -> CapabilityOffer {
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_net::install_typed_record_catalogs(&mut startup, &mut profile)
        .expect("typed-record catalog is exact");
    let definition = profile
        .get(&conduit_core::kind_id(conduit_net::TYPED_RECORD_FRAME_KIND))
        .expect("frame definition exists");
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from("std-typed-record-frame-v1"),
        kind_id: definition.kind_id.clone(),
        kind_contract_revision: KindContractRevision::from(
            conduit_net::TYPED_RECORD_CONTRACT_REVISION,
        ),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(TYPED_RECORD_FRAME_STD_PROFILE),
            implementation_id: ImplementationId::from(TYPED_RECORD_FRAME_STD_IMPLEMENTATION),
            artifact_id: ArtifactId::from(TYPED_RECORD_FRAME_STD_ARTIFACT),
        },
        inputs: definition.inputs.clone(),
        outputs: definition.outputs.clone(),
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(TYPED_RECORD_FRAME_HOST_OPERATION),
            target_kind: Some(definition.kind_id.clone()),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            maximum_output_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        }],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        },
    }
}
