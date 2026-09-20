//! Pure-kernel std offer for bounded ordered framed-record queueing.

use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    ImplementationId,
};

pub const ORDERED_RECORD_QUEUE_STD_IMPLEMENTATION: &str = "std/ordered-record-queue@1";

pub fn ordered_record_queue_std_offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        conduit_net::ordered_record_queue_semantic_contract(),
        Back {
            capability_id: CapabilityId::from(ORDERED_RECORD_QUEUE_STD_IMPLEMENTATION),
            execution_profile_id: ExecutionProfileId::from("std/ordered-record-queue@1"),
            implementation_id: ImplementationId::from(ORDERED_RECORD_QUEUE_STD_IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-net/ordered-record-queue@1"),
            host_operations: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}
