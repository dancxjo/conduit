//! Selected hardware mapping. A missing or shared annunciator is never guessed.
use super::{SegmentPatternForm, SevenSegmentShow};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WireBit {
    pub address: u8,
    pub bit: u8,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DigitWiring {
    pub address: u8,
    pub segment_bits: [u8; 7],
}
pub struct Tm1637Profile {
    pub digits: [DigitWiring; 4],
    pub decimal_points: [Option<WireBit>; 4],
    pub colon: Option<WireBit>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WiringRefusal {
    InvalidProfile,
    UnwiredAnnunciator,
    SharedControlConflict,
}

impl Tm1637Profile {
    pub fn validate(&self) -> Result<(), WiringRefusal> {
        let mut addresses = 0u8;
        let mut occupied = [0u8; 6];
        for digit in self.digits {
            if digit.address >= 6 || addresses & (1 << digit.address) != 0 {
                return Err(WiringRefusal::InvalidProfile);
            }
            addresses |= 1 << digit.address;
            for bit in digit.segment_bits {
                if bit >= 8 || occupied[digit.address as usize] & (1 << bit) != 0 {
                    return Err(WiringRefusal::InvalidProfile);
                }
                occupied[digit.address as usize] |= 1 << bit;
            }
        }
        for wire in self
            .decimal_points
            .into_iter()
            .chain([self.colon])
            .flatten()
        {
            if wire.address >= 6
                || wire.bit >= 8
                || occupied[wire.address as usize] & (1 << wire.bit) != 0
            {
                return Err(WiringRefusal::InvalidProfile);
            }
        }
        Ok(())
    }
    pub fn map(&self, show: &SevenSegmentShow) -> Result<[u8; 6], WiringRefusal> {
        self.validate()?;
        let mut bytes = [0u8; 6];
        for (position, digit) in self.digits.iter().enumerate() {
            let pattern = SegmentPatternForm::encode(show.digits()[position])[0];
            for (semantic_bit, physical_bit) in digit.segment_bits.iter().enumerate() {
                if pattern & (1 << semantic_bit) != 0 {
                    bytes[digit.address as usize] |= 1 << physical_bit;
                }
            }
        }
        let mut known = [0u8; 6];
        let requested = show.decimal_points().iter().copied().chain([*show.colon()]);
        let wires = self.decimal_points.into_iter().chain([self.colon]);
        for (wire, on) in wires.zip(requested) {
            let Some(wire) = wire else {
                if on {
                    return Err(WiringRefusal::UnwiredAnnunciator);
                }
                continue;
            };
            let address = wire.address as usize;
            let bit = 1 << wire.bit;
            if known[address] & bit != 0 && (bytes[address] & bit != 0) != on {
                return Err(WiringRefusal::SharedControlConflict);
            }
            known[address] |= bit;
            if on {
                bytes[address] |= bit;
            }
        }
        Ok(bytes)
    }
}
