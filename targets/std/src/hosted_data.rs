//! Finite std Host offers for the host-neutral finance and tabular contracts.

use conduit_core::{
    ArtifactId, CapabilityId, CapabilityOffer, CapabilityOfferBuilder, CapabilityRealization,
    ExecutionProfileId, HostOperationContractId, HostOperationRequirement, ImplementationId,
    SemanticCapabilityContract, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_data::*;

pub const FINANCE_PROFILE: &str = "std/finance-kernel-hosted@1";
pub const FINANCE_ARTIFACT: &str = "conduit-std-host/finance@1";
pub const FINANCE_HOST_OPERATION: &str = "conduit.host/finance-exact@1";
pub const TABULAR_PROFILE: &str = "std/tabular-kernel-hosted@1";
pub const TABULAR_ARTIFACT: &str = "conduit-std-host/tabular@1";
pub const TABULAR_HOST_OPERATION: &str = "conduit.host/tabular@1";

pub fn finance_std_offers() -> Vec<CapabilityOffer> {
    finance_semantic_contracts()
        .into_iter()
        .map(|contract| {
            offer(
                contract,
                FINANCE_PROFILE,
                FINANCE_ARTIFACT,
                FINANCE_HOST_OPERATION,
            )
        })
        .collect()
}

pub fn tabular_std_offers() -> Vec<CapabilityOffer> {
    tabular_semantic_contracts()
        .into_iter()
        .map(|contract| {
            offer(
                contract,
                TABULAR_PROFILE,
                TABULAR_ARTIFACT,
                TABULAR_HOST_OPERATION,
            )
        })
        .collect()
}

fn offer(
    contract: SemanticCapabilityContract,
    profile: &str,
    artifact: &str,
    operation: &str,
) -> CapabilityOffer {
    let kind = contract.kind_id.as_str().to_owned();
    CapabilityOfferBuilder::new(
        contract,
        CapabilityRealization {
            capability_id: CapabilityId::from(format!("std/{kind}@1")),
            execution_profile_id: ExecutionProfileId::from(profile),
            implementation_id: ImplementationId::from(format!("std/{kind}@1")),
            artifact_id: ArtifactId::from(artifact),
            host_operations: vec![HostOperationRequirement {
                contract_id: HostOperationContractId::from(operation),
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
