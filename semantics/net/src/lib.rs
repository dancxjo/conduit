#![no_std]

extern crate alloc;

#[allow(dead_code)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/semantic_types.rs"));
}

pub use generated::{
    ApplicationNetworkRefusal, DnsQuery, DnsRecordKind, DnsResolution, DnsResult, DnsResultStale,
    DnsTtl, NetworkAddress, NetworkAttachmentId, NetworkConnectionState,
    NetworkConnectionStateConnected, NetworkEndpoint, NetworkJoinError, NetworkReason,
    NetworkTransport, RecordCorrelation, RecordDeliveryEvent, RecordDeliveryEventDisconnected,
    RecordDeliveryEventFailed, RecordDeliveryEventFramedQueued, RecordDeliveryEventPartiallySent,
    RecordDeliveryEventRefused, RecordDeliveryEventTimedOut,
    RecordDeliveryEventTransportUnavailable, RecordDeliveryObservation, RecordDeliveryState,
    RecordDeliveryStateDisconnected, RecordDeliveryStateFailed, RecordDeliveryStateFramedQueued,
    RecordDeliveryStatePartiallySent, RecordDeliveryStateRefused, RecordDeliveryStateTimedOut,
    RecordDeliveryStateTransportUnavailable, RecordReceipt, RecordTranscriptDirection,
    RecordTranscriptEntry, RecordTranscriptEvent, RecordTranscriptEventRecord,
    RecordTranscriptTerminal, RecordTranscriptTerminalFailed, RecordTranscriptTerminalRefused,
    ResolvedNetworkAddress, ResolvedNetworkEndpoint, TypedRecordFrame,
};

#[cfg(feature = "plot-catalog")]
use alloc::string::{String, ToString};
use alloc::vec;
use conduit_core::{
    resource_offer, ArtifactId, AuthorityGrant, AuthorityRequirement, Back, BackOfferBuilder,
    BootId, CapabilityId, CapabilityLimits, CapabilityOffer, HostAdvertisement,
    HostCallRequirement, HostId, HostProfileId, ImplementationId, Kind, OfferGeneration,
    PortDirection, PortTemporal, ResourceBinding, ResourceOffer, PROTOCOL_VERSION,
};

mod external_websocket;
pub use external_websocket::*;
mod typed_record;
pub use typed_record::*;
mod text_record;
pub use text_record::*;
mod ordered_record_queue;
pub use ordered_record_queue::*;
mod record_transcript;
pub use record_transcript::*;
mod record_delivery;
pub use record_delivery::*;
#[cfg(feature = "plot-catalog")]
mod typed_record_catalog;
#[cfg(feature = "plot-catalog")]
pub use typed_record_catalog::*;
#[cfg(feature = "plot-catalog")]
mod ordered_record_queue_catalog;
#[cfg(feature = "plot-catalog")]
pub use ordered_record_queue_catalog::*;
#[cfg(feature = "plot-catalog")]
mod record_temporal_catalog;
#[cfg(feature = "plot-catalog")]
pub use record_temporal_catalog::*;
#[cfg(feature = "plot-catalog")]
mod record_transcript_catalog;
#[cfg(feature = "plot-catalog")]
pub use record_transcript_catalog::*;
#[cfg(feature = "plot-catalog")]
mod record_delivery_catalog;
#[cfg(feature = "plot-catalog")]
pub use record_delivery_catalog::*;
mod borrowed_contracts;
pub use borrowed_contracts::{
    network_attachment_sign_contract_ref, network_credentials_contract_ref,
    network_join_contract_ref, CheckedNetworkContract, PreparedCanonicalNetworkOffer,
};
mod known_networks;
pub use known_networks::*;
mod network_info;
pub use network_info::*;
mod application_info;
pub use application_info::*;
#[cfg(feature = "plot-catalog")]
mod application_catalog;
#[cfg(feature = "plot-catalog")]
pub use application_catalog::*;

pub const WIFI_STATION_RESOURCE_CLASS: &str = "conduit.resource/network/wifi-station@1";
pub const NETWORK_JOIN_OPERATION: &str = "network/join";
pub const NETWORK_JOIN_EXECUTION_PROFILE: &str = "conduit.network/join-base@1";
pub const NETWORK_CREDENTIALS_OPERATION: &str = "network/credentials";
pub const NETWORK_ATTACHMENT_SIGN_OPERATION: &str = "network/attachment-sign";
pub const NETWORK_JOIN_CONTRACT_REVISION: &str = "conduit.network/join@1";
pub const NETWORK_CREDENTIALS_CONTRACT_REVISION: &str = "conduit.network/credentials@1";
pub const NETWORK_ATTACHMENT_SIGN_CONTRACT_REVISION: &str = "conduit.network/attachment-sign@1";
pub const NETWORK_JOIN_HOST_CALL: &str = "conduit.host/network-join@1";
pub const NETWORK_CREDENTIALS_HOST_CALL: &str = "conduit.host/network-credentials@1";
pub const NETWORK_ATTACHMENT_SIGN_HOST_CALL: &str = "conduit.host/network-attachment-sign@1";
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
    borrowed_contracts::materialize(
        borrowed_contracts::Shape::Join,
        capability_id,
        implementation_id,
        artifact_id,
    )
}

/// The semantic join Kind, independent of any initialized radio or authority.
pub fn network_join_contract() -> Kind {
    borrowed_contracts::network_join_contract_ref().materialize()
}

/// Semantic source of one volatile credential-bearing join request. The plan
/// binds only this front and an exact authority grant; secret bytes enter only
/// as the bounded Host Call result after Play starts.
pub fn network_credentials_offer(
    capability_id: CapabilityId,
    implementation_id: ImplementationId,
    artifact_id: ArtifactId,
) -> CapabilityOffer {
    borrowed_contracts::materialize(
        borrowed_contracts::Shape::Credentials,
        capability_id,
        implementation_id,
        artifact_id,
    )
}

pub fn network_credentials_contract() -> Kind {
    borrowed_contracts::network_credentials_contract_ref().materialize()
}

pub fn network_attachment_sign_offer(
    capability_id: CapabilityId,
    implementation_id: ImplementationId,
    artifact_id: ArtifactId,
) -> CapabilityOffer {
    borrowed_contracts::materialize(
        borrowed_contracts::Shape::AttachmentUsb,
        capability_id,
        implementation_id,
        artifact_id,
    )
}

pub const NETWORK_ATTACHMENT_LOCAL_PROFILE: &str = "conduit.network/attachment-sign-local@1";
/// The caller retains an actual bounded local receipt sink. This describes its
/// implementation; it does not create a sink, an attachment or authority.
pub fn network_attachment_sign_local_offer(
    capability_id: CapabilityId,
    implementation_id: ImplementationId,
    artifact_id: ArtifactId,
) -> CapabilityOffer {
    borrowed_contracts::materialize(
        borrowed_contracts::Shape::AttachmentLocal,
        capability_id,
        implementation_id,
        artifact_id,
    )
}

#[cfg(feature = "plot-catalog")]
fn network_attachment_sign_contract() -> Kind {
    borrowed_contracts::network_attachment_sign_contract_ref().materialize()
}

#[cfg(feature = "plot-catalog")]
pub fn install_network_bootstrap_catalogs(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    use conduit_plot::{KindProjection, KindSignature};

    for contract in [
        network_credentials_contract(),
        network_join_contract(),
        network_attachment_sign_contract(),
        network_scan_contract(),
        network_scan_sign_contract(),
    ] {
        startup.insert(KindSignature {
            kind: contract.kind_id.as_str().to_string(),
            startup_parameters: vec![],
        })?;
        profile
            .insert(KindProjection {
                kind_id: contract.kind_id,
                kind_contract_revision: contract.kind_contract_revision,
                inputs: contract.inputs,
                outputs: contract.outputs,
                configuration: vec![],
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub fn network_capable_advertisement(host_id: &str, boot_id: &str) -> HostAdvertisement {
    HostAdvertisement {
        declared_name: None,
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(host_id),
        boot_id: BootId::from(boot_id),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("network/fixture-base"),
        bases: vec![],
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
        declared_name: None,
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(host_id),
        boot_id: BootId::from(boot_id),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("network/omitting-host"),
        bases: vec![],
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
    if offer.checked_front()
        != network_join_offer(
            CapabilityId::from("front/network-join"),
            ImplementationId::from("front/network-join"),
            ArtifactId::from("front/network-join"),
        )
        .checked_front()
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
        || authority.host_call_contract_id.as_str() != NETWORK_JOIN_HOST_CALL
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
        resource_pool_id: resource.pool_id.clone(),
        generation,
    })
}

mod station_attempt;
pub use station_attempt::*;

mod network_scan;
pub use network_scan::*;
mod network_scan_sign;
pub use network_scan_sign::*;
