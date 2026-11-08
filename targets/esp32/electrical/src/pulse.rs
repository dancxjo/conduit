use conduit_pulse_capture::{
    CheckedPulseRead, PulseReadCapture, PulseReadDisposition, PulseReadLimits, PulseReadRefusal,
};
use esp_hal::{
    delay::Delay,
    gpio::{Flex, InputConfig, Pin, Pull},
    time::Instant,
};

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
        // The checked hold runs with interrupts enabled. The separate exclusion
        // budget covers the capture deadline plus bounded polling/exit overhead.
        let (complete, elapsed_us) = critical_section::with(|_| {
            let start = Instant::now();
            self.pin.set_output_enable(false);
            let complete = self.read_bits(request, start, &mut wire, &mut length);
            (complete, start.elapsed().as_micros())
        });
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
    ) -> bool {
        if !self.wait_level(true, start, request.capture_timeout_us)
            || !self.wait_level(false, start, request.capture_timeout_us)
        {
            return false;
        }
        for _ in 0..request.skip_high_pulses {
            if !self.wait_level(true, start, request.capture_timeout_us)
                || !self.wait_level(false, start, request.capture_timeout_us)
            {
                return false;
            }
        }
        for octet in wire.iter_mut().take(usize::from(request.octets)) {
            let mut value = 0;
            for _ in 0..8 {
                if !self.wait_level(true, start, request.capture_timeout_us) {
                    return false;
                }
                self.delay
                    .delay_micros(u32::from(request.sample_after_rise_us));
                if start.elapsed().as_micros() >= u64::from(request.capture_timeout_us) {
                    return false;
                }
                value = (value << 1) | u8::from(self.pin.is_high());
                if !self.wait_level(false, start, request.capture_timeout_us) {
                    return false;
                }
            }
            *octet = value;
            *length += 1;
        }
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
