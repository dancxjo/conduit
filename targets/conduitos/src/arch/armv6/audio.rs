//! Sole boot-owned PWM PCM provider for the Model B+ analogue jack.
use super::{read_counter, read32, write32};
use crate::{
    machine::BaseError,
    pcm_output::{PcmOutput, PcmProvider},
};
use core::sync::atomic::{AtomicBool, Ordering};
const PWM: usize = 0x2020_c000;
const CLOCK: usize = 0x2010_10a0;
const PASSWORD: u32 = 0x5a00_0000;
const RANGE: u32 = 200;
const MAX_FRAMES: u32 = 96_000;
static OWNED: AtomicBool = AtomicBool::new(false);
pub struct PwmAudio {
    last_playback: Option<PlaybackObservation>,
}
#[derive(Clone, Copy)]
pub struct PlaybackObservation {
    pub frames_queued: u32,
    pub elapsed_us: u32,
    pub pwm_status: u32,
    pub completed: bool,
    pub failure: Option<BaseError>,
}

impl PwmAudio {
    pub fn prepare() -> Result<Self, BaseError> {
        if OWNED.swap(true, Ordering::AcqRel) {
            return Err(BaseError::Unavailable);
        }
        unsafe {
            write32(PWM, 0);
            write32(CLOCK, PASSWORD | 1);
        }
        wait_clock(false)?;
        // The B+ oscillator is 19.2 MHz; /2 and a 200-tick PWM range gives
        // exactly 48 kHz. No PLL assumption or DMA ownership is introduced.
        unsafe {
            write32(CLOCK + 4, PASSWORD | (2 << 12));
            write32(CLOCK, PASSWORD | 1 | (1 << 4));
        }
        wait_clock(true)?;
        unsafe {
            let fsel = super::GPIO_BASE + 0x10;
            let value = read32(fsel) & !((7 << 0) | (7 << 15));
            write32(fsel, value | (4 << 0) | (4 << 15)); // GPIO40 PWM0, GPIO45 PWM1 (ALT0).
            write32(PWM + 0x10, RANGE);
            write32(PWM + 0x20, RANGE);
            write32(PWM + 0x14, RANGE / 2);
            write32(PWM + 0x24, RANGE / 2);
            write32(PWM + 4, 0x1fc);
            write32(PWM, (1 << 0) | (1 << 8));
        }
        Ok(Self {
            last_playback: None,
        })
    }
    pub fn last_playback(&self) -> Option<PlaybackObservation> {
        self.last_playback
    }
}
fn wait_clock(busy: bool) -> Result<(), BaseError> {
    let start = read_counter() as u32;
    for _ in 0..1_000_000 {
        if (unsafe { read32(CLOCK) } & (1 << 7) != 0) == busy {
            return Ok(());
        }
        if (read_counter() as u32).wrapping_sub(start) > 100_000 {
            break;
        }
    }
    Err(BaseError::Unavailable)
}
impl PcmOutput for PwmAudio {
    fn provider(&self) -> PcmProvider {
        PcmProvider {
            base_id: "conduitos/bcm2835/pwm-jack/0",
            provider_generation: 1,
            sample_rate: 48_000,
            maximum_frames: MAX_FRAMES,
        }
    }
    fn play(&mut self, samples: &[i16]) -> Result<(), BaseError> {
        if samples.is_empty() || samples.len() > MAX_FRAMES as usize {
            return Err(BaseError::OutOfRange);
        }
        let start = read_counter() as u32;
        unsafe {
            write32(PWM, 1 << 6);
            write32(PWM + 4, 0x1fc);
        }
        let mut queued = 0usize;
        // Prefill both channels before enabling their shared FIFO consumption.
        while queued < samples.len().min(8) {
            let duty = ((i32::from(samples[queued]) + 32768) as u32 * RANGE) >> 16;
            unsafe {
                write32(PWM + 0x18, duty);
                write32(PWM + 0x18, duty);
            }
            queued += 1;
        }
        unsafe {
            write32(PWM, (1 << 0) | (1 << 5) | (1 << 8) | (1 << 13));
        }
        let result = (|| {
            for sample in &samples[queued..] {
                let duty = ((i32::from(*sample) + 32768) as u32 * RANGE) >> 16;
                for _ in 0..2 {
                    loop {
                        let status = unsafe { read32(PWM + 4) };
                        if status & (1 << 8) != 0 {
                            return Err(BaseError::Unavailable);
                        }
                        if (read_counter() as u32).wrapping_sub(start) > 2_500_000 {
                            return Err(BaseError::WorkPressure);
                        }
                        if status & 1 == 0 {
                            break;
                        }
                    }
                    unsafe {
                        write32(PWM + 0x18, duty);
                    }
                }
                queued += 1;
            }
            while unsafe { read32(PWM + 4) } & (1 << 1) == 0 {
                if (read_counter() as u32).wrapping_sub(start) > 2_500_000 {
                    return Err(BaseError::WorkPressure);
                }
            }
            let tail = read_counter() as u32;
            while (read_counter() as u32).wrapping_sub(tail) < 50 {
                core::hint::spin_loop();
            }
            Ok(())
        })();
        self.last_playback = Some(PlaybackObservation {
            frames_queued: queued as u32,
            elapsed_us: (read_counter() as u32).wrapping_sub(start),
            pwm_status: unsafe { read32(PWM + 4) },
            completed: result.is_ok(),
            failure: result.as_ref().err().copied(),
        });
        // Remain at silent midpoint rather than replaying the last sample.
        unsafe {
            write32(PWM, 0);
            write32(PWM + 0x14, RANGE / 2);
            write32(PWM + 0x24, RANGE / 2);
            write32(PWM, (1 << 0) | (1 << 8));
        }
        result
    }
}
