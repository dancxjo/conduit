//! Portable body-relative proximity, beacon, button, and acceleration values.

use conduit_core::{semantic_digest, InfoDecodeError};

use crate::BODY_SECTOR_MASK;

pub const ROBOTICS_PROXIMITY_INFO_ID: &str = "robotics/proximity-body-sectors@1";
pub const ROBOTICS_BEACON_INFO_ID: &str = "robotics/beacon-observation@1";
pub const ROBOTICS_BUTTONS_INFO_ID: &str = "input/button-set@1";
pub const ROBOTICS_ACCELERATION_INFO_ID: &str = "robotics/acceleration-mm-s2-body@1";

pub const ROBOTICS_PROXIMITY_ENCODED_LEN: usize = 1;
pub const ROBOTICS_BEACON_ENCODED_LEN: usize = 2;
pub const ROBOTICS_BUTTONS_ENCODED_LEN: usize = 4;
pub const ROBOTICS_ACCELERATION_ENCODED_LEN: usize = 12;
pub const MAXIMUM_ACCELERATION_MM_S2: i32 = 200_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProximityObservation {
    active_body_sectors: u8,
}

impl ProximityObservation {
    pub fn new(active_body_sectors: u8) -> Result<Self, InfoDecodeError> {
        reject_reserved(
            "proximity-body-sectors",
            active_body_sectors,
            BODY_SECTOR_MASK,
        )?;
        Ok(Self {
            active_body_sectors,
        })
    }

    pub const fn active_body_sectors(self) -> u8 {
        self.active_body_sectors
    }

    pub const fn encode(self) -> [u8; ROBOTICS_PROXIMITY_ENCODED_LEN] {
        [self.active_body_sectors]
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, InfoDecodeError> {
        exact_len(encoded, ROBOTICS_PROXIMITY_ENCODED_LEN)?;
        Self::new(encoded[0])
    }

    pub fn semantic_digest(self) -> [u8; 32] {
        semantic_digest(ROBOTICS_PROXIMITY_INFO_ID, &self.encode())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum BeaconKind {
    VirtualWall = 0,
    InfraredCode = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BeaconObservation {
    pub kind: BeaconKind,
    pub code: u8,
}

impl BeaconObservation {
    pub fn new(kind: BeaconKind, code: u8) -> Result<Self, InfoDecodeError> {
        if kind == BeaconKind::VirtualWall && code != 0 {
            return Err(InfoDecodeError::InconsistentValue(
                "virtual-wall observation has no fabricated code",
            ));
        }
        Ok(Self { kind, code })
    }

    pub const fn encode(self) -> [u8; ROBOTICS_BEACON_ENCODED_LEN] {
        [self.kind as u8, self.code]
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, InfoDecodeError> {
        exact_len(encoded, ROBOTICS_BEACON_ENCODED_LEN)?;
        let kind = match encoded[0] {
            0 => BeaconKind::VirtualWall,
            1 => BeaconKind::InfraredCode,
            other => return Err(InfoDecodeError::NonCanonicalEnum(other)),
        };
        Self::new(kind, encoded[1])
    }

    pub fn semantic_digest(self) -> [u8; 32] {
        semantic_digest(ROBOTICS_BEACON_INFO_ID, &self.encode())
    }
}

/// A finite set of semantic button positions. The meaning assigned to each
/// position belongs to the exact producing implementation/Presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ButtonSetObservation(u32);

impl ButtonSetObservation {
    pub const fn new(pressed: u32) -> Self {
        Self(pressed)
    }

    pub const fn pressed(self) -> u32 {
        self.0
    }

    pub const fn encode(self) -> [u8; ROBOTICS_BUTTONS_ENCODED_LEN] {
        self.0.to_le_bytes()
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, InfoDecodeError> {
        exact_len(encoded, ROBOTICS_BUTTONS_ENCODED_LEN)?;
        Ok(Self::new(u32::from_le_bytes(
            encoded.try_into().expect("checked button-set length"),
        )))
    }

    pub fn semantic_digest(self) -> [u8; 32] {
        semantic_digest(ROBOTICS_BUTTONS_INFO_ID, &self.encode())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AccelerationObservation {
    x_forward_mm_s2: i32,
    y_left_mm_s2: i32,
    z_up_mm_s2: i32,
}

impl AccelerationObservation {
    pub fn new(
        x_forward_mm_s2: i32,
        y_left_mm_s2: i32,
        z_up_mm_s2: i32,
    ) -> Result<Self, InfoDecodeError> {
        for (field, value) in [
            ("x-forward-mm-s2", x_forward_mm_s2),
            ("y-left-mm-s2", y_left_mm_s2),
            ("z-up-mm-s2", z_up_mm_s2),
        ] {
            if !(-MAXIMUM_ACCELERATION_MM_S2..=MAXIMUM_ACCELERATION_MM_S2).contains(&value) {
                return Err(InfoDecodeError::OutOfRange {
                    field,
                    minimum: i64::from(-MAXIMUM_ACCELERATION_MM_S2),
                    maximum: i64::from(MAXIMUM_ACCELERATION_MM_S2),
                    actual: i64::from(value),
                });
            }
        }
        Ok(Self {
            x_forward_mm_s2,
            y_left_mm_s2,
            z_up_mm_s2,
        })
    }

    pub const fn components(self) -> (i32, i32, i32) {
        (self.x_forward_mm_s2, self.y_left_mm_s2, self.z_up_mm_s2)
    }

    pub fn encode(self) -> [u8; ROBOTICS_ACCELERATION_ENCODED_LEN] {
        let x = self.x_forward_mm_s2.to_le_bytes();
        let y = self.y_left_mm_s2.to_le_bytes();
        let z = self.z_up_mm_s2.to_le_bytes();
        [
            x[0], x[1], x[2], x[3], y[0], y[1], y[2], y[3], z[0], z[1], z[2], z[3],
        ]
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, InfoDecodeError> {
        exact_len(encoded, ROBOTICS_ACCELERATION_ENCODED_LEN)?;
        Self::new(
            i32::from_le_bytes(
                encoded[0..4]
                    .try_into()
                    .expect("checked acceleration length"),
            ),
            i32::from_le_bytes(
                encoded[4..8]
                    .try_into()
                    .expect("checked acceleration length"),
            ),
            i32::from_le_bytes(
                encoded[8..12]
                    .try_into()
                    .expect("checked acceleration length"),
            ),
        )
    }

    pub fn semantic_digest(self) -> [u8; 32] {
        semantic_digest(ROBOTICS_ACCELERATION_INFO_ID, &self.encode())
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
    use crate::BODY_SECTOR_FRONT_LEFT;

    #[test]
    fn proximity_is_not_range_and_reserved_sectors_refuse() {
        let value = ProximityObservation::new(BODY_SECTOR_FRONT_LEFT).unwrap();
        assert_eq!(ProximityObservation::decode(&value.encode()), Ok(value));
        assert!(ProximityObservation::new(0x80).is_err());
        assert_ne!(ROBOTICS_PROXIMITY_INFO_ID, crate::ROBOTICS_RANGE_INFO_ID);
    }

    #[test]
    fn virtual_wall_never_fabricates_an_ir_code() {
        assert!(BeaconObservation::new(BeaconKind::VirtualWall, 4).is_err());
        let ir = BeaconObservation::new(BeaconKind::InfraredCode, 137).unwrap();
        assert_eq!(BeaconObservation::decode(&ir.encode()), Ok(ir));
    }

    #[test]
    fn buttons_and_body_frame_acceleration_are_exact_and_bounded() {
        let buttons = ButtonSetObservation::new(0x8000_0001);
        assert_eq!(ButtonSetObservation::decode(&buttons.encode()), Ok(buttons));
        let acceleration = AccelerationObservation::new(9_810, -20, 0).unwrap();
        assert_eq!(
            AccelerationObservation::decode(&acceleration.encode()),
            Ok(acceleration)
        );
        assert!(AccelerationObservation::new(MAXIMUM_ACCELERATION_MM_S2 + 1, 0, 0).is_err());
    }
}
