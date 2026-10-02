//! Portable robotics hazard and charging observations.
//!
//! These values describe meaning only. Their identities contain no Create OI
//! packet, Host, GPIO, UART, or Pete facts. Observation freshness,
//! producing Host/Boot, clock, and Sign provenance remain in the enclosing
//! observation/Port evidence rather than being invented inside the value.

use conduit_core::{semantic_digest, InfoDecodeError, Quantity, QuantityUnit};
use core::{cmp::Ordering, hash::Hash};

use crate::{
    ChargingObservation, CliffObservation, CliffSignal, ContactObservation, WheelDropObservation,
};

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

impl ContactObservation {
    pub fn active_body_sectors(self) -> u8 {
        *self.sectors()
    }

    pub fn encode(self) -> [u8; ROBOTICS_CONTACT_ENCODED_LEN] {
        [self.active_body_sectors()]
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, InfoDecodeError> {
        exact_len(encoded, ROBOTICS_CONTACT_ENCODED_LEN)?;
        reject_reserved("active-body-sectors", encoded[0], BODY_SECTOR_MASK)?;
        Ok(Self::new(encoded[0]).expect("codec bounds match generated contracts"))
    }

    pub fn semantic_digest(self) -> [u8; 32] {
        semantic_digest(ROBOTICS_CONTACT_INFO_ID, &self.encode())
    }
}

impl PartialOrd for ContactObservation {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ContactObservation {
    fn cmp(&self, other: &Self) -> Ordering {
        self.active_body_sectors().cmp(&other.active_body_sectors())
    }
}

impl Hash for ContactObservation {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.active_body_sectors().hash(state);
    }
}

/// Four exact cliff detectors in body order: left, front-left, front-right,
/// right. Signal values are meaningful only when their matching bit is set in
/// `signal_available`; unavailable is never encoded as a made zero.
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
        let signal = |index: usize| {
            if signal_available & (1_u8 << index) == 0 {
                CliffSignal::unavailable()
            } else {
                CliffSignal::observed(signals[index]).expect("U16 signal has no added constraint")
            }
        };
        Ok(
            Self::new_native(active_sectors, signal(0), signal(1), signal(2), signal(3))
                .expect("explicit mask checks match generated contract"),
        )
    }

    pub fn signals(self) -> (u8, [u16; 4]) {
        let mut available = 0;
        let mut signals = [0; 4];
        for (index, signal) in [
            self.left_signal(),
            self.front_left_signal(),
            self.front_right_signal(),
            self.right_signal(),
        ]
        .into_iter()
        .enumerate()
        {
            if let CliffSignal::Observed(observed) = signal {
                available |= 1_u8 << index;
                signals[index] = *observed.value();
            }
        }
        (available, signals)
    }

    pub fn encode(self) -> [u8; ROBOTICS_CLIFF_ENCODED_LEN] {
        let (signal_available, signals) = self.signals();
        let [left, front_left, front_right, right] = signals.map(u16::to_le_bytes);
        [
            self.active_sectors(),
            signal_available,
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

impl PartialOrd for CliffObservation {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for CliffObservation {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.active_sectors(), self.signals()).cmp(&(other.active_sectors(), other.signals()))
    }
}
impl Hash for CliffObservation {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.active_sectors().hash(state);
        self.signals().hash(state);
    }
}

impl WheelDropObservation {
    pub fn dropped_wheels(self) -> u8 {
        *self.wheels()
    }

    pub fn encode(self) -> [u8; ROBOTICS_WHEEL_DROP_ENCODED_LEN] {
        [self.dropped_wheels()]
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, InfoDecodeError> {
        exact_len(encoded, ROBOTICS_WHEEL_DROP_ENCODED_LEN)?;
        reject_reserved("dropped-wheels", encoded[0], WHEEL_MASK)?;
        Ok(Self::new(encoded[0]).expect("wheel-mask bound matches generated contract"))
    }

    pub fn semantic_digest(self) -> [u8; 32] {
        semantic_digest(ROBOTICS_WHEEL_DROP_INFO_ID, &self.encode())
    }
}

impl PartialOrd for WheelDropObservation {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for WheelDropObservation {
    fn cmp(&self, other: &Self) -> Ordering {
        self.dropped_wheels().cmp(&other.dropped_wheels())
    }
}

impl Hash for WheelDropObservation {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.dropped_wheels().hash(state);
    }
}

use crate::{ChargingState, ChargingStateForm};

impl TryFrom<u8> for ChargingState {
    type Error = InfoDecodeError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        ChargingStateForm::decode(&[value]).map_err(|_| InfoDecodeError::NonCanonicalEnum(value))
    }
}

impl ChargingState {
    pub const fn wire_tag(self) -> u8 {
        ChargingStateForm::encode(self)[0]
    }
}

pub const CHARGING_SOURCE_INTERNAL: u8 = 1 << 0;
pub const CHARGING_SOURCE_HOME_BASE: u8 = 1 << 1;
pub const CHARGING_SOURCE_MASK: u8 = CHARGING_SOURCE_INTERNAL | CHARGING_SOURCE_HOME_BASE;

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
            self.state.wire_tag(),
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

    pub const fn voltage(self) -> Quantity {
        Quantity::new(self.millivolts as i64, QuantityUnit::Millivolt)
    }

    pub const fn current(self) -> Quantity {
        Quantity::new(self.milliamps as i64, QuantityUnit::Milliampere)
    }

    pub const fn temperature(self) -> Quantity {
        Quantity::new(self.temperature_celsius as i64, QuantityUnit::Celsius)
    }

    pub const fn charge(self) -> Quantity {
        Quantity::new(self.charge_mah as i64, QuantityUnit::MilliampereHour)
    }

    pub const fn capacity(self) -> Quantity {
        Quantity::new(self.capacity_mah as i64, QuantityUnit::MilliampereHour)
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

impl PartialOrd for ChargingObservation {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for ChargingObservation {
    fn cmp(&self, other: &Self) -> Ordering {
        (
            self.state,
            self.sources,
            self.millivolts,
            self.milliamps,
            self.temperature_celsius,
            self.charge_mah,
            self.capacity_mah,
        )
            .cmp(&(
                other.state,
                other.sources,
                other.millivolts,
                other.milliamps,
                other.temperature_celsius,
                other.charge_mah,
                other.capacity_mah,
            ))
    }
}
impl Hash for ChargingObservation {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        (
            self.state,
            self.sources,
            self.millivolts,
            self.milliamps,
            self.temperature_celsius,
            self.charge_mah,
            self.capacity_mah,
        )
            .hash(state);
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
        assert!(ContactObservation::new(0x80).is_err());
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
        assert_eq!(
            observed.voltage(),
            Quantity::new(14_200, QuantityUnit::Millivolt)
        );
        assert_eq!(
            observed.current(),
            Quantity::new(240, QuantityUnit::Milliampere)
        );
        assert_eq!(
            observed.temperature(),
            Quantity::new(31, QuantityUnit::Celsius)
        );
        assert_eq!(
            observed.charge(),
            Quantity::new(1_200, QuantityUnit::MilliampereHour)
        );
        assert_eq!(
            observed.capacity(),
            Quantity::new(2_400, QuantityUnit::MilliampereHour)
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
