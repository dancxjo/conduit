#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum OscillatorShape {
    Sine,
    Triangle,
    Saw,
    Pulse,
}

/// Unsigned wraparound phase: one full cycle is `u32::MAX + 1`.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct Oscillator {
    phase: u32,
}

impl Oscillator {
    pub const fn phase(self) -> u32 {
        self.phase
    }

    pub fn sample(&mut self, shape: OscillatorShape, increment: u32, pulse_width: u16) -> i32 {
        let phase = self.phase;
        self.phase = self.phase.wrapping_add(increment);
        match shape {
            OscillatorShape::Saw => ((phase >> 16) as i32 - 32_768) << 15,
            OscillatorShape::Pulse => {
                if (phase >> 16) < u32::from(pulse_width) {
                    i32::MAX
                } else {
                    i32::MIN + 1
                }
            }
            OscillatorShape::Triangle => triangle(phase),
            OscillatorShape::Sine => sine_approximation(phase),
        }
    }
}

fn triangle(phase: u32) -> i32 {
    let quarter = phase >> 30;
    let within = ((phase >> 14) & 0xffff) as i32;
    let value = match quarter {
        0 => within,
        1 => 65_535 - within,
        2 => -within,
        _ => -65_535 + within,
    };
    value << 15
}

/// Integer parabolic sine approximation with exact wraparound and no target
/// floating-point dependency. Error is part of the frozen reference profile.
fn sine_approximation(phase: u32) -> i32 {
    let signed = (phase as i32) >> 1;
    let x = i64::from(signed);
    let absolute = x.unsigned_abs() as i64;
    let parabola = (x * (i64::from(i32::MAX) - absolute)) >> 29;
    clamp_i64_to_i32(parabola)
}

pub(crate) const fn clamp_i64_to_i32(value: i64) -> i32 {
    if value > i32::MAX as i64 {
        i32::MAX
    } else if value < i32::MIN as i64 {
        i32::MIN
    } else {
        value as i32
    }
}
