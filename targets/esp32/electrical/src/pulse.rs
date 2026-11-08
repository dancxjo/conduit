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
            self.pin.set_output_enable(false);
            let complete = self.read_bits(request, start, &mut wire, &mut length, &mut observation);
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

    fn wait_level(&self, high: bool, start: Instant, timeout_us: u32) -> bool {
        loop {
            // Observe short levels before the comparatively slow timer read.
            // The following deadline check still gates acceptance of that sample.
            match sample_before_deadline(
                || self.pin.is_high(),
                || start.elapsed().as_micros() >= u64::from(timeout_us),
            ) {
                Some(level) if level == high => return true,
                None => return false,
                Some(_) => core::hint::spin_loop(),
            }
        }
    }

    fn read_bits(
        &self,
        request: conduit_pulse_capture::PulseReadRequest,
        start: Instant,
        wire: &mut [u8; conduit_pulse_capture::MAXIMUM_OCTETS],
        length: &mut usize,
        observation: &mut CaptureObservation,
    ) -> bool {
        observation.phase = CapturePhase::ReleaseRise;
        if !self.wait_level(true, start, request.capture_timeout_us) {
            return false;
        }
        observation.phase = CapturePhase::ResponseLow;
        if !self.wait_level(false, start, request.capture_timeout_us) {
            return false;
        }
        for _ in 0..request.skip_high_pulses {
            observation.phase = CapturePhase::SkippedRise;
            if !self.wait_level(true, start, request.capture_timeout_us) {
                return false;
            }
            observation.phase = CapturePhase::SkippedFall;
            if !self.wait_level(false, start, request.capture_timeout_us) {
                return false;
            }
        }
        for octet in wire.iter_mut().take(usize::from(request.octets)) {
            let mut value = 0;
            for _ in 0..8 {
                observation.phase = CapturePhase::DataRise;
                if !self.wait_level(true, start, request.capture_timeout_us) {
                    return false;
                }
                observation.phase = CapturePhase::Sample;
                self.delay
                    .delay_micros(u32::from(request.sample_after_rise_us));
                // Sample at the requested offset before reading the timer, but
                // never consume the sampled bit if the capture deadline expired.
                let Some(high) = sample_before_deadline(
                    || self.pin.is_high(),
                    || start.elapsed().as_micros() >= u64::from(request.capture_timeout_us),
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
                if !self.wait_level(false, start, request.capture_timeout_us) {
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

// Keep the GPIO-before-timer order identical for edge polling and bit sampling.
// Inlined closures retain direct HAL accesses in the timed realization.
#[inline(always)]
fn sample_before_deadline(
    sample: impl FnOnce() -> bool,
    deadline_reached: impl FnOnce() -> bool,
) -> Option<bool> {
    let level = sample();
    if deadline_reached() {
        None
    } else {
        Some(level)
    }
}

#[cfg(test)]
mod timing_tests {
    use super::sample_before_deadline;
    use core::cell::Cell;

    #[test]
    fn short_release_high_is_observed_before_slow_timer_poll() {
        let time = Cell::new(0u32);
        // Release high ends before one timer read. A timer-first realization
        // mistakes the later response high for the release high.
        let level = || matches!(time.get(), 0..10 | 90..170 | 220..290 | 340..410);
        let expired = || {
            time.set(time.get() + 20);
            time.get() >= 1000
        };
        let wait = |wanted| loop {
            match sample_before_deadline(level, expired) {
                Some(actual) if actual == wanted => return time.get(),
                Some(_) => (),
                None => panic!("finite fixture deadline"),
            }
        };
        assert_eq!(wait(true), 20); // release high, sampled at zero
        assert_eq!(wait(false), 40); // response low
        assert_eq!(wait(true), 120); // skipped response high
        assert_eq!(wait(false), 200); // response fall/data low
        assert_eq!(wait(true), 240); // first data bit, not the second
    }

    #[test]
    fn bit_sample_precedes_timer_but_expiration_still_refuses_it() {
        let time = Cell::new(40u32);
        let sampled = sample_before_deadline(
            || time.get() < 70,
            || {
                time.set(80);
                time.get() >= 1000
            },
        );
        assert_eq!(sampled, Some(true));
        assert_eq!(time.get(), 80); // A timer-first GPIO read would have been low.
        assert_eq!(sample_before_deadline(|| true, || true), None);
        assert_eq!(sample_before_deadline(|| false, || true), None);
    }
}
