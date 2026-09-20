//! Pure-kernel std offers for explicit framed-record temporal boundaries.

use conduit_core::{
    ArtifactId, CapabilityId, CapabilityOffer, CapabilityOfferBuilder, CapabilityRealization,
    ExecutionProfileId, ImplementationId, SemanticCapabilityContract,
};

pub const RECORD_SINGLETON_STREAM_STD_IMPLEMENTATION: &str = "std/record-singleton-stream@1";
pub const RECORD_EXACTLY_ONE_STD_IMPLEMENTATION: &str = "std/record-exactly-one@1";

pub fn record_singleton_stream_std_offer() -> CapabilityOffer {
    offer(
        conduit_net::record_singleton_stream_semantic_contract(),
        RECORD_SINGLETON_STREAM_STD_IMPLEMENTATION,
    )
}

pub fn record_exactly_one_std_offer() -> CapabilityOffer {
    offer(
        conduit_net::record_exactly_one_semantic_contract(),
        RECORD_EXACTLY_ONE_STD_IMPLEMENTATION,
    )
}

fn offer(contract: SemanticCapabilityContract, implementation: &str) -> CapabilityOffer {
    CapabilityOfferBuilder::new(
        contract,
        CapabilityRealization {
            capability_id: CapabilityId::from(implementation),
            execution_profile_id: ExecutionProfileId::from("std/record-temporal@1"),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from("conduit-net/record-temporal@1"),
            host_operations: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}
