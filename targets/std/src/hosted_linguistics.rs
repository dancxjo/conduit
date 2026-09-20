//! Finite std-host offers for portable linguistic contracts.

use conduit_core::{
    ArtifactId, CapabilityId, CapabilityOffer, CapabilityOfferBuilder, CapabilityRealization,
    ExecutionProfileId, HostOperationContractId, HostOperationRequirement, ImplementationId,
    SemanticCapabilityContract, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};

pub const LINGUISTICS_PROFILE: &str = "std/linguistics-kernel-hosted@1";
pub const LINGUISTICS_ARTIFACT: &str = "conduit-std-host/linguistics@1";
pub const LINGUISTICS_HOST_OPERATION: &str = "conduit.host/linguistics@1";

pub fn linguistics_std_offers() -> Vec<CapabilityOffer> {
    vec![
        offer(conduit_language::tokenize_four_semantic_contract()),
        offer(conduit_language::annotate_four_semantic_contract()),
    ]
}

fn offer(contract: SemanticCapabilityContract) -> CapabilityOffer {
    let kind = contract.kind_id.as_str().to_owned();
    CapabilityOfferBuilder::new(
        contract,
        CapabilityRealization {
            capability_id: CapabilityId::from(format!("std/{kind}@1")),
            execution_profile_id: ExecutionProfileId::from(LINGUISTICS_PROFILE),
            implementation_id: ImplementationId::from(format!("std/{kind}@1")),
            artifact_id: ArtifactId::from(LINGUISTICS_ARTIFACT),
            host_operations: vec![HostOperationRequirement {
                contract_id: HostOperationContractId::from(LINGUISTICS_HOST_OPERATION),
                target_kind: Some(conduit_core::kind_id(&kind)),
                maximum_in_flight: 1,
                maximum_input_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
                maximum_output_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            }],
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}
