//! Exact planned capability, resource, and authority for GitHub delivery.

use conduit_core::{
    authority_grant, resource_offer, resource_requirement, ArtifactId, AuthorityGrant,
    CapabilityId, CapabilityOffer, ExecutionProfileId, HostId, HostOperationContractId,
    ImplementationId, ResourceOffer,
};

pub const GITHUB_MESSAGING_RESOURCE_CLASS: &str =
    "conduit.resource/messaging/github-issue-account@1";
pub const GITHUB_MESSAGING_RESOURCE_ID: &str = "std/github-issue-account";
pub const GITHUB_MESSAGING_AUTHORITY: &str = "conduit.authority/messaging-github-comment@1";
pub const GITHUB_MESSAGING_OPERATION: &str = "conduit.host/messaging-github-comment@1";
const PROFILE: &str = "std/messaging-github-issue-comment@1";
const IMPLEMENTATION: &str = "std/kernel-messaging-github-issue-comment@1";
const ARTIFACT: &str = "conduit-std-host/messaging-github-issue-comment@1";

pub fn github_messaging_offer() -> CapabilityOffer {
    let mut offer = conduit_std_catalog::messaging_std_offers()
        .into_iter()
        .find(|offer| offer.kind_id.as_str() == conduit_std_catalog::MESSAGING_DELIVERY_KIND)
        .expect("reviewed messaging delivery offer");
    offer.capability_id = CapabilityId::from("std/messaging-github-issue-comment@1");
    offer.implementation.execution_profile_id = ExecutionProfileId::from(PROFILE);
    offer.implementation.implementation_id = ImplementationId::from(IMPLEMENTATION);
    offer.implementation.artifact_id = ArtifactId::from(ARTIFACT);
    offer.host_operations[0].contract_id =
        HostOperationContractId::from(GITHUB_MESSAGING_OPERATION);
    offer.authority_requirements[0].contract_id = GITHUB_MESSAGING_AUTHORITY.into();
    offer.authority_requirements[0].host_operation_contract_id =
        HostOperationContractId::from(GITHUB_MESSAGING_OPERATION);
    offer.resource_requirements = vec![resource_requirement(GITHUB_MESSAGING_RESOURCE_CLASS, 1)];
    offer
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
