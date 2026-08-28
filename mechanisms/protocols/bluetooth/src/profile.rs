use conduit_core::{
    LineContinuation, LineContract, LineDuplex, LineOrdering, LineReliability, LineScope,
    LineSecurity, LineTrafficShape, LinkLimits,
};
use serde::{Deserialize, Serialize};

use crate::{MAXIMUM_BLE_FRAME_BYTES, MAXIMUM_BLE_GATT_PACKET_BYTES};

pub const BLE_ATT_HEADER_BYTES: u16 = 3;
pub const BLE_FRAGMENT_HEADER_BYTES: u16 = 7;

/// Buffering below Conduit's acquired BlueZ I/O boundary is finite platform
/// state, but BlueZ does not expose a byte ceiling that Conduit can admit.
/// It therefore cannot be counted as available Conduit queue capacity.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BluetoothHostStackBuffering {
    ExternallyManagedUnmeasured,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BleGattProfile {
    pub negotiated_att_mtu: u16,
    pub maximum_gatt_packet_bytes: u16,
    pub maximum_frame_bytes: u32,
    pub maximum_fragments_per_frame: u8,
    pub maximum_in_flight_items: u16,
    pub maximum_payload_bytes: u16,
    pub implementation_staging_bytes: u32,
    pub host_stack_buffering: BluetoothHostStackBuffering,
    pub maximum_reconnect_attempts: u8,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BleProfileError {
    InvalidMtu,
    InvalidFrameLimit,
    InvalidItemLimit,
    InvalidPayloadLimit,
    InvalidBufferLimit,
    AutomaticReconnectForbidden,
}

impl BleGattProfile {
    pub const FIRST: Self = Self {
        negotiated_att_mtu: 185,
        maximum_gatt_packet_bytes: 182,
        maximum_frame_bytes: 2_048,
        maximum_fragments_per_frame: 12,
        maximum_in_flight_items: 1,
        maximum_payload_bytes: 96,
        implementation_staging_bytes: 4_096,
        host_stack_buffering: BluetoothHostStackBuffering::ExternallyManagedUnmeasured,
        maximum_reconnect_attempts: 0,
    };

    pub fn validate(self) -> Result<Self, BleProfileError> {
        if self.negotiated_att_mtu <= BLE_ATT_HEADER_BYTES {
            return Err(BleProfileError::InvalidMtu);
        }
        if self.maximum_gatt_packet_bytes == 0
            || usize::from(self.maximum_gatt_packet_bytes) > MAXIMUM_BLE_GATT_PACKET_BYTES
            || self.maximum_gatt_packet_bytes
                > self.negotiated_att_mtu.saturating_sub(BLE_ATT_HEADER_BYTES)
        {
            return Err(BleProfileError::InvalidFrameLimit);
        }
        let fragment_payload = self
            .maximum_gatt_packet_bytes
            .saturating_sub(BLE_FRAGMENT_HEADER_BYTES);
        if self.maximum_frame_bytes == 0
            || usize::try_from(self.maximum_frame_bytes).unwrap_or(usize::MAX)
                > MAXIMUM_BLE_FRAME_BYTES
            || self.maximum_fragments_per_frame == 0
            || self.maximum_frame_bytes
                > u32::from(fragment_payload)
                    .saturating_mul(u32::from(self.maximum_fragments_per_frame))
        {
            return Err(BleProfileError::InvalidFrameLimit);
        }
        if self.maximum_in_flight_items == 0 {
            return Err(BleProfileError::InvalidItemLimit);
        }
        if self.maximum_payload_bytes == 0
            || u32::from(self.maximum_payload_bytes) > self.maximum_frame_bytes
        {
            return Err(BleProfileError::InvalidPayloadLimit);
        }
        let minimum_staging = self
            .maximum_frame_bytes
            .saturating_mul(u32::from(self.maximum_in_flight_items));
        if self.implementation_staging_bytes < minimum_staging {
            return Err(BleProfileError::InvalidBufferLimit);
        }
        if self.maximum_reconnect_attempts != 0 {
            return Err(BleProfileError::AutomaticReconnectForbidden);
        }
        Ok(self)
    }

    pub fn link_limits(self) -> Result<LinkLimits, BleProfileError> {
        let profile = self.validate()?;
        Ok(LinkLimits {
            maximum_in_flight_items: profile.maximum_in_flight_items,
            maximum_payload_bytes: u32::from(profile.maximum_payload_bytes),
            maximum_buffered_bytes: profile.implementation_staging_bytes,
            maximum_frame_bytes: profile.maximum_frame_bytes,
        })
    }

    pub const fn line_contract() -> LineContract {
        LineContract {
            scope: LineScope::PointToPoint,
            traffic_shape: LineTrafficShape::Message,
            duplex: LineDuplex::FullDuplex,
            ordering: LineOrdering::Ordered,
            // AcquireWrite uses ATT Write Commands and the return path uses
            // notifications. Neither operation acknowledges delivery.
            reliability: LineReliability::BestEffort,
            continuation: LineContinuation::None,
            // Pairing is an admission observation, not a claim that every
            // packet crossed a currently authenticated encrypted link.
            security: LineSecurity::PlaintextNetwork,
        }
    }
}
