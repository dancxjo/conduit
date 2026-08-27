use conduit_core::{
    BaseImplementationId, BaseInstanceId, BootId, HostId, LineAvailability, LineAvailabilitySign,
    LineId, LineOffer, LinkAuthorityReference, LinkBinding, LinkBindingId, LinkCredentialReference,
    LinkEndpoint, LinkEndpointId, SignId,
};

use crate::{
    BleGattProfile, BleProfileError, BluetoothLineObservation, BluetoothLineState,
    BluetoothPairingState,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BleGattLineIdentity {
    pub line_id: LineId,
    pub binding_id: LinkBindingId,
    pub base_instance_id: BaseInstanceId,
    pub source_host_id: HostId,
    pub source_boot_id: BootId,
    pub source_endpoint_id: LinkEndpointId,
    pub sink_host_id: HostId,
    pub sink_boot_id: BootId,
    pub sink_endpoint_id: LinkEndpointId,
    pub credential: LinkCredentialReference,
    pub authority: LinkAuthorityReference,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BleLineAdmissionError {
    InvalidProfile(BleProfileError),
    NotReady,
    MissingConduitService,
    NotPaired,
    MissingPeerIdentity,
    HostMismatch,
    BootMismatch,
    EmptyIdentity,
    BaseInstanceMismatch,
}

pub fn offer_ble_gatt_line(
    identity: BleGattLineIdentity,
    observation: &BluetoothLineObservation,
    profile: BleGattProfile,
    ready_sign_id: SignId,
) -> Result<LineOffer, BleLineAdmissionError> {
    let limits = profile
        .link_limits()
        .map_err(BleLineAdmissionError::InvalidProfile)?;
    if observation.state != BluetoothLineState::Ready {
        return Err(BleLineAdmissionError::NotReady);
    }
    if !observation.candidate.advertises_conduit_service {
        return Err(BleLineAdmissionError::MissingConduitService);
    }
    if observation.pairing == BluetoothPairingState::Unpaired {
        return Err(BleLineAdmissionError::NotPaired);
    }
    let peer = observation
        .negotiated_peer
        .as_ref()
        .ok_or(BleLineAdmissionError::MissingPeerIdentity)?;
    if peer.host_id != identity.sink_host_id {
        return Err(BleLineAdmissionError::HostMismatch);
    }
    if peer.boot_id != identity.sink_boot_id {
        return Err(BleLineAdmissionError::BootMismatch);
    }
    if observation.base_instance_id != identity.base_instance_id {
        return Err(BleLineAdmissionError::BaseInstanceMismatch);
    }
    if identity.line_id.as_str().is_empty()
        || identity.binding_id.as_str().is_empty()
        || identity.base_instance_id.as_str().is_empty()
        || ready_sign_id.as_str().is_empty()
    {
        return Err(BleLineAdmissionError::EmptyIdentity);
    }

    let binding = LinkBinding {
        binding_id: identity.binding_id.clone(),
        source: LinkEndpoint {
            host_id: identity.source_host_id,
            boot_id: identity.source_boot_id,
            endpoint_id: identity.source_endpoint_id,
        },
        sink: LinkEndpoint {
            host_id: identity.sink_host_id,
            boot_id: identity.sink_boot_id,
            endpoint_id: identity.sink_endpoint_id,
        },
        base: BaseImplementationId::from("conduit.base/bluetooth-le-gatt@1"),
        base_instance_id: identity.base_instance_id,
        credential: identity.credential,
        authority: identity.authority,
        limits,
    };
    Ok(LineOffer {
        line_id: identity.line_id.clone(),
        availability: LineAvailabilitySign {
            line_id: identity.line_id,
            binding_id: identity.binding_id,
            availability: LineAvailability::Ready,
            sign_id: ready_sign_id,
        },
        binding,
        contract: BleGattProfile::line_contract(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedBluetoothLineBasis {
    pub line: conduit_core::AdmittedLine,
    pub address: [u8; 6],
    pub address_kind: crate::BluetoothAddressKind,
    pub discovery_sign_id: SignId,
    pub state_sign_id: SignId,
    pub peer_host_id: HostId,
    pub peer_boot_id: BootId,
    pub profile: BleGattProfile,
}

impl AdmittedBluetoothLineBasis {
    pub fn from_offer(
        offer: &LineOffer,
        observation: &BluetoothLineObservation,
        profile: BleGattProfile,
    ) -> Result<Self, BleLineAdmissionError> {
        let peer = observation
            .negotiated_peer
            .as_ref()
            .ok_or(BleLineAdmissionError::MissingPeerIdentity)?;
        if offer.binding.base != BaseImplementationId::from("conduit.base/bluetooth-le-gatt@1")
            || offer.binding.base_instance_id != observation.base_instance_id
        {
            return Err(BleLineAdmissionError::BaseInstanceMismatch);
        }
        if observation.state != BluetoothLineState::Ready {
            return Err(BleLineAdmissionError::NotReady);
        }
        if offer.binding.sink.host_id != peer.host_id {
            return Err(BleLineAdmissionError::HostMismatch);
        }
        if offer.binding.sink.boot_id != peer.boot_id {
            return Err(BleLineAdmissionError::BootMismatch);
        }
        Ok(Self {
            line: offer.admitted_line(),
            address: observation.candidate.address,
            address_kind: observation.candidate.address_kind,
            discovery_sign_id: observation.candidate.observation_sign_id.clone(),
            state_sign_id: observation.state_sign_id.clone(),
            peer_host_id: peer.host_id.clone(),
            peer_boot_id: peer.boot_id.clone(),
            profile: profile
                .validate()
                .map_err(BleLineAdmissionError::InvalidProfile)?,
        })
    }

    pub fn validate_current(
        &self,
        observation: &BluetoothLineObservation,
    ) -> Result<(), crate::BluetoothObservationError> {
        if observation.state == BluetoothLineState::Lost {
            return Err(crate::BluetoothObservationError::Lost);
        }
        if observation.state != BluetoothLineState::Ready {
            return Err(crate::BluetoothObservationError::NotReady);
        }
        if !observation.candidate.advertises_conduit_service {
            return Err(crate::BluetoothObservationError::MissingConduitService);
        }
        if observation.pairing == BluetoothPairingState::Unpaired {
            return Err(crate::BluetoothObservationError::NotPaired);
        }
        if observation.base_instance_id != self.line.binding.base_instance_id {
            return Err(crate::BluetoothObservationError::StaleBaseInstance);
        }
        if observation.candidate.address != self.address {
            return Err(crate::BluetoothObservationError::AddressMismatch);
        }
        if observation.candidate.address_kind != self.address_kind {
            return Err(crate::BluetoothObservationError::AddressKindMismatch);
        }
        let peer = observation
            .negotiated_peer
            .as_ref()
            .ok_or(crate::BluetoothObservationError::PeerBootMismatch)?;
        if peer.host_id != self.peer_host_id {
            return Err(crate::BluetoothObservationError::PeerHostMismatch);
        }
        if peer.boot_id != self.peer_boot_id {
            return Err(crate::BluetoothObservationError::PeerBootMismatch);
        }
        Ok(())
    }
}
