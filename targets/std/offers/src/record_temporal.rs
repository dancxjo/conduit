//! Pure-kernel std offers for explicit framed-record temporal boundaries.

use conduit_core::{
    ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
    ImplementationId, ImplementationOffer, KindContractRevision,
};

pub const RECORD_SINGLETON_STREAM_STD_IMPLEMENTATION: &str = "std/record-singleton-stream@1";
pub const RECORD_EXACTLY_ONE_STD_IMPLEMENTATION: &str = "std/record-exactly-one@1";

pub fn record_singleton_stream_std_offer() -> CapabilityOffer {
    offer(
        conduit_net::record_singleton_stream_definition(),
        RECORD_SINGLETON_STREAM_STD_IMPLEMENTATION,
    )
}

pub fn record_exactly_one_std_offer() -> CapabilityOffer {
    offer(
        conduit_net::record_exactly_one_definition(),
        RECORD_EXACTLY_ONE_STD_IMPLEMENTATION,
    )
}

fn offer(definition: conduit_form::KindDefinition, implementation: &str) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: Some((
            definition.inputs[0].port_id.clone(),
            definition.outputs[0].port_id.clone(),
        )),
        capability_id: CapabilityId::from(implementation),
        kind_id: definition.kind_id,
        kind_contract_revision: KindContractRevision::from(
            conduit_net::RECORD_TEMPORAL_CONTRACT_REVISION,
        ),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("std/record-temporal@1"),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from("conduit-net/record-temporal@1"),
        },
        inputs: definition.inputs,
        outputs: definition.outputs,
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 4,
            max_queue_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32 * 4,
        },
    }
}
