//! Legacy PC-speaker Base over PIT channel 2 and the system-control gate.

use conduit_audio::{Gate, ToneIntent};

use crate::machine::{BaseError, RealizedTone, ToneBase};

use super::io::{inb, outb};

pub const PIT_INPUT_HZ: u64 = 1_193_182;
pub const MINIMUM_DIVISOR: u16 = 19;
pub const MAXIMUM_DIVISOR: u16 = u16::MAX;
pub const MAXIMUM_ERROR_PARTS_PER_MILLION: u32 = 2_500;

const PIT_CONTROL: u16 = 0x43;
const PIT_CHANNEL_TWO: u16 = 0x42;
const SYSTEM_CONTROL_B: u16 = 0x61;
const CHANNEL_TWO_SQUARE_WAVE: u8 = 0xb6;
const SPEAKER_GATE_BITS: u8 = 0x03;

pub struct PcSpeaker {
    active: bool,
    transitions: u32,
}

impl PcSpeaker {
    pub const fn new() -> Self {
        Self {
            active: false,
            transitions: 0,
        }
    }

    pub const fn active(&self) -> bool {
        self.active
    }
}

impl Default for PcSpeaker {
    fn default() -> Self {
        Self::new()
    }
}

impl ToneBase for PcSpeaker {
    fn apply(&mut self, intent: ToneIntent) -> Result<RealizedTone, BaseError> {
        match intent.gate {
            Gate::Off => {
                self.silence()?;
                Ok(RealizedTone {
                    correlation: intent.correlation,
                    requested_millihertz: intent.pitch.frequency_millihertz,
                    realized_millihertz: 0,
                    divisor: 0,
                    gate_open: false,
                })
            }
            Gate::On => {
                let (divisor, realized_millihertz) = quantize(intent.pitch.frequency_millihertz)?;
                unsafe {
                    // Channel 2 only: channel 0 remains the kernel timer Base.
                    outb(PIT_CONTROL, CHANNEL_TWO_SQUARE_WAVE);
                    outb(PIT_CHANNEL_TWO, divisor as u8);
                    outb(PIT_CHANNEL_TWO, (divisor >> 8) as u8);
                    let gate = inb(SYSTEM_CONTROL_B);
                    outb(SYSTEM_CONTROL_B, gate | SPEAKER_GATE_BITS);
                }
                self.active = true;
                self.transitions = self
                    .transitions
                    .checked_add(1)
                    .ok_or(BaseError::Unavailable)?;
                Ok(RealizedTone {
                    correlation: intent.correlation,
                    requested_millihertz: intent.pitch.frequency_millihertz,
                    realized_millihertz,
                    divisor,
                    gate_open: true,
                })
            }
        }
    }

    fn silence(&mut self) -> Result<(), BaseError> {
        unsafe {
            let gate = inb(SYSTEM_CONTROL_B);
            outb(SYSTEM_CONTROL_B, gate & !SPEAKER_GATE_BITS);
        }
        self.active = false;
        self.transitions = self
            .transitions
            .checked_add(1)
            .ok_or(BaseError::Unavailable)?;
        Ok(())
    }

    fn transition_count(&self) -> u32 {
        self.transitions
    }
}

pub fn quantize(requested_millihertz: u64) -> Result<(u16, u64), BaseError> {
    if requested_millihertz == 0 {
        return Err(BaseError::UnsupportedValue);
    }
    let numerator = PIT_INPUT_HZ
        .checked_mul(1_000)
        .ok_or(BaseError::UnsupportedValue)?;
    let rounded = numerator
        .checked_add(requested_millihertz / 2)
        .ok_or(BaseError::UnsupportedValue)?
        / requested_millihertz;
    let divisor = u16::try_from(rounded).map_err(|_| BaseError::UnsupportedValue)?;
    if !(MINIMUM_DIVISOR..=MAXIMUM_DIVISOR).contains(&divisor) {
        return Err(BaseError::UnsupportedValue);
    }
    let realized = numerator / u64::from(divisor);
    let error = realized.abs_diff(requested_millihertz);
    let error_ppm = error
        .checked_mul(1_000_000)
        .ok_or(BaseError::UnsupportedValue)?
        / requested_millihertz;
    if error_ppm > u64::from(MAXIMUM_ERROR_PARTS_PER_MILLION) {
        return Err(BaseError::UnsupportedValue);
    }
    Ok((divisor, realized))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_divisor_profile_is_bounded_and_refuses_unrepresentable_pitch() {
        assert_eq!(quantize(440_000), Ok((2_712, 439_963)));
        assert_eq!(quantize(660_000), Ok((1_808, 659_945)));
        assert_eq!(quantize(8_000), Err(BaseError::UnsupportedValue));
        assert_eq!(quantize(40_000_000), Err(BaseError::UnsupportedValue));
    }
}
