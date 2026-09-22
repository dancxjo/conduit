//! Finite std-host offers for portable geometry semantics.

use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    HostCallContractId, HostCallRequirement, ImplementationId, Kind,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_presentation::{geometry_semantic_contracts, POINT2_LITERAL_KIND};
use std::{format, vec, vec::Vec};

pub const GEOMETRY_PROFILE: &str = "std/geometry-kernel-hosted@1";
pub const GEOMETRY_ARTIFACT: &str = "conduit-std-host/geometry@1";
pub const GEOMETRY_HOST_CALL: &str = "conduit.host/geometry-transform@1";

pub fn geometry_std_offers() -> Vec<CapabilityOffer> {
    geometry_semantic_contracts()
        .into_iter()
        .map(offer)
        .collect()
}

fn offer(contract: Kind) -> CapabilityOffer {
    let kind = contract.kind_id.as_str().to_owned();
    let uses_operation = kind != POINT2_LITERAL_KIND;
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(format!("std/{kind}@1")),
            execution_profile_id: ExecutionProfileId::from(GEOMETRY_PROFILE),
            implementation_id: ImplementationId::from(format!("std/{kind}@1")),
            artifact_id: ArtifactId::from(GEOMETRY_ARTIFACT),
            host_calls: if uses_operation {
                vec![HostCallRequirement {
                    contract_id: HostCallContractId::from(GEOMETRY_HOST_CALL),
                    target_kind: Some(conduit_core::kind_id(&kind)),
                    maximum_in_flight: 1,
                    maximum_input_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
                    maximum_output_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
                }]
            } else {
                Vec::new()
            },
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}
