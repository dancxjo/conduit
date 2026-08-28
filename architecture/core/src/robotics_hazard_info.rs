//! Portable robotics hazard and charging observations.
//!
//! These values describe meaning only. Their identities contain no Create OI
//! packet, Host, GPIO, UART, or Pete facts. Observation freshness,
//! producing Host/Boot, clock, and Sign provenance remain in the enclosing
//! observation/Port evidence rather than being invented inside the value.

use crate::info::semantic_digest;
use crate::InfoDecodeError;

pub const ROBOTICS_CONTACT_INFO_ID: &str = "robotics/contact-body-sectors@1";
pub const ROBOTICS_CLIFF_INFO_ID: &str = "robotics/cliff-body-sectors@1";
pub const ROBOTICS_WHEEL_DROP_INFO_ID: &str = "robotics/wheel-drop-body-wheels@1";
pub const ROBOTICS_CHARGING_INFO_ID: &str = "robotics/charging-electrical@1";

pub const ROBOTICS_CONTACT_ENCODED_LEN: usize = 1;
pub const ROBOTICS_CLIFF_ENCODED_LEN: usize = 10;
pub const ROBOTICS_WHEEL_DROP_ENCODED_LEN: usize = 1;
pub const ROBOTICS_CHARGING_ENCODED_LEN: usize = 12;

pub const BODY_SECTOR_LEFT: u8 = 1 << 0;
pub const BODY_SECTOR_FRONT_LEFT: u8 = 1 << 1;
pub const BODY_SECTOR_FRONT_RIGHT: u8 = 1 << 2;
pub const BODY_SECTOR_RIGHT: u8 = 1 << 3;
pub const BODY_SECTOR_REAR: u8 = 1 << 4;
pub const BODY_SECTOR_MASK: u8 = BODY_SECTOR_LEFT
    | BODY_SECTOR_FRONT_LEFT
    | BODY_SECTOR_FRONT_RIGHT
    | BODY_SECTOR_RIGHT
    | BODY_SECTOR_REAR;

pub const WHEEL_LEFT: u8 = 1 << 0;
pub const WHEEL_RIGHT: u8 = 1 << 1;
pub const WHEEL_CASTER: u8 = 1 << 2;
pub const WHEEL_MASK: u8 = WHEEL_LEFT | WHEEL_RIGHT | WHEEL_CASTER;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContactObservation {
    active_body_sectors: u8,
}

impl ContactObservation {
    pub fn new(active_body_sectors: u8) -> Result<Self, InfoDecodeError> {
        reject_reserved("active-body-sectors", active_body_sectors, BODY_SECTOR_MASK)?;
        Ok(Self {
            active_body_sectors,
        })
    }

    pub const fn active_body_sectors(self) -> u8 {
        self.active_body_sectors
    }

    pub const fn encode(self) -> [u8; ROBOTICS_CONTACT_ENCODED_LEN] {
        [self.active_body_sectors]
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, InfoDecodeError> {
        exact_len(encoded, ROBOTICS_CONTACT_ENCODED_LEN)?;
        Self::new(encoded[0])
    }

    pub fn semantic_digest(self) -> [u8; 32] {
        semantic_digest(ROBOTICS_CONTACT_INFO_ID, &self.encode())
    }
}

/// Four exact cliff detectors in body order: left, front-left, front-right,
/// right. Signal values are meaningful only when their matching bit is set in
/// `signal_available`; unavailable is never encoded as a fabricated zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CliffObservation {
    active_sectors: u8,
    signal_available: u8,
    signals: [u16; 4],
}

impl CliffObservation {
    const CLIFF_SECTOR_MASK: u8 =
        BODY_SECTOR_LEFT | BODY_SECTOR_FRONT_LEFT | BODY_SECTOR_FRONT_RIGHT | BODY_SECTOR_RIGHT;

    pub fn new(
        active_sectors: u8,
        signal_available: u8,
        signals: [u16; 4],
    ) -> Result<Self, InfoDecodeError> {
        reject_reserved(
            "active-cliff-sectors",
            active_sectors,
            Self::CLIFF_SECTOR_MASK,
        )?;
        reject_reserved(
            "available-cliff-signals",
            signal_available,
            Self::CLIFF_SECTOR_MASK,
        )?;
        for (index, signal) in signals.iter().enumerate() {
            let bit = 1_u8 << index;
            if signal_available & bit == 0 && *signal != 0 {
                return Err(InfoDecodeError::InconsistentValue(
                    "unavailable cliff signal must use canonical zero",
                ));
            }
        }
        Ok(Self {
            active_sectors,
            signal_available,
            signals,
        })
    }

    pub const fn active_sectors(self) -> u8 {
        self.active_sectors
    }

    pub const fn signals(self) -> (u8, [u16; 4]) {
        (self.signal_available, self.signals)
    }

    pub fn encode(self) -> [u8; ROBOTICS_CLIFF_ENCODED_LEN] {
        let [left, front_left, front_right, right] = self.signals.map(u16::to_le_bytes);
        [
            self.active_sectors,
            self.signal_available,
            left[0],
            left[1],
            front_left[0],
            front_left[1],
            front_right[0],
            front_right[1],
            right[0],
            right[1],
        ]
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, InfoDecodeError> {
        exact_len(encoded, ROBOTICS_CLIFF_ENCODED_LEN)?;
        Self::new(
            encoded[0],
            encoded[1],
            [
                u16::from_le_bytes([encoded[2], encoded[3]]),
                u16::from_le_bytes([encoded[4], encoded[5]]),
                u16::from_le_bytes([encoded[6], encoded[7]]),
                u16::from_le_bytes([encoded[8], encoded[9]]),
            ],
        )
    }

    pub fn semantic_digest(self) -> [u8; 32] {
        semantic_digest(ROBOTICS_CLIFF_INFO_ID, &self.encode())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WheelDropObservation {
    dropped_wheels: u8,
}

impl WheelDropObservation {
    pub fn new(dropped_wheels: u8) -> Result<Self, InfoDecodeError> {
        reject_reserved("dropped-wheels", dropped_wheels, WHEEL_MASK)?;
        Ok(Self { dropped_wheels })
    }

    pub const fn dropped_wheels(self) -> u8 {
        self.dropped_wheels
    }

    pub const fn encode(self) -> [u8; ROBOTICS_WHEEL_DROP_ENCODED_LEN] {
        [self.dropped_wheels]
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, InfoDecodeError> {
        exact_len(encoded, ROBOTICS_WHEEL_DROP_ENCODED_LEN)?;
        Self::new(encoded[0])
    }

    pub fn semantic_digest(self) -> [u8; 32] {
        semantic_digest(ROBOTICS_WHEEL_DROP_INFO_ID, &self.encode())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum ChargingState {
    NotCharging = 0,
    Reconditioning = 1,
    Full = 2,
    Trickle = 3,
    Waiting = 4,
    Fault = 5,
}

impl TryFrom<u8> for ChargingState {
    type Error = InfoDecodeError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::NotCharging),
            1 => Ok(Self::Reconditioning),
            2 => Ok(Self::Full),
            3 => Ok(Self::Trickle),
            4 => Ok(Self::Waiting),
            5 => Ok(Self::Fault),
            other => Err(InfoDecodeError::NonCanonicalEnum(other)),
        }
    }
}

pub const CHARGING_SOURCE_INTERNAL: u8 = 1 << 0;
pub const CHARGING_SOURCE_HOME_BASE: u8 = 1 << 1;
pub const CHARGING_SOURCE_MASK: u8 = CHARGING_SOURCE_INTERNAL | CHARGING_SOURCE_HOME_BASE;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChargingObservation {
    pub state: ChargingState,
    pub sources: u8,
    pub millivolts: u16,
    pub milliamps: i16,
    pub temperature_celsius: i8,
    pub charge_mah: u16,
    pub capacity_mah: u16,
}

impl ChargingObservation {
    pub fn new(self) -> Result<Self, InfoDecodeError> {
        reject_reserved("charging-sources", self.sources, CHARGING_SOURCE_MASK)?;
        if self.capacity_mah == 0 && self.charge_mah != 0 {
            return Err(InfoDecodeError::InconsistentValue(
                "charge requires nonzero capacity",
            ));
        }
        if self.charge_mah > self.capacity_mah {
            return Err(InfoDecodeError::InconsistentValue(
                "charge exceeds capacity",
            ));
        }
        Ok(self)
    }

    pub fn encode(self) -> [u8; ROBOTICS_CHARGING_ENCODED_LEN] {
        let voltage = self.millivolts.to_le_bytes();
        let current = self.milliamps.to_le_bytes();
        let charge = self.charge_mah.to_le_bytes();
        let capacity = self.capacity_mah.to_le_bytes();
        [
            self.state as u8,
            self.sources,
            voltage[0],
            voltage[1],
            current[0],
            current[1],
            self.temperature_celsius as u8,
            0,
            charge[0],
            charge[1],
            capacity[0],
            capacity[1],
        ]
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, InfoDecodeError> {
        exact_len(encoded, ROBOTICS_CHARGING_ENCODED_LEN)?;
        if encoded[7] != 0 {
            return Err(InfoDecodeError::ReservedValue {
                field: "charging-reserved",
                actual: encoded[7],
            });
        }
        Self {
            state: ChargingState::try_from(encoded[0])?,
            sources: encoded[1],
            millivolts: u16::from_le_bytes([encoded[2], encoded[3]]),
            milliamps: i16::from_le_bytes([encoded[4], encoded[5]]),
            temperature_celsius: encoded[6] as i8,
            charge_mah: u16::from_le_bytes([encoded[8], encoded[9]]),
            capacity_mah: u16::from_le_bytes([encoded[10], encoded[11]]),
        }
        .new()
    }

    pub fn semantic_digest(self) -> [u8; 32] {
        semantic_digest(ROBOTICS_CHARGING_INFO_ID, &self.encode())
    }
}

fn exact_len(encoded: &[u8], expected: usize) -> Result<(), InfoDecodeError> {
    if encoded.len() == expected {
        Ok(())
    } else {
        Err(InfoDecodeError::WrongLength {
            expected,
            actual: encoded.len(),
        })
    }
}

fn reject_reserved(field: &'static str, actual: u8, allowed: u8) -> Result<(), InfoDecodeError> {
    let reserved = actual & !allowed;
    if reserved == 0 {
        Ok(())
    } else {
        Err(InfoDecodeError::ReservedValue {
            field,
            actual: reserved,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sector_and_wheel_bits_are_exact_and_reserved_bits_refuse() {
        let contact = ContactObservation::new(BODY_SECTOR_LEFT | BODY_SECTOR_RIGHT).unwrap();
        assert_eq!(ContactObservation::decode(&contact.encode()), Ok(contact));
        assert!(matches!(
            ContactObservation::new(0x80),
            Err(InfoDecodeError::ReservedValue { .. })
        ));
        assert!(WheelDropObservation::new(WHEEL_LEFT | WHEEL_CASTER).is_ok());
        assert!(WheelDropObservation::new(0x08).is_err());
    }

    #[test]
    fn unavailable_cliff_signal_cannot_masquerade_as_observed_zero() {
        assert!(matches!(
            CliffObservation::new(BODY_SECTOR_LEFT, 0, [12, 0, 0, 0]),
            Err(InfoDecodeError::InconsistentValue(_))
        ));
        let observed = CliffObservation::new(
            BODY_SECTOR_FRONT_LEFT,
            BODY_SECTOR_LEFT | BODY_SECTOR_FRONT_LEFT,
            [0, 42, 0, 0],
        )
        .unwrap();
        assert_eq!(CliffObservation::decode(&observed.encode()), Ok(observed));
    }

    #[test]
    fn charging_shape_is_canonical_and_bounded_by_real_capacity() {
        let observed = ChargingObservation {
            state: ChargingState::Trickle,
            sources: CHARGING_SOURCE_HOME_BASE,
            millivolts: 14_200,
            milliamps: 240,
            temperature_celsius: 31,
            charge_mah: 1_200,
            capacity_mah: 2_400,
        }
        .new()
        .unwrap();
        assert_eq!(
            ChargingObservation::decode(&observed.encode()),
            Ok(observed)
        );
        assert!(ChargingObservation {
            charge_mah: 2,
            capacity_mah: 1,
            ..observed
        }
        .new()
        .is_err());
        let mut noncanonical = observed.encode();
        noncanonical[7] = 1;
        assert!(matches!(
            ChargingObservation::decode(&noncanonical),
            Err(InfoDecodeError::ReservedValue { .. })
        ));
    }

    #[test]
    fn each_semantic_shape_has_a_distinct_identity_and_digest_domain() {
        let contact = ContactObservation::new(0).unwrap();
        let wheel = WheelDropObservation::new(0).unwrap();
        assert_ne!(ROBOTICS_CONTACT_INFO_ID, ROBOTICS_WHEEL_DROP_INFO_ID);
        assert_ne!(contact.semantic_digest(), wheel.semantic_digest());
    }
}
