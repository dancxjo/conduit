//! Exact portable observation values for the first robotics Kind family.
//!
//! Units and reference frames are part of each Info identity. These values do
//! not imply a sensor, Host, Base, implementation, or physical observation.

use crate::info::semantic_digest;
use crate::{InfoDecodeError, Quantity, QuantityUnit};

pub const ROBOTICS_RANGE_INFO_ID: &str = "robotics/range-mm-sensor-forward@1";
pub const ROBOTICS_RANGE_ENCODED_LEN: usize = 8;
pub const ROBOTICS_ODOMETRY_INFO_ID: &str = "robotics/odometry-mm-start-local@1";
pub const ROBOTICS_ODOMETRY_ENCODED_LEN: usize = 12;
pub const ROBOTICS_BATTERY_INFO_ID: &str = "robotics/battery-permille-millivolts@1";
pub const ROBOTICS_BATTERY_ENCODED_LEN: usize = 4;
pub const ROBOTICS_ORIENTATION_INFO_ID: &str = "robotics/orientation-microrad-body@1";
pub const ROBOTICS_ORIENTATION_ENCODED_LEN: usize = 12;

pub const MAXIMUM_RANGE_MM: u32 = 1_000_000;
pub const MAXIMUM_OBSERVATION_AGE_MS: u32 = 60_000;
pub const MAXIMUM_ODOMETRY_MM: i32 = 10_000_000;
pub const PI_MICRORADIANS: i32 = 3_141_593;
pub const HALF_PI_MICRORADIANS: i32 = 1_570_797;
pub const MAXIMUM_BATTERY_MILLIVOLTS: u16 = 60_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RangeObservation {
    distance_mm: u32,
    age_ms: u32,
}

impl RangeObservation {
    pub fn from_quantities(distance: Quantity, age: Quantity) -> Result<Self, InfoDecodeError> {
        Self::new(
            quantity_u32(
                "distance-mm",
                distance,
                QuantityUnit::Millimeter,
                MAXIMUM_RANGE_MM,
            )?,
            quantity_u32(
                "age-ms",
                age,
                QuantityUnit::Millisecond,
                MAXIMUM_OBSERVATION_AGE_MS,
            )?,
        )
    }

    pub fn new(distance_mm: u32, age_ms: u32) -> Result<Self, InfoDecodeError> {
        bounded_u32("distance-mm", distance_mm, MAXIMUM_RANGE_MM)?;
        bounded_u32("age-ms", age_ms, MAXIMUM_OBSERVATION_AGE_MS)?;
        Ok(Self {
            distance_mm,
            age_ms,
        })
    }

    pub const fn distance_mm(self) -> u32 {
        self.distance_mm
    }

    pub const fn age_ms(self) -> u32 {
        self.age_ms
    }

    pub const fn distance(self) -> Quantity {
        Quantity::new(self.distance_mm as i64, QuantityUnit::Millimeter)
    }

    pub const fn age(self) -> Quantity {
        Quantity::new(self.age_ms as i64, QuantityUnit::Millisecond)
    }

    pub const fn encode(self) -> [u8; ROBOTICS_RANGE_ENCODED_LEN] {
        let distance = self.distance_mm.to_le_bytes();
        let age = self.age_ms.to_le_bytes();
        [
            distance[0],
            distance[1],
            distance[2],
            distance[3],
            age[0],
            age[1],
            age[2],
            age[3],
        ]
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, InfoDecodeError> {
        exact_len(encoded, ROBOTICS_RANGE_ENCODED_LEN)?;
        Self::new(
            u32::from_le_bytes(encoded[0..4].try_into().expect("checked range length")),
            u32::from_le_bytes(encoded[4..8].try_into().expect("checked range length")),
        )
    }

    pub fn semantic_digest(self) -> [u8; 32] {
        semantic_digest(ROBOTICS_RANGE_INFO_ID, &self.encode())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OdometryObservation {
    forward_mm: i32,
    lateral_mm: i32,
    yaw_microradians: i32,
}

impl OdometryObservation {
    pub fn new(
        forward_mm: i32,
        lateral_mm: i32,
        yaw_microradians: i32,
    ) -> Result<Self, InfoDecodeError> {
        bounded_i32(
            "forward-mm",
            forward_mm,
            -MAXIMUM_ODOMETRY_MM,
            MAXIMUM_ODOMETRY_MM,
        )?;
        bounded_i32(
            "lateral-mm",
            lateral_mm,
            -MAXIMUM_ODOMETRY_MM,
            MAXIMUM_ODOMETRY_MM,
        )?;
        bounded_i32(
            "yaw-microradians",
            yaw_microradians,
            -PI_MICRORADIANS,
            PI_MICRORADIANS,
        )?;
        Ok(Self {
            forward_mm,
            lateral_mm,
            yaw_microradians,
        })
    }

    pub const fn components(self) -> (i32, i32, i32) {
        (self.forward_mm, self.lateral_mm, self.yaw_microradians)
    }

    pub fn encode(self) -> [u8; ROBOTICS_ODOMETRY_ENCODED_LEN] {
        encode_three_i32(self.forward_mm, self.lateral_mm, self.yaw_microradians)
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, InfoDecodeError> {
        let [forward, lateral, yaw] = decode_three_i32(encoded, ROBOTICS_ODOMETRY_ENCODED_LEN)?;
        Self::new(forward, lateral, yaw)
    }

    pub fn semantic_digest(self) -> [u8; 32] {
        semantic_digest(ROBOTICS_ODOMETRY_INFO_ID, &self.encode())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BatteryObservation {
    charge_permille: u16,
    millivolts: u16,
}

impl BatteryObservation {
    pub fn from_quantities(charge: Quantity, voltage: Quantity) -> Result<Self, InfoDecodeError> {
        Self::new(
            quantity_u16("charge-permille", charge, QuantityUnit::Permille, 1_000)?,
            quantity_u16(
                "millivolts",
                voltage,
                QuantityUnit::Millivolt,
                MAXIMUM_BATTERY_MILLIVOLTS,
            )?,
        )
    }

    pub fn new(charge_permille: u16, millivolts: u16) -> Result<Self, InfoDecodeError> {
        bounded_i64("charge-permille", i64::from(charge_permille), 0, 1_000)?;
        bounded_i64(
            "millivolts",
            i64::from(millivolts),
            0,
            i64::from(MAXIMUM_BATTERY_MILLIVOLTS),
        )?;
        Ok(Self {
            charge_permille,
            millivolts,
        })
    }

    pub const fn charge_permille(self) -> u16 {
        self.charge_permille
    }

    pub const fn millivolts(self) -> u16 {
        self.millivolts
    }

    pub const fn charge(self) -> Quantity {
        Quantity::new(self.charge_permille as i64, QuantityUnit::Permille)
    }

    pub const fn voltage(self) -> Quantity {
        Quantity::new(self.millivolts as i64, QuantityUnit::Millivolt)
    }

    pub const fn encode(self) -> [u8; ROBOTICS_BATTERY_ENCODED_LEN] {
        let charge = self.charge_permille.to_le_bytes();
        let voltage = self.millivolts.to_le_bytes();
        [charge[0], charge[1], voltage[0], voltage[1]]
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, InfoDecodeError> {
        exact_len(encoded, ROBOTICS_BATTERY_ENCODED_LEN)?;
        Self::new(
            u16::from_le_bytes(encoded[0..2].try_into().expect("checked battery length")),
            u16::from_le_bytes(encoded[2..4].try_into().expect("checked battery length")),
        )
    }

    pub fn semantic_digest(self) -> [u8; 32] {
        semantic_digest(ROBOTICS_BATTERY_INFO_ID, &self.encode())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OrientationObservation {
    roll_microradians: i32,
    pitch_microradians: i32,
    yaw_microradians: i32,
}

impl OrientationObservation {
    pub fn new(
        roll_microradians: i32,
        pitch_microradians: i32,
        yaw_microradians: i32,
    ) -> Result<Self, InfoDecodeError> {
        bounded_i32(
            "roll-microradians",
            roll_microradians,
            -PI_MICRORADIANS,
            PI_MICRORADIANS,
        )?;
        bounded_i32(
            "pitch-microradians",
            pitch_microradians,
            -HALF_PI_MICRORADIANS,
            HALF_PI_MICRORADIANS,
        )?;
        bounded_i32(
            "yaw-microradians",
            yaw_microradians,
            -PI_MICRORADIANS,
            PI_MICRORADIANS,
        )?;
        Ok(Self {
            roll_microradians,
            pitch_microradians,
            yaw_microradians,
        })
    }

    pub const fn components(self) -> (i32, i32, i32) {
        (
            self.roll_microradians,
            self.pitch_microradians,
            self.yaw_microradians,
        )
    }

    pub fn encode(self) -> [u8; ROBOTICS_ORIENTATION_ENCODED_LEN] {
        encode_three_i32(
            self.roll_microradians,
            self.pitch_microradians,
            self.yaw_microradians,
        )
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, InfoDecodeError> {
        let [roll, pitch, yaw] = decode_three_i32(encoded, ROBOTICS_ORIENTATION_ENCODED_LEN)?;
        Self::new(roll, pitch, yaw)
    }

    pub fn semantic_digest(self) -> [u8; 32] {
        semantic_digest(ROBOTICS_ORIENTATION_INFO_ID, &self.encode())
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

fn quantity_u32(
    field: &'static str,
    quantity: Quantity,
    unit: QuantityUnit,
    maximum: u32,
) -> Result<u32, InfoDecodeError> {
    let converted = quantity
        .convert(unit)
        .map_err(InfoDecodeError::QuantityConversion)?;
    let value = u32::try_from(converted.value()).map_err(|_| InfoDecodeError::OutOfRange {
        field,
        minimum: 0,
        maximum: i64::from(maximum),
        actual: converted.value(),
    })?;
    bounded_u32(field, value, maximum)?;
    Ok(value)
}

fn quantity_u16(
    field: &'static str,
    quantity: Quantity,
    unit: QuantityUnit,
    maximum: u16,
) -> Result<u16, InfoDecodeError> {
    let converted = quantity
        .convert(unit)
        .map_err(InfoDecodeError::QuantityConversion)?;
    let value = u16::try_from(converted.value()).map_err(|_| InfoDecodeError::OutOfRange {
        field,
        minimum: 0,
        maximum: i64::from(maximum),
        actual: converted.value(),
    })?;
    bounded_i64(field, i64::from(value), 0, i64::from(maximum))?;
    Ok(value)
}

fn bounded_u32(field: &'static str, value: u32, maximum: u32) -> Result<(), InfoDecodeError> {
    bounded_i64(field, i64::from(value), 0, i64::from(maximum))
}

fn bounded_i32(
    field: &'static str,
    value: i32,
    minimum: i32,
    maximum: i32,
) -> Result<(), InfoDecodeError> {
    bounded_i64(
        field,
        i64::from(value),
        i64::from(minimum),
        i64::from(maximum),
    )
}

fn bounded_i64(
    field: &'static str,
    actual: i64,
    minimum: i64,
    maximum: i64,
) -> Result<(), InfoDecodeError> {
    if (minimum..=maximum).contains(&actual) {
        Ok(())
    } else {
        Err(InfoDecodeError::OutOfRange {
            field,
            minimum,
            maximum,
            actual,
        })
    }
}

fn encode_three_i32(first: i32, second: i32, third: i32) -> [u8; 12] {
    let first = first.to_le_bytes();
    let second = second.to_le_bytes();
    let third = third.to_le_bytes();
    [
        first[0], first[1], first[2], first[3], second[0], second[1], second[2], second[3],
        third[0], third[1], third[2], third[3],
    ]
}

fn decode_three_i32(encoded: &[u8], expected: usize) -> Result<[i32; 3], InfoDecodeError> {
    exact_len(encoded, expected)?;
    Ok([
        i32::from_le_bytes(encoded[0..4].try_into().expect("checked triple length")),
        i32::from_le_bytes(encoded[4..8].try_into().expect("checked triple length")),
        i32::from_le_bytes(encoded[8..12].try_into().expect("checked triple length")),
    ])
}
