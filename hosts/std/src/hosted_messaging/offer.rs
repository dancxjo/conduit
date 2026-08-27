//! Exact planned capability, resource, and authority for GitHub delivery.

use conduit_chat::{
    delivery_request_type, delivery_update_type, notification_event_type, portable_message_type,
    MESSAGING_DELIVERY_KIND, MESSAGING_MESSAGE_KIND, MESSAGING_REVISION,
};
use conduit_core::{
    authority_grant, kind_id, port_id, resource_offer, resource_requirement, ArtifactId,
    AuthorityContractId, AuthorityGrant, AuthorityRequirement, CapabilityId, CapabilityLimits,
    CapabilityOffer, ExecutionProfileId, HostId, HostOperationContractId, HostOperationRequirement,
    ImplementationId, ImplementationOffer, KindContractRevision, PortDescriptor, PortDirection,
    PortTemporal, ResourceOffer, StructuredInfoType, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
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
    vec![
        deterministic_offer(
            MESSAGING_MESSAGE_KIND,
            vec![],
            vec![
                port("message", &portable_message_type(), PortDirection::Output),
                port("request", &delivery_request_type(), PortDirection::Output),
            ],
        ),
        deterministic_offer(
            MESSAGING_DELIVERY_KIND,
            vec![port(
                "request",
                &delivery_request_type(),
                PortDirection::Input,
            )],
            vec![
                port(
                    "notification",
                    &notification_event_type(),
                    PortDirection::Output,
                ),
                port("update", &delivery_update_type(), PortDirection::Output),
            ],
        ),
    ]
}

pub fn github_messaging_offer() -> CapabilityOffer {
    let mut offer = messaging_std_offers()
        .into_iter()
        .find(|offer| offer.kind_id.as_str() == MESSAGING_DELIVERY_KIND)
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

fn port(name: &str, value_type: &StructuredInfoType, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: value_type
            .profile()
            .expect("reviewed messaging profile")
            .value_kind()
            .clone(),
        direction,
        temporal: PortTemporal::Value,
    }
}

fn deterministic_offer(
    kind: &str,
    inputs: Vec<PortDescriptor>,
    outputs: Vec<PortDescriptor>,
) -> CapabilityOffer {
    let operation_target = if kind == MESSAGING_DELIVERY_KIND {
        delivery_request_type()
            .profile()
            .expect("reviewed delivery request profile")
            .value_kind()
            .clone()
    } else {
        kind_id(kind)
    };
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from(format!("std/{kind}@1")),
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(MESSAGING_REVISION),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(MESSAGING_PROFILE),
            implementation_id: ImplementationId::from(format!("std/{kind}@1")),
            artifact_id: ArtifactId::from(MESSAGING_ARTIFACT),
        },
        inputs,
        outputs,
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(MESSAGING_HOST_OPERATION),
            target_kind: Some(operation_target.clone()),
            maximum_in_flight: 1,
            maximum_input_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            maximum_output_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: (kind == MESSAGING_DELIVERY_KIND)
            .then(|| AuthorityRequirement {
                contract_id: AuthorityContractId::from(MESSAGING_DELIVERY_AUTHORITY),
                host_operation_contract_id: HostOperationContractId::from(MESSAGING_HOST_OPERATION),
                subject_kind: operation_target,
            })
            .into_iter()
            .collect(),
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: 4,
            max_queue_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES * 4) as u32,
        },
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
