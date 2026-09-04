//! Create OI sensor lowering into portable robotics observations.
//!
//! Group 0 carries several meanings but not every fact needed to complete
//! them. In particular, cliff signal strengths and charging sources are
//! separate packets. This module keeps those absences explicit.

use crate::CreateOiPacket;
use conduit_core::InfoDecodeError;
use conduit_create_oi::{
    Create1BatteryEstimate, Create1BatteryNormalizationDisposition,
    NormalizedCreate1BatteryEstimate,
};
use conduit_robotics::{
    BatteryObservation, BeaconKind, BeaconObservation, ButtonSetObservation, ChargingObservation,
    ChargingState, CliffObservation, ContactObservation, ProximityObservation,
    WheelDropObservation, BODY_SECTOR_FRONT_LEFT, BODY_SECTOR_FRONT_RIGHT, BODY_SECTOR_LEFT,
    BODY_SECTOR_RIGHT, CHARGING_SOURCE_MASK, WHEEL_CASTER, WHEEL_LEFT, WHEEL_RIGHT,
};

pub const CREATE_GROUP_ZERO_PACKET_ID: u8 = 0;
pub const CREATE_CHARGING_SOURCES_PACKET_ID: u8 = 34;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateSensorLoweringError {
    WrongPacket { expected: u8, actual: u8 },
    Semantic(InfoDecodeError),
}

impl From<InfoDecodeError> for CreateSensorLoweringError {
    fn from(value: InfoDecodeError) -> Self {
        Self::Semantic(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateChargingSample {
    pub state: ChargingState,
    pub millivolts: u16,
    pub milliamps: i16,
    pub temperature_celsius: i8,
    pub charge_mah: u16,
    pub capacity_mah: u16,
}

impl CreateChargingSample {
    pub const fn normalized_battery(self) -> NormalizedCreate1BatteryEstimate {
        Create1BatteryEstimate {
            reported_charge_mah: self.charge_mah,
            reported_capacity_mah: self.capacity_mah,
        }
        .normalize()
    }

    pub fn with_sources(
        self,
        sources: CreateChargingSources,
    ) -> Result<ChargingObservation, CreateSensorLoweringError> {
        let battery = self.normalized_battery();
        Ok(ChargingObservation {
            state: self.state,
            sources: sources.bits,
            millivolts: self.millivolts,
            milliamps: self.milliamps,
            temperature_celsius: self.temperature_celsius,
            charge_mah: battery.charge_mah,
            capacity_mah: battery.capacity_mah,
        }
        .new()?)
    }

    /// Exact integer ratio rounded down. Zero capacity is absence, not a
    /// fabricated empty battery.
    pub fn battery(self) -> Result<Option<BatteryObservation>, CreateSensorLoweringError> {
        let normalized = self.normalized_battery();
        if normalized.disposition
            == Create1BatteryNormalizationDisposition::EstimatedCapacityUnavailable
        {
            return Ok(None);
        }
        let permille =
            u32::from(normalized.charge_mah) * 1_000 / u32::from(normalized.capacity_mah);
        Ok(Some(BatteryObservation::new(
            u16::try_from(permille).expect("bounded battery ratio"),
            self.millivolts,
        )?))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateChargingSources {
    bits: u8,
}

impl CreateChargingSources {
    pub const fn bits(self) -> u8 {
        self.bits
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateGroupZeroObservation {
    pub contact: ContactObservation,
    pub cliff: CliffObservation,
    pub wheel_drop: WheelDropObservation,
    pub proximity: ProximityObservation,
    pub virtual_wall: Option<BeaconObservation>,
    pub infrared: Option<BeaconObservation>,
    pub buttons: ButtonSetObservation,
    pub charging: CreateChargingSample,
    /// Create mechanism delta since the preceding packet request. It is not
    /// portable start-local odometry until an admitted accumulator integrates it.
    pub distance_delta_mm: i16,
    /// Create mechanism delta in degrees since the preceding packet request.
    pub angle_delta_degrees: i16,
}

pub fn lower_group_zero(
    packet: &CreateOiPacket,
) -> Result<CreateGroupZeroObservation, CreateSensorLoweringError> {
    require_packet(packet, CREATE_GROUP_ZERO_PACKET_ID)?;
    let bytes = packet.bytes();
    let bump_and_drop = bytes[0];
    let contact_sectors = active_bit(bump_and_drop & (1 << 1) != 0, BODY_SECTOR_FRONT_LEFT)
        | active_bit(bump_and_drop & 1 != 0, BODY_SECTOR_FRONT_RIGHT);
    let dropped_wheels = active_bit(bump_and_drop & (1 << 3) != 0, WHEEL_LEFT)
        | active_bit(bump_and_drop & (1 << 2) != 0, WHEEL_RIGHT)
        | active_bit(bump_and_drop & (1 << 4) != 0, WHEEL_CASTER);
    let cliff_sectors = active_bit(bytes[2] != 0, BODY_SECTOR_LEFT)
        | active_bit(bytes[3] != 0, BODY_SECTOR_FRONT_LEFT)
        | active_bit(bytes[4] != 0, BODY_SECTOR_FRONT_RIGHT)
        | active_bit(bytes[5] != 0, BODY_SECTOR_RIGHT);
    let proximity_sectors = active_bit(bytes[1] != 0, BODY_SECTOR_RIGHT);
    let virtual_wall = (bytes[6] != 0)
        .then(|| BeaconObservation::new(BeaconKind::VirtualWall, 0))
        .transpose()?;
    let infrared = (bytes[10] != 0)
        .then(|| BeaconObservation::new(BeaconKind::InfraredCode, bytes[10]))
        .transpose()?;
    let charging = CreateChargingSample {
        state: ChargingState::try_from(bytes[16])?,
        millivolts: u16::from_be_bytes([bytes[17], bytes[18]]),
        milliamps: i16::from_be_bytes([bytes[19], bytes[20]]),
        temperature_celsius: bytes[21] as i8,
        charge_mah: u16::from_be_bytes([bytes[22], bytes[23]]),
        capacity_mah: u16::from_be_bytes([bytes[24], bytes[25]]),
    };
    charging.battery()?;
    Ok(CreateGroupZeroObservation {
        contact: ContactObservation::new(contact_sectors)?,
        cliff: CliffObservation::new(cliff_sectors, 0, [0; 4])?,
        wheel_drop: WheelDropObservation::new(dropped_wheels)?,
        proximity: ProximityObservation::new(proximity_sectors)?,
        virtual_wall,
        infrared,
        buttons: ButtonSetObservation::new(u32::from(bytes[11])),
        charging,
        distance_delta_mm: i16::from_be_bytes([bytes[12], bytes[13]]),
        angle_delta_degrees: i16::from_be_bytes([bytes[14], bytes[15]]),
    })
}

const fn active_bit(active: bool, bit: u8) -> u8 {
    if active {
        bit
    } else {
        0
    }
}

pub fn lower_charging_sources(
    packet: &CreateOiPacket,
) -> Result<CreateChargingSources, CreateSensorLoweringError> {
    require_packet(packet, CREATE_CHARGING_SOURCES_PACKET_ID)?;
    let bits = packet.bytes()[0];
    if bits & !CHARGING_SOURCE_MASK != 0 {
        return Err(CreateSensorLoweringError::Semantic(
            InfoDecodeError::ReservedValue {
                field: "charging-sources",
                actual: bits & !CHARGING_SOURCE_MASK,
            },
        ));
    }
    Ok(CreateChargingSources { bits })
}

fn require_packet(packet: &CreateOiPacket, expected: u8) -> Result<(), CreateSensorLoweringError> {
    if packet.packet_id == expected {
        Ok(())
    } else {
        Err(CreateSensorLoweringError::WrongPacket {
            expected,
            actual: packet.packet_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{decode_stream_frame, CreateOiFailure};
    use conduit_robotics::{CHARGING_SOURCE_HOME_BASE, CHARGING_SOURCE_INTERNAL};

    fn frame(packet_id: u8, payload: &[u8]) -> Vec<u8> {
        let mut frame = vec![19, payload.len() as u8 + 1, packet_id];
        frame.extend_from_slice(payload);
        let sum = frame.iter().fold(0_u8, |sum, byte| sum.wrapping_add(*byte));
        frame.push(0_u8.wrapping_sub(sum));
        frame
    }

    fn group_zero() -> [u8; 26] {
        let mut bytes = [0_u8; 26];
        bytes[0] = 0b0001_1011;
        bytes[1] = 1;
        bytes[2] = 1;
        bytes[4] = 1;
        bytes[6] = 1;
        bytes[10] = 137;
        // Create 1 packet 18 exposes Play on bit 0 and Advance on bit 2.
        // Reserved bits must not make a lowering fixture protocol-invalid.
        bytes[11] = 0b0101;
        bytes[12..14].copy_from_slice(&(-120_i16).to_be_bytes());
        bytes[14..16].copy_from_slice(&(30_i16).to_be_bytes());
        bytes[16] = ChargingState::Trickle as u8;
        bytes[17..19].copy_from_slice(&14_200_u16.to_be_bytes());
        bytes[19..21].copy_from_slice(&(-240_i16).to_be_bytes());
        bytes[21] = 31;
        bytes[22..24].copy_from_slice(&1_200_u16.to_be_bytes());
        bytes[24..26].copy_from_slice(&2_400_u16.to_be_bytes());
        bytes
    }

    #[test]
    fn group_zero_lowers_each_present_meaning_without_fabricating_cliff_signals() {
        let packet = decode_stream_frame(0, &frame(0, &group_zero())).unwrap();
        let lowered = lower_group_zero(&packet).unwrap();
        assert_eq!(
            lowered.contact.active_body_sectors(),
            BODY_SECTOR_FRONT_LEFT | BODY_SECTOR_FRONT_RIGHT
        );
        assert_eq!(
            lowered.wheel_drop.dropped_wheels(),
            WHEEL_LEFT | WHEEL_CASTER
        );
        assert_eq!(
            lowered.cliff.active_sectors(),
            BODY_SECTOR_LEFT | BODY_SECTOR_FRONT_RIGHT
        );
        assert_eq!(lowered.cliff.signals(), (0, [0; 4]));
        assert_eq!(lowered.proximity.active_body_sectors(), BODY_SECTOR_RIGHT);
        assert_eq!(
            lowered.virtual_wall,
            Some(BeaconObservation::new(BeaconKind::VirtualWall, 0).unwrap())
        );
        assert_eq!(lowered.infrared.unwrap().code, 137);
        assert_eq!(lowered.buttons.pressed(), 0b0101);
        assert_eq!(
            (lowered.distance_delta_mm, lowered.angle_delta_degrees),
            (-120, 30)
        );
        assert_eq!(
            lowered
                .charging
                .battery()
                .unwrap()
                .unwrap()
                .charge_permille(),
            500
        );
    }

    #[test]
    fn charging_sources_are_a_separate_required_packet() {
        let group = decode_stream_frame(0, &frame(0, &group_zero())).unwrap();
        let lowered = lower_group_zero(&group).unwrap();
        let sources = decode_stream_frame(34, &frame(34, &[0b11])).unwrap();
        let sources = lower_charging_sources(&sources).unwrap();
        assert_eq!(
            sources.bits(),
            CHARGING_SOURCE_INTERNAL | CHARGING_SOURCE_HOME_BASE
        );
        let charging = lowered.charging.with_sources(sources).unwrap();
        assert_eq!(charging.sources, 0b11);
        assert_eq!(charging.milliamps, -240);
    }

    #[test]
    fn packet_identity_and_codec_validation_fail_closed() {
        let group = decode_stream_frame(0, &frame(0, &group_zero())).unwrap();
        assert!(matches!(
            lower_charging_sources(&group),
            Err(CreateSensorLoweringError::WrongPacket { .. })
        ));
        let mut malformed = frame(34, &[4]);
        let checksum = malformed.len() - 1;
        malformed[checksum] = malformed[checksum].wrapping_sub(4);
        assert_eq!(
            decode_stream_frame(34, &malformed),
            Err(CreateOiFailure::MalformedFrame)
        );
    }
}
