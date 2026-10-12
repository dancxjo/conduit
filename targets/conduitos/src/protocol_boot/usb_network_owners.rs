//! Independent Root-owned frame queues, socket storage and network possession.
use super::super::{
    authority::{OperationAuthority, authority},
    usb_request::UsbEthernetNetworkRequest,
};
use crate::ethernet_frame::{
    contract::{FRAME_CALL, FRAME_KIND, FRAME_MAXIMUM_BYTES},
    factory::{FRAME_AUTHORITY, FRAME_IMPLEMENTATION},
    installation::EthernetFrameNativeIdentity,
    network_session::{EthernetNetworkApproval, EthernetNetworkScope},
    owned_installation::{OwnedEthernetNetworkInput, ReadyOwnedEthernetFrameBase},
};
use crate::{
    arch, cryptographic_entropy::CryptographicEntropyBase, protocol_source::NativeProtocolIssuer,
};
use alloc::{boxed::Box, format};
use conduit_core::*;

pub(in crate::protocol_boot) struct NativeEthernetOwner {
    pub ready: Box<ReadyOwnedEthernetFrameBase>,
    pub issuer: Box<NativeProtocolIssuer>,
    pub random_seed: u64,
}

/// # Safety
/// Called once by the sole trusted Root after independently approved local
/// network input is validated. This reserves one exclusively owned queue/stack;
/// neither USB discovery nor portable Source supplies this permission.
pub(super) unsafe fn admit(
    request: &UsbEthernetNetworkRequest,
    host: &HostId,
    boot: &BootId,
    artifact: &str,
) -> Result<NativeEthernetOwner, &'static str> {
    crate::ethernet_frame::retention::admit_backing(
        crate::allocation::BOOT_ARENA.capacity(),
        crate::allocation::BOOT_ARENA.live_bytes(),
        super::duplex_resources::pending_heap_bytes(),
    )
    .map_err(|_| "protocol-network-retention-backing-refused")?;
    let prefix = format!("conduitos/protocol/{}/network", boot.as_str());
    let identity = EthernetFrameNativeIdentity {
        host_id: host.clone(),
        boot_id: boot.clone(),
        base_id: format!("{prefix}/base").into(),
        provider_instance_id: format!("{prefix}/provider").into(),
        provider_generation: 1,
        resource_pool_id: format!("{prefix}/queues").into(),
        resource_generation_id: ResourceGenerationId(format!("{prefix}/queues/1")),
        envelope_id: format!("{prefix}/envelope").into(),
        artifact_id: artifact.into(),
    };
    let capability = authority(
        host,
        boot,
        "ethernet-frame",
        (
            &identity.provider_instance_id,
            &identity.resource_pool_id,
            &identity.resource_generation_id,
            &identity.envelope_id,
        ),
        OperationAuthority {
            contract: FRAME_AUTHORITY,
            call: FRAME_CALL,
            kind: FRAME_KIND,
            capability: FRAME_IMPLEMENTATION,
            bytes: FRAME_MAXIMUM_BYTES,
            work: 1,
            operations: request.maximum_operations,
        },
    );
    // SAFETY: this independently validated scope came from trusted local input.
    let approval = unsafe {
        EthernetNetworkApproval::admit(
            host.clone(),
            boot.clone(),
            EthernetNetworkScope {
                mac: request.mac,
                endpoint: crate::virtio_tcp::VirtioTcpEndpoint {
                    guest_address: request.guest_address,
                    prefix_length: request.prefix_length,
                    gateway: request.gateway,
                    remote_address: request.remote_address,
                    remote_port: request.remote_port,
                    local_port: request.local_port,
                },
                maximum_polls: request.maximum_polls,
            },
        )
    }
    .map_err(|_| "protocol-network-approval-refused")?;
    // SAFETY: the Root retains this exclusively owned stable allocation.
    let ready = unsafe {
        ReadyOwnedEthernetFrameBase::prepare(
            identity,
            approval,
            OwnedEthernetNetworkInput {
                request: &request.request,
                maximum_response_bytes: request.maximum_response_bytes,
                expected_response_bytes: request.expected_response_bytes,
            },
        )
    }
    .map_err(|_| "protocol-network-storage-refused")?;
    let source = arch::DomainEntropy::detect(1).map_err(|_| "protocol-entropy-unavailable")?;
    let mut entropy =
        CryptographicEntropyBase::<_, 2>::admit(source).map_err(|_| "protocol-entropy-refused")?;
    // SAFETY: this issuer receives only the independently approved frame authority.
    let issuer = unsafe { NativeProtocolIssuer::admit(capability, &mut entropy) }
        .map_err(|_| "protocol-network-issuance-refused")?;
    let mut seed = [0; 8];
    entropy
        .fill(&mut seed)
        .map_err(|_| "protocol-network-seed-refused")?;
    let random_seed = u64::from_le_bytes(seed);
    seed.fill(0);
    Ok(NativeEthernetOwner {
        ready: Box::new(ready),
        issuer: Box::new(issuer),
        random_seed,
    })
}
