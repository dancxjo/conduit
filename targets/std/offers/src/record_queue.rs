//! Pure-kernel std offer for bounded ordered framed-record queueing.

use conduit_core::{
    ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
    FaceStartupParameter, ImplementationId, ImplementationOffer, KindContractRevision,
};

pub const ORDERED_RECORD_QUEUE_STD_IMPLEMENTATION: &str = "std/ordered-record-queue@1";

pub fn ordered_record_queue_std_offer() -> CapabilityOffer {
    let definition = conduit_net::ordered_record_queue_kind_definition();
    CapabilityOffer {
        startup_parameters: vec![
            FaceStartupParameter {
                name: "maximum-items".into(),
                value_type: "Count".into(),
                has_default: true,
            },
            FaceStartupParameter {
                name: "maximum-frame-bytes".into(),
                value_type: "Count".into(),
                has_default: true,
            },
        ],
        shorthand: None,
        capability_id: CapabilityId::from(ORDERED_RECORD_QUEUE_STD_IMPLEMENTATION),
        kind_id: definition.kind_id,
        kind_contract_revision: KindContractRevision::from(
            conduit_net::ORDERED_RECORD_QUEUE_CONTRACT_REVISION,
        ),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("std/ordered-record-queue@1"),
            implementation_id: ImplementationId::from(ORDERED_RECORD_QUEUE_STD_IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-net/ordered-record-queue@1"),
        },
        inputs: definition.inputs,
        outputs: definition.outputs,
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: conduit_net::MAXIMUM_ORDERED_RECORD_QUEUE_ITEMS as u16,
            max_queue_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        },
    }
}
