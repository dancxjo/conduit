use crate::oscillator::clamp_i64_to_i32;

/// Bounded integer state-variable low-pass filter.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct ResonantLowPass {
    low: i32,
    band: i32,
}

impl ResonantLowPass {
    pub fn process(&mut self, input: i32, cutoff_q16: u16, resonance_q16: u16) -> i32 {
        let frequency = i64::from(cutoff_q16.clamp(1, 32_768));
        let damping = i64::from(65_535u16.saturating_sub(resonance_q16).max(5_535));
        let low_delta = (frequency * i64::from(self.band)) >> 16;
        self.low = clamp_i64_to_i32(i64::from(self.low) + low_delta);
        let high =
            i64::from(input) - i64::from(self.low) - ((damping * i64::from(self.band)) >> 16);
        let band_delta = (frequency * high) >> 16;
        self.band = clamp_i64_to_i32(i64::from(self.band) + band_delta);
        self.low
    }

    pub const fn state(self) -> (i32, i32) {
        (self.low, self.band)
    }

    pub fn clear(&mut self) {
        self.low = 0;
        self.band = 0;
    }
}
