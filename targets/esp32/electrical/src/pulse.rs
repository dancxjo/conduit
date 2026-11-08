use conduit_pulse_capture::{
    CheckedPulseRead, PulseReadCapture, PulseReadDisposition, PulseReadLimits, PulseReadRefusal,
};
use core::cell::Cell;
use esp_hal::{
    delay::Delay,
    gpio::{Flex, InputConfig, Pin, Pull},
    time::Instant,
};

/// Recovery evidence from the last actual capture, never an IO command or grant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum CapturePhase {
    ReleaseRise = 1,
    ResponseLow,
    SkippedRise,
    SkippedFall,
    DataRise,
    Sample,
    DataFall,
    Complete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaptureObservation {
    pub phase: CapturePhase,
    pub completed_bits: u8,
    pub completed_octets: u8,
    /// Timed capture interval, saturating at u32::MAX microseconds.
    pub elapsed_us: u32,
    /// Completed octets only; remaining slots stay zero. Protocol interpretation
    /// belongs to Source. Published after capture, never inside the timed loop.
    pub wire: [u8; conduit_pulse_capture::MAXIMUM_OCTETS],
}

// Publish/read the complete diagnostic under a short exclusion after capture.
// No shared writes, atomics, formatting or diagnostic locks enter the GPIO loop.
static LAST_CAPTURE: critical_section::Mutex<Cell<Option<CaptureObservation>>> =
    critical_section::Mutex::new(Cell::new(None));

pub fn last_capture_observation() -> Option<CaptureObservation> {
    critical_section::with(|cs| LAST_CAPTURE.borrow(cs).get())
}

impl CaptureObservation {
    fn publish(self) {
        critical_section::with(|cs| LAST_CAPTURE.borrow(cs).set(Some(self)));
    }
}

pub const PULSE_LIMITS: PulseReadLimits = PulseReadLimits {
    maximum_hold_low_us: 50_000,
    maximum_skipped_pulses: 4,
    minimum_sample_offset_us: 10,
    maximum_sample_offset_us: 200,
    maximum_octets: 8,
    maximum_capture_timeout_us: 50_000,
};

/// Finite synchronous GPIO realization. It allocates nothing and never drives
/// the line high. Construction bounds the non-yielding transaction and the
/// interrupt exclusion during capture. Protocol/checksum validation stays in Source.
pub struct Esp32PulseCapture<'d> {
    pin: Flex<'d>,
    delay: Delay,
    maximum_transaction_us: u32,
    maximum_interrupt_exclusion_us: u32,
}

impl<'d> Esp32PulseCapture<'d> {
    /// Consume actual pin ownership; weak pull-up selection is attachment truth.
    pub fn new(
        pin: impl Pin + 'd,
        weak_pullup: bool,
        maximum_transaction_us: u32,
        maximum_interrupt_exclusion_us: u32,
    ) -> Result<Self, PulseReadRefusal> {
        if maximum_interrupt_exclusion_us == 0
            || maximum_interrupt_exclusion_us > 20_000
            || maximum_interrupt_exclusion_us > maximum_transaction_us
        {
            return Err(PulseReadRefusal::InvalidLimits);
        }
        let mut pin = Flex::new(pin);
        pin.set_output_enable(false);
        pin.set_low();
        pin.set_input_enable(true);
        pin.apply_input_config(&InputConfig::default().with_pull(if weak_pullup {
            Pull::Up
        } else {
            Pull::None
        }));
        Ok(Self {
            pin,
            delay: Delay::new(),
            maximum_transaction_us,
            maximum_interrupt_exclusion_us,
        })
    }

    pub fn capture(
        &mut self,
        checked: CheckedPulseRead,
    ) -> Result<PulseReadCapture, PulseReadRefusal> {
        // Recheck the actual provider envelope before electrical effects.
        let request = CheckedPulseRead::admit(checked.request(), PULSE_LIMITS)?.request();
        if request.capture_timeout_us > self.maximum_interrupt_exclusion_us
            || request
                .hold_low_us
                .checked_add(request.capture_timeout_us)
                .is_none_or(|span| span > self.maximum_transaction_us)
        {
            return Err(PulseReadRefusal::DeadlineExtent);
        }
        // Calibrate from the actual initialized CPU clock before effects. The
        // finite intervals fit below half the wrapping CCOUNT range.
        let hz = esp_hal::clock::cpu_clock().as_hz();
        let capture_cycles = cycles_for_micros(hz, request.capture_timeout_us)
            .ok_or(PulseReadRefusal::DeadlineExtent)?;
        let sample_cycles = cycles_for_micros(hz, u32::from(request.sample_after_rise_us))
            .ok_or(PulseReadRefusal::SampleOffset)?;
        self.pin.set_output_enable(true);
        self.delay.delay_micros(request.hold_low_us);
        let mut wire = [0; conduit_pulse_capture::MAXIMUM_OCTETS];
        let mut length = 0;
        let mut observation = CaptureObservation {
            phase: CapturePhase::ReleaseRise,
            completed_bits: 0,
            completed_octets: 0,
            elapsed_us: 0,
            wire: [0; conduit_pulse_capture::MAXIMUM_OCTETS],
        };
        // The checked hold runs with interrupts enabled. The separate exclusion
        // budget covers the capture deadline plus bounded polling/exit overhead.
        let (complete, elapsed_us) = critical_section::with(|_| {
            let start = Instant::now();
            let deadline = CycleDeadline {
                start: xtensa_lx::timer::get_cycle_count(),
                budget: capture_cycles,
            };
            self.pin.set_output_enable(false);
            let complete = self.read_bits(
                request,
                deadline,
                sample_cycles,
                &mut wire,
                &mut length,
                &mut observation,
            );
            (complete, start.elapsed().as_micros())
        });
        observation.elapsed_us = elapsed_us.min(u64::from(u32::MAX)) as u32;
        observation.wire = wire;
        observation.publish();
        if elapsed_us > u64::from(self.maximum_interrupt_exclusion_us) {
            return Err(PulseReadRefusal::DeadlineExtent);
        }
        // Every normal return, including timeout, leaves the line released.
        self.pin.set_output_enable(false);
        Ok(PulseReadCapture::from_complete_octets(
            if complete {
                PulseReadDisposition::Complete
            } else {
                PulseReadDisposition::TimedOut
            },
            &wire[..length],
        )
        .expect("checked finite capture extent"))
    }

    fn wait_level(&self, high: bool, deadline: CycleDeadline) -> Option<u32> {
        loop {
            match sample_before_deadline(
                || self.pin.is_high(),
                xtensa_lx::timer::get_cycle_count,
                deadline,
            ) {
                Some((level, observed_at)) if level == high => return Some(observed_at),
                None => return None,
                Some(_) => core::hint::spin_loop(),
            }
        }
    }

    fn read_bits(
        &self,
        request: conduit_pulse_capture::PulseReadRequest,
        deadline: CycleDeadline,
        sample_cycles: u32,
        wire: &mut [u8; conduit_pulse_capture::MAXIMUM_OCTETS],
        length: &mut usize,
        observation: &mut CaptureObservation,
    ) -> bool {
        observation.phase = CapturePhase::ReleaseRise;
        if self.wait_level(true, deadline).is_none() {
            return false;
        }
        observation.phase = CapturePhase::ResponseLow;
        if self.wait_level(false, deadline).is_none() {
            return false;
        }
        for _ in 0..request.skip_high_pulses {
            observation.phase = CapturePhase::SkippedRise;
            if self.wait_level(true, deadline).is_none() {
                return false;
            }
            observation.phase = CapturePhase::SkippedFall;
            if self.wait_level(false, deadline).is_none() {
                return false;
            }
        }
        for octet in wire.iter_mut().take(usize::from(request.octets)) {
            let mut value = 0;
            for _ in 0..8 {
                observation.phase = CapturePhase::DataRise;
                let Some(rise) = self.wait_level(true, deadline) else {
                    return false;
                };
                observation.phase = CapturePhase::Sample;
                // Anchor the offset to the observed rise. No slow LACT timer
                // reads or HAL Delay setup occur between capture edges.
                loop {
                    let now = xtensa_lx::timer::get_cycle_count();
                    if deadline.expired(now) {
                        return false;
                    }
                    if now.wrapping_sub(rise) >= sample_cycles {
                        break;
                    }
                    core::hint::spin_loop();
                }
                let Some((high, _)) = sample_before_deadline(
                    || self.pin.is_high(),
                    xtensa_lx::timer::get_cycle_count,
                    deadline,
                ) else {
                    return false;
                };
                value = (value << 1) | u8::from(high);
                observation.completed_bits += 1;
                // The request asks for samples, not a trailing edge after its
                // final bit. Retain that complete octet before returning.
                if observation.completed_bits == request.octets * 8 {
                    *octet = value;
                    *length += 1;
                    observation.completed_octets = *length as u8;
                    observation.phase = CapturePhase::Complete;
                    return true;
                }
                observation.phase = CapturePhase::DataFall;
                if self.wait_level(false, deadline).is_none() {
                    return false;
                }
            }
            *octet = value;
            *length += 1;
            observation.completed_octets = *length as u8;
        }
        observation.phase = CapturePhase::Complete;
        true
    }
}

#[cfg(feature = "host-call")]
impl conduit_pulse_capture::owner::PulseReadProvider for Esp32PulseCapture<'_> {
    async fn read(
        &mut self,
        request: CheckedPulseRead,
    ) -> Result<PulseReadCapture, PulseReadRefusal> {
        // No suspension after touching hardware: cancellation before polling has
        // no effects; completion releases the pin before returning to the owner.
        self.capture(request)
    }
}

impl Drop for Esp32PulseCapture<'_> {
    fn drop(&mut self) {
        self.pin.set_output_enable(false);
    }
}

// Pure calibration and wrapping-deadline law shared by the timed realization.
fn cycles_for_micros(hz: u32, micros: u32) -> Option<u32> {
    let cycles = u64::from(hz)
        .checked_mul(u64::from(micros))?
        .checked_add(999_999)?
        / 1_000_000;
    (hz != 0 && cycles != 0 && cycles < (1u64 << 31)).then_some(cycles as u32)
}
#[derive(Clone, Copy)]
struct CycleDeadline {
    start: u32,
    budget: u32,
}
impl CycleDeadline {
    #[inline(always)]
    fn expired(self, now: u32) -> bool {
        now.wrapping_sub(self.start) >= self.budget
    }
}
#[inline(always)]
fn sample_before_deadline(
    sample: impl FnOnce() -> bool,
    clock: impl FnOnce() -> u32,
    deadline: CycleDeadline,
) -> Option<(bool, u32)> {
    let level = sample();
    let now = clock();
    if deadline.expired(now) {
        None
    } else {
        Some((level, now))
    }
}

#[cfg(test)]
mod timing_tests {
    use super::{cycles_for_micros, sample_before_deadline, CycleDeadline};
    use core::cell::Cell;

    #[test]
    fn calibration_uses_the_actual_rate_and_refuses_ambiguous_wrap_intervals() {
        assert_eq!(cycles_for_micros(240_000_000, 50_000), Some(12_000_000));
        assert_eq!(cycles_for_micros(80_000_000, 40), Some(3200));
        assert_eq!(cycles_for_micros(1_500_001, 1), Some(2));
        assert_eq!(cycles_for_micros(0, 40), None);
        assert_eq!(cycles_for_micros(240_000_000, 0), None);
        assert_eq!(cycles_for_micros(1_000_000, 1 << 31), None);
        assert_eq!(cycles_for_micros(u32::MAX, u32::MAX), None);
    }
    #[test]
    fn deadline_and_sample_offset_remain_exact_across_counter_wrap() {
        let deadline = CycleDeadline {
            start: u32::MAX - 9,
            budget: 40,
        };
        assert!(!deadline.expired(u32::MAX));
        assert!(!deadline.expired(29));
        assert!(deadline.expired(30));
        let rise = u32::MAX - 4;
        assert_eq!(4u32.wrapping_sub(rise), 9);
        assert_eq!(5u32.wrapping_sub(rise), 10);
        assert_eq!(
            sample_before_deadline(|| true, || 29, deadline),
            Some((true, 29))
        );
        assert_eq!(sample_before_deadline(|| true, || 30, deadline), None);
    }
    #[test]
    fn fast_poll_catches_release_that_rises_between_iterations() {
        let tick = Cell::new(0u32);
        // Initially low: unlike just reordering one slow timer read, fast polling
        // must see a complete short high which begins AFTER the first GPIO read.
        let level = || matches!(tick.get(), 2..7 | 90..170 | 220..290);
        let clock = || {
            tick.set(tick.get() + 1);
            tick.get()
        };
        let deadline = CycleDeadline {
            start: 0,
            budget: 1000,
        };
        let wait = |wanted| loop {
            match sample_before_deadline(level, clock, deadline) {
                Some((actual, at)) if actual == wanted => return at,
                Some(_) => (),
                None => panic!("finite fixture deadline"),
            }
        };
        assert_eq!(wait(true), 3);
        assert_eq!(wait(false), 8);
        assert_eq!(wait(true), 91);
        assert_eq!(wait(false), 171);
        assert_eq!(wait(true), 221);
        tick.set(1000);
        assert_eq!(sample_before_deadline(|| true, clock, deadline), None);
    }
}
