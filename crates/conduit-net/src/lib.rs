#![no_std]

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec;
use conduit_core::{
    kind_id, resource_offer, resource_requirement, ArtifactId, AuthorityContractId, AuthorityGrant,
    AuthorityRequirement, BootId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, HostAdvertisement, HostId, HostOperationContractId,
    HostOperationRequirement, HostProfileId, ImplementationId, KindContractRevision,
    OfferGeneration, PortDescriptor, PortDirection, PortId, PortTemporal, ResourceBinding,
    ResourceOffer, ResourcePoolId, PROTOCOL_VERSION,
};
use serde::{Deserialize, Serialize};

mod external_websocket;
pub use external_websocket::*;

pub const WIFI_STATION_RESOURCE_CLASS: &str = "conduit.resource/network/wifi-station@1";
pub const NETWORK_JOIN_OPERATION: &str = "network/join";
pub const NETWORK_JOIN_CONTRACT_REVISION: &str = "conduit.network/join@1";
pub const NETWORK_JOIN_HOST_OPERATION: &str = "conduit.host/network-join@1";
pub const NETWORK_CONFIG_AUTHORITY: &str = "conduit.authority/network-config@1";
pub const NETWORK_CONFIG_SUBJECT: &str = "authority/network-configurator";
pub const NETWORK_JOIN_REQUEST_KIND: &str = "network/join-request";
pub const NETWORK_ATTACHMENT_KIND: &str = "network/attachment";
pub const MAXIMUM_SSID_BYTES: usize = 32;
pub const MAXIMUM_CREDENTIAL_BYTES: usize = 128;
pub const MAXIMUM_ATTACHMENT_ID_BYTES: usize = 96;
pub const MAXIMUM_JOIN_INPUT_BYTES: u32 = 160;
pub const MAXIMUM_JOIN_OUTPUT_BYTES: u32 = 128;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NetworkAttachmentId(String);

impl From<&str> for NetworkAttachmentId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl NetworkAttachmentId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Boot-scoped runtime truth produced after successful provider execution.
/// It deliberately contains no SSID, credential, address, socket, or carrier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkAttachment {
    pub attachment_id: NetworkAttachmentId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub interface_pool_id: ResourcePoolId,
    pub generation: u64,
}

/// Volatile provider input. Secret bytes intentionally implement neither
/// serialization nor display/debug formatting.
pub struct NetworkJoinRequest<'a> {
    pub ssid: &'a [u8],
    pub credential: &'a [u8],
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum NetworkJoinError {
    MalformedRequest,
    CredentialTooLarge,
    StaleHostBoot,
    Unsupported,
    MissingResource,
    ResourceMismatch,
    MissingAuthority,
    StaleAuthority,
    AuthorityMismatch,
    InvalidAttachment,
}

pub fn wifi_station_resource(pool_id: &str) -> ResourceOffer {
    resource_offer(pool_id, WIFI_STATION_RESOURCE_CLASS, 1)
}

pub fn network_join_offer(
    capability_id: CapabilityId,
    implementation_id: ImplementationId,
    artifact_id: ArtifactId,
) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: Some((PortId::from("request"), PortId::from("attachment"))),
        capability_id,
        kind_id: kind_id(NETWORK_JOIN_OPERATION),
        kind_contract_revision: KindContractRevision::from(NETWORK_JOIN_CONTRACT_REVISION),
        execution_profile_id: ExecutionProfileId::from("conduit.network/join-provider@1"),
        implementation_id,
        artifact_id,
        inputs: vec![PortDescriptor {
            port_id: PortId::from("request"),
            value_kind: kind_id(NETWORK_JOIN_REQUEST_KIND),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
        }],
        outputs: vec![PortDescriptor {
            port_id: PortId::from("attachment"),
            value_kind: kind_id(NETWORK_ATTACHMENT_KIND),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
        }],
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(NETWORK_JOIN_HOST_OPERATION),
            target_kind: None,
            maximum_in_flight: 1,
            maximum_input_bytes: MAXIMUM_JOIN_INPUT_BYTES,
            maximum_output_bytes: MAXIMUM_JOIN_OUTPUT_BYTES,
        }],
        resource_requirements: vec![resource_requirement(WIFI_STATION_RESOURCE_CLASS, 1)],
        authority_requirements: vec![AuthorityRequirement {
            contract_id: AuthorityContractId::from(NETWORK_CONFIG_AUTHORITY),
            host_operation_contract_id: HostOperationContractId::from(NETWORK_JOIN_HOST_OPERATION),
            subject_kind: kind_id(NETWORK_CONFIG_SUBJECT),
        }],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_JOIN_INPUT_BYTES,
        },
    }
}

pub fn network_capable_advertisement(host_id: &str, boot_id: &str) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(host_id),
        boot_id: BootId::from(boot_id),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("network/fixture-provider"),
        resources: vec![wifi_station_resource("resource/wifi-station-0")],
        capabilities: vec![network_join_offer(
            CapabilityId::from("capability/network-join"),
            ImplementationId::from("fixture/network-join-v1"),
            ArtifactId::from("fixture/network-join-artifact"),
        )],
        planner_capabilities: vec![],
    }
}

pub fn network_omitting_advertisement(host_id: &str, boot_id: &str) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(host_id),
        boot_id: BootId::from(boot_id),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("network/omitting-host"),
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    }
}

/// Deterministic fixture provider for contract conformance only. It validates
/// the current advertised/resource/authority facts and never retains secrets.
pub fn execute_fixture_join(
    request: NetworkJoinRequest<'_>,
    advertisement: &HostAdvertisement,
    selected_capability_id: &CapabilityId,
    resource: &ResourceBinding,
    authority: Option<&AuthorityGrant>,
    attachment_id: NetworkAttachmentId,
    generation: u64,
) -> Result<NetworkAttachment, NetworkJoinError> {
    if request.ssid.is_empty() || request.ssid.len() > MAXIMUM_SSID_BYTES {
        return Err(NetworkJoinError::MalformedRequest);
    }
    if request.credential.is_empty() || request.credential.len() > MAXIMUM_CREDENTIAL_BYTES {
        return Err(NetworkJoinError::CredentialTooLarge);
    }
    if advertisement.protocol_version != PROTOCOL_VERSION {
        return Err(NetworkJoinError::StaleHostBoot);
    }
    let offer = advertisement
        .capabilities
        .iter()
        .find(|offer| &offer.capability_id == selected_capability_id)
        .ok_or(NetworkJoinError::Unsupported)?;
    if offer.checked_face()
        != network_join_offer(
            CapabilityId::from("face/network-join"),
            ImplementationId::from("face/network-join"),
            ArtifactId::from("face/network-join"),
        )
        .checked_face()
    {
        return Err(NetworkJoinError::Unsupported);
    }
    let resource_offer = advertisement
        .resources
        .iter()
        .find(|item| item.pool_id == resource.pool_id)
        .ok_or(NetworkJoinError::MissingResource)?;
    if resource.class_id.as_str() != WIFI_STATION_RESOURCE_CLASS
        || resource.units != 1
        || resource.protected.is_some()
        || resource_offer.class_id != resource.class_id
        || resource_offer.capacity_units < resource.units
    {
        return Err(NetworkJoinError::ResourceMismatch);
    }
    let authority = authority.ok_or(NetworkJoinError::MissingAuthority)?;
    if authority.host_id != advertisement.host_id || authority.boot_id != advertisement.boot_id {
        return Err(NetworkJoinError::StaleAuthority);
    }
    if authority.capability_id != *selected_capability_id
        || authority.contract_id.as_str() != NETWORK_CONFIG_AUTHORITY
        || authority.host_operation_contract_id.as_str() != NETWORK_JOIN_HOST_OPERATION
        || authority.subject_kind.as_str() != NETWORK_CONFIG_SUBJECT
    {
        return Err(NetworkJoinError::AuthorityMismatch);
    }
    let encoded_identity_bytes = attachment_id
        .as_str()
        .len()
        .checked_add(advertisement.host_id.as_str().len())
        .and_then(|size| size.checked_add(advertisement.boot_id.as_str().len()))
        .and_then(|size| size.checked_add(resource.pool_id.as_str().len()))
        .and_then(|size| size.checked_add(core::mem::size_of::<u64>()));
    if attachment_id.as_str().is_empty()
        || attachment_id.as_str().len() > MAXIMUM_ATTACHMENT_ID_BYTES
        || encoded_identity_bytes.is_none_or(|size| size > MAXIMUM_JOIN_OUTPUT_BYTES as usize)
        || generation == 0
    {
        return Err(NetworkJoinError::InvalidAttachment);
    }
    Ok(NetworkAttachment {
        attachment_id,
        host_id: advertisement.host_id.clone(),
        boot_id: advertisement.boot_id.clone(),
        interface_pool_id: resource.pool_id.clone(),
        generation,
    })
}
