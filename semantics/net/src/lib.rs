#![no_std]

extern crate alloc;

#[cfg(feature = "form-catalog")]
use alloc::string::{String, ToString};
use alloc::vec;
use conduit_core::{
    kind_id, resource_offer, resource_requirement, ArtifactId, AuthorityContractId, AuthorityGrant,
    AuthorityRequirement, BootId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, HostAdvertisement, HostId, HostOperationContractId,
    HostOperationRequirement, HostProfileId, ImplementationId, KindContractRevision,
    OfferGeneration, PortDescriptor, PortDirection, PortId, PortTemporal, ResourceBinding,
    ResourceOffer, PROTOCOL_VERSION,
};

mod external_websocket;
pub use external_websocket::*;
mod typed_record;
pub use typed_record::*;
#[cfg(feature = "form-catalog")]
mod typed_record_catalog;
#[cfg(feature = "form-catalog")]
pub use typed_record_catalog::*;
mod network_info;
pub use network_info::*;
mod application_info;
pub use application_info::*;
#[cfg(feature = "form-catalog")]
mod application_catalog;
#[cfg(feature = "form-catalog")]
pub use application_catalog::*;

pub const WIFI_STATION_RESOURCE_CLASS: &str = "conduit.resource/network/wifi-station@1";
pub const NETWORK_JOIN_OPERATION: &str = "network/join";
pub const NETWORK_CREDENTIALS_OPERATION: &str = "network/credentials";
pub const NETWORK_ATTACHMENT_SIGN_OPERATION: &str = "network/attachment-sign";
pub const NETWORK_JOIN_CONTRACT_REVISION: &str = "conduit.network/join@1";
pub const NETWORK_CREDENTIALS_CONTRACT_REVISION: &str = "conduit.network/credentials@1";
pub const NETWORK_ATTACHMENT_SIGN_CONTRACT_REVISION: &str = "conduit.network/attachment-sign@1";
pub const NETWORK_JOIN_HOST_OPERATION: &str = "conduit.host/network-join@1";
pub const NETWORK_CREDENTIALS_HOST_OPERATION: &str = "conduit.host/network-credentials@1";
pub const NETWORK_ATTACHMENT_SIGN_HOST_OPERATION: &str = "conduit.host/network-attachment-sign@1";
pub const NETWORK_CONFIG_AUTHORITY: &str = "conduit.authority/network-config@1";
pub const NETWORK_CONFIG_SUBJECT: &str = "authority/network-configurator";
pub const NETWORK_CREDENTIALS_AUTHORITY: &str = "conduit.authority/network-credentials@1";
pub const NETWORK_CREDENTIALS_SUBJECT: &str = "authority/network-credential-reader";
pub const NETWORK_JOIN_REQUEST_KIND: &str = "network/join-request";
pub const NETWORK_ATTACHMENT_KIND: &str = "network/attachment";
pub const MAXIMUM_SSID_BYTES: usize = 32;
pub const MAXIMUM_CREDENTIAL_BYTES: usize = 128;
pub const MAXIMUM_ATTACHMENT_ID_BYTES: usize = 96;
pub const MAXIMUM_JOIN_INPUT_BYTES: u32 = 167;
pub const MAXIMUM_JOIN_OUTPUT_BYTES: u32 = 167;
pub const NETWORK_JOIN_WIRE_VERSION: u8 = 1;
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
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("conduit.network/join-base@1"),
            implementation_id,
            artifact_id,
        },
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
            target_kind: Some(kind_id(NETWORK_CONFIG_SUBJECT)),
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

/// Semantic source of one volatile credential-bearing join request. The Plan
/// binds only this Face and an exact authority grant; secret bytes enter only
/// as the bounded host-operation result after Play starts.
pub fn network_credentials_offer(
    capability_id: CapabilityId,
    implementation_id: ImplementationId,
    artifact_id: ArtifactId,
) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id,
        kind_id: kind_id(NETWORK_CREDENTIALS_OPERATION),
        kind_contract_revision: KindContractRevision::from(NETWORK_CREDENTIALS_CONTRACT_REVISION),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("conduit.network/credentials-hosted@1"),
            implementation_id,
            artifact_id,
        },
        inputs: vec![],
        outputs: vec![PortDescriptor {
            port_id: PortId::from("request"),
            value_kind: kind_id(NETWORK_JOIN_REQUEST_KIND),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
        }],
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(NETWORK_CREDENTIALS_HOST_OPERATION),
            target_kind: Some(kind_id(NETWORK_CREDENTIALS_SUBJECT)),
            maximum_in_flight: 1,
            // The kernel host-operation table requires a non-zero admitted
            // input bound even though this source invokes the operation with
            // an exact empty value.
            maximum_input_bytes: 1,
            maximum_output_bytes: MAXIMUM_JOIN_INPUT_BYTES,
        }],
        resource_requirements: vec![],
        authority_requirements: vec![AuthorityRequirement {
            contract_id: AuthorityContractId::from(NETWORK_CREDENTIALS_AUTHORITY),
            host_operation_contract_id: HostOperationContractId::from(
                NETWORK_CREDENTIALS_HOST_OPERATION,
            ),
            subject_kind: kind_id(NETWORK_CREDENTIALS_SUBJECT),
        }],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_JOIN_INPUT_BYTES,
        },
    }
}

pub fn network_attachment_sign_offer(
    capability_id: CapabilityId,
    implementation_id: ImplementationId,
    artifact_id: ArtifactId,
) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id,
        kind_id: kind_id(NETWORK_ATTACHMENT_SIGN_OPERATION),
        kind_contract_revision: KindContractRevision::from(
            NETWORK_ATTACHMENT_SIGN_CONTRACT_REVISION,
        ),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("conduit.network/attachment-sign-usb@1"),
            implementation_id,
            artifact_id,
        },
        inputs: vec![PortDescriptor {
            port_id: PortId::from("attachment"),
            value_kind: kind_id(NETWORK_ATTACHMENT_KIND),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
        }],
        outputs: vec![],
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(NETWORK_ATTACHMENT_SIGN_HOST_OPERATION),
            target_kind: None,
            maximum_in_flight: 1,
            maximum_input_bytes: MAXIMUM_JOIN_OUTPUT_BYTES,
            maximum_output_bytes: 1,
        }],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_JOIN_OUTPUT_BYTES,
        },
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_network_bootstrap_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    use conduit_form::{KindDefinition, KindSignature};

    for offer in [
        network_credentials_offer(
            CapabilityId::from("catalog/network-credentials"),
            ImplementationId::from("catalog/network-credentials"),
            ArtifactId::from("catalog/network-credentials"),
        ),
        network_join_offer(
            CapabilityId::from("catalog/network-join"),
            ImplementationId::from("catalog/network-join"),
            ArtifactId::from("catalog/network-join"),
        ),
        network_attachment_sign_offer(
            CapabilityId::from("catalog/network-attachment-sign"),
            ImplementationId::from("catalog/network-attachment-sign"),
            ArtifactId::from("catalog/network-attachment-sign"),
        ),
    ] {
        startup.insert(KindSignature {
            kind: offer.kind_id.as_str().to_string(),
            startup_parameters: vec![],
        })?;
        profile
            .insert(KindDefinition {
                kind_id: offer.kind_id,
                kind_contract_revision: offer.kind_contract_revision,
                inputs: offer.inputs,
                outputs: offer.outputs,
                configuration: vec![],
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub fn network_capable_advertisement(host_id: &str, boot_id: &str) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(host_id),
        boot_id: BootId::from(boot_id),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("network/fixture-base"),
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

/// Deterministic fixture base for contract conformance only. It validates
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
    validate_join_request(&request)?;
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
