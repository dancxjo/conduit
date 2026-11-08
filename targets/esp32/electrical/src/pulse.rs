use conduit_pulse_capture::{
    CheckedPulseRead, PulseReadCapture, PulseReadDisposition, PulseReadLimits, PulseReadRefusal,
};
use core::sync::atomic::{AtomicU32, Ordering};
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
    /// Saturates at 131071us; that already exceeds every admitted exclusion.
    pub elapsed_us: u32,
}

// One coherent word; no shared writes or atomics occur in the timed GPIO loop.
static LAST_CAPTURE: AtomicU32 = AtomicU32::new(0);

pub fn last_capture_observation() -> Option<CaptureObservation> {
    let snapshot = LAST_CAPTURE.load(Ordering::Acquire);
    let phase = match snapshot & 15 {
        1 => CapturePhase::ReleaseRise,
        2 => CapturePhase::ResponseLow,
        3 => CapturePhase::SkippedRise,
        4 => CapturePhase::SkippedFall,
        5 => CapturePhase::DataRise,
        6 => CapturePhase::Sample,
        7 => CapturePhase::DataFall,
        8 => CapturePhase::Complete,
        _ => return None,
    };
    Some(CaptureObservation {
        phase,
        completed_bits: ((snapshot >> 4) & 127) as u8,
        completed_octets: ((snapshot >> 11) & 15) as u8,
        elapsed_us: snapshot >> 15,
    })
}

impl CaptureObservation {
    fn publish(self) {
        let snapshot = self.phase as u32
            | (u32::from(self.completed_bits) << 4)
            | (u32::from(self.completed_octets) << 11)
            | (self.elapsed_us.min(131071) << 15);
        LAST_CAPTURE.store(snapshot, Ordering::Release);
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
            if start.elapsed().as_micros() >= u64::from(timeout_us) {
                return false;
            }
            if self.pin.is_high() == high {
                return true;
            }
            core::hint::spin_loop();
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
                if start.elapsed().as_micros() >= u64::from(request.capture_timeout_us) {
                    return false;
                }
                value = (value << 1) | u8::from(self.pin.is_high());
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
