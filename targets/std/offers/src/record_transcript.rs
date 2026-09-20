//! Pure-kernel std offer for bounded typed-record transcript retention.

use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    ImplementationId,
};

pub const RECORD_TRANSCRIPT_STD_IMPLEMENTATION: &str = "std/bounded-record-transcript@1";

pub fn record_transcript_std_offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        conduit_net::record_transcript_semantic_contract(),
        Back {
            capability_id: CapabilityId::from(RECORD_TRANSCRIPT_STD_IMPLEMENTATION),
            execution_profile_id: ExecutionProfileId::from("std/bounded-record-transcript@1"),
            implementation_id: ImplementationId::from(RECORD_TRANSCRIPT_STD_IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-net/bounded-record-transcript@1"),
            host_operations: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}
