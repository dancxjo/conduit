//! Exact planned capability, resource, and authority for GitHub delivery.

use conduit_chat::{delivery_request_type, messaging_semantic_contracts, MESSAGING_DELIVERY_KIND};
use conduit_core::{
    authority_grant, kind_id, resource_offer, resource_requirement, ArtifactId,
    AuthorityContractId, AuthorityGrant, AuthorityRequirement, CapabilityId, CapabilityOffer,
    CapabilityOfferBuilder, CapabilityRealization, ExecutionProfileId, HostId,
    HostOperationContractId, HostOperationRequirement, ImplementationId, ResourceOffer,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};

pub const MESSAGING_PROFILE: &str = "std/messaging-deterministic-hosted@1";
pub const MESSAGING_ARTIFACT: &str = "conduit-std-host/messaging-deterministic@1";
pub const MESSAGING_HOST_OPERATION: &str = "conduit.host/messaging-deterministic@1";
pub const MESSAGING_DELIVERY_AUTHORITY: &str = "conduit.authority/messaging-deliver@1";

pub const GITHUB_MESSAGING_RESOURCE_CLASS: &str =
    "conduit.resource/messaging/github-issue-account@1";
pub const GITHUB_MESSAGING_RESOURCE_ID: &str = "std/github-issue-account";
pub const GITHUB_MESSAGING_AUTHORITY: &str = "conduit.authority/messaging-github-comment@1";
pub const GITHUB_MESSAGING_OPERATION: &str = "conduit.host/messaging-github-comment@1";
const PROFILE: &str = "std/messaging-github-issue-comment@1";
const IMPLEMENTATION: &str = "std/kernel-messaging-github-issue-comment@1";
const ARTIFACT: &str = "conduit-std-host/messaging-github-issue-comment@1";

pub fn messaging_std_offers() -> Vec<CapabilityOffer> {
    messaging_semantic_contracts()
        .into_iter()
        .map(|contract| {
            let kind = contract.kind_id.as_str().to_owned();
            CapabilityOfferBuilder::new(contract, deterministic_realization(&kind)).build()
        })
        .collect()
}

pub fn github_messaging_offer() -> CapabilityOffer {
    let contract = messaging_semantic_contracts()
        .into_iter()
        .find(|contract| contract.kind_id.as_str() == MESSAGING_DELIVERY_KIND)
        .expect("reviewed messaging delivery contract");
    let operation_target = delivery_request_type()
        .profile()
        .expect("reviewed delivery request profile")
        .value_kind()
        .clone();
    CapabilityOfferBuilder::new(
        contract,
        CapabilityRealization {
            capability_id: CapabilityId::from("std/messaging-github-issue-comment@1"),
            execution_profile_id: ExecutionProfileId::from(PROFILE),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from(ARTIFACT),
            host_operations: vec![host_operation(
                GITHUB_MESSAGING_OPERATION,
                operation_target.clone(),
            )],
            resource_requirements: vec![resource_requirement(GITHUB_MESSAGING_RESOURCE_CLASS, 1)],
            authority_requirements: vec![AuthorityRequirement {
                contract_id: AuthorityContractId::from(GITHUB_MESSAGING_AUTHORITY),
                host_operation_contract_id: HostOperationContractId::from(
                    GITHUB_MESSAGING_OPERATION,
                ),
                subject_kind: operation_target,
            }],
        },
    )
    .build()
}

fn deterministic_realization(kind: &str) -> CapabilityRealization {
    let operation_target = if kind == MESSAGING_DELIVERY_KIND {
        delivery_request_type()
            .profile()
            .expect("reviewed delivery request profile")
            .value_kind()
            .clone()
    } else {
        kind_id(kind)
    };
    CapabilityRealization {
        capability_id: CapabilityId::from(format!("std/{kind}@1")),
        execution_profile_id: ExecutionProfileId::from(MESSAGING_PROFILE),
        implementation_id: ImplementationId::from(format!("std/{kind}@1")),
        artifact_id: ArtifactId::from(MESSAGING_ARTIFACT),
        host_operations: vec![host_operation(
            MESSAGING_HOST_OPERATION,
            operation_target.clone(),
        )],
        resource_requirements: vec![],
        authority_requirements: (kind == MESSAGING_DELIVERY_KIND)
            .then(|| AuthorityRequirement {
                contract_id: AuthorityContractId::from(MESSAGING_DELIVERY_AUTHORITY),
                host_operation_contract_id: HostOperationContractId::from(MESSAGING_HOST_OPERATION),
                subject_kind: operation_target,
            })
            .into_iter()
            .collect(),
    }
}

fn host_operation(contract: &str, target_kind: conduit_core::KindId) -> HostOperationRequirement {
    HostOperationRequirement {
        contract_id: HostOperationContractId::from(contract),
        target_kind: Some(target_kind),
        maximum_in_flight: 1,
        maximum_input_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        maximum_output_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
    }
}

pub fn github_messaging_resource_offer() -> ResourceOffer {
    resource_offer(
        GITHUB_MESSAGING_RESOURCE_ID,
        GITHUB_MESSAGING_RESOURCE_CLASS,
        1,
    )
}

pub fn github_messaging_authority_grant(
    offer: &CapabilityOffer,
    grant_id: &str,
    host_id: HostId,
    boot_id: conduit_core::BootId,
) -> Result<AuthorityGrant, String> {
    let requirement = offer
        .authority_requirements
        .first()
        .ok_or_else(|| "GitHub messaging authority requirement is absent".to_string())?;
    Ok(authority_grant(
        grant_id,
        requirement,
        host_id,
        boot_id,
        offer.capability_id.clone(),
    ))
}
