use alloc::string::String;

use conduit_core::{BootId, ConnectionBaseInstanceId, HostId, SignId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BluetoothControllerId(String);

impl BluetoothControllerId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for BluetoothControllerId {
    fn from(value: &str) -> Self {
        Self(value.into())
    }
}

impl From<String> for BluetoothControllerId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BluetoothAddressKind {
    Public,
    RandomStatic,
    ResolvablePrivate,
    NonResolvablePrivate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BluetoothDiscoveryCandidate {
    pub observation_sign_id: SignId,
    pub controller_id: BluetoothControllerId,
    pub address: [u8; 6],
    pub address_kind: BluetoothAddressKind,
    pub advertises_conduit_service: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BluetoothPairingState {
    Unpaired,
    Paired,
    Bonded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NegotiatedPeerIdentity {
    pub host_id: HostId,
    pub boot_id: BootId,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BluetoothLineState {
    Discovered,
    Connecting,
    Negotiating,
    Ready,
    Lost,
}

/// Current realization evidence. It is deliberately not embedded in an
/// authored Form or semantic Cord and does not derive Host identity from a
/// Bluetooth address or name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BluetoothLineObservation {
    pub candidate: BluetoothDiscoveryCandidate,
    /// Fresh boot-scoped instance for this exact controller/address/session
    /// observation. Address or peer-Boot changes must rotate this identity.
    pub base_instance_id: ConnectionBaseInstanceId,
    pub pairing: BluetoothPairingState,
    pub state: BluetoothLineState,
    pub negotiated_peer: Option<NegotiatedPeerIdentity>,
    pub state_sign_id: SignId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BluetoothObservationError {
    AddressChangedWithoutFreshBase,
    PeerBootChangedWithoutFreshBase,
    PeerHostChangedWithoutFreshBase,
    LostRealizationReused,
    StaleBaseInstance,
    AddressMismatch,
    AddressKindMismatch,
    PeerHostMismatch,
    PeerBootMismatch,
    Lost,
    NotReady,
    MissingConduitService,
    NotPaired,
}

impl BluetoothLineObservation {
    pub fn validate_successor(&self, next: &Self) -> Result<(), BluetoothObservationError> {
        if self.state == BluetoothLineState::Lost
            && next.state != BluetoothLineState::Lost
            && self.base_instance_id == next.base_instance_id
        {
            return Err(BluetoothObservationError::LostRealizationReused);
        }
        if (self.candidate.address != next.candidate.address
            || self.candidate.address_kind != next.candidate.address_kind)
            && self.base_instance_id == next.base_instance_id
        {
            return Err(BluetoothObservationError::AddressChangedWithoutFreshBase);
        }
        if self.negotiated_peer.as_ref().map(|peer| &peer.boot_id)
            != next.negotiated_peer.as_ref().map(|peer| &peer.boot_id)
            && self.base_instance_id == next.base_instance_id
        {
            return Err(BluetoothObservationError::PeerBootChangedWithoutFreshBase);
        }
        if self.negotiated_peer.as_ref().map(|peer| &peer.host_id)
            != next.negotiated_peer.as_ref().map(|peer| &peer.host_id)
            && self.base_instance_id == next.base_instance_id
        {
            return Err(BluetoothObservationError::PeerHostChangedWithoutFreshBase);
        }
        Ok(())
    }
}
