//! Pure-kernel std offer for bounded typed-record transcript retention.

use conduit_core::{
    ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
    FaceStartupParameter, ImplementationId, ImplementationOffer, KindContractRevision,
};

pub const RECORD_TRANSCRIPT_STD_IMPLEMENTATION: &str = "std/bounded-record-transcript@1";

pub fn record_transcript_std_offer() -> CapabilityOffer {
    let definition = conduit_net::record_transcript_kind_definition();
    CapabilityOffer {
        startup_parameters: vec![
            parameter("maximum-items"),
            parameter("maximum-events"),
            parameter("maximum-frame-bytes"),
            parameter("maximum-retained-bytes"),
        ],
        shorthand: None,
        capability_id: CapabilityId::from(RECORD_TRANSCRIPT_STD_IMPLEMENTATION),
        kind_id: definition.kind_id,
        kind_contract_revision: KindContractRevision::from(
            conduit_net::RECORD_TRANSCRIPT_CONTRACT_REVISION,
        ),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("std/bounded-record-transcript@1"),
            implementation_id: ImplementationId::from(RECORD_TRANSCRIPT_STD_IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-net/bounded-record-transcript@1"),
        },
        inputs: definition.inputs,
        outputs: definition.outputs,
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 3,
            max_queue_bytes: conduit_net::MAXIMUM_RECORD_TRANSCRIPT_BYTES as u32,
        },
    }
}

fn parameter(name: &str) -> FaceStartupParameter {
    FaceStartupParameter {
        name: name.into(),
        value_type: "Count".into(),
        has_default: true,
    }
}
