#![no_std]
//! Finite cooperative GPIO capture. The selected Root must admit the hold time,
//! capture work and interrupt exclusion before offering this implementation.
//! Peripheral recognition and decoding stay in Source.
use conduit_pulse_capture::{
    CheckedPulseRead, PulseReadCapture, PulseReadDisposition as D, PulseReadLimits,
    PulseReadRefusal,
};
use esp_hal::{
    delay::Delay,
    gpio::{Flex, InputConfig, Pin, Pull},
    time::Instant,
};

pub const LIMITS: PulseReadLimits = PulseReadLimits {
    maximum_hold_low_us: 50_000,
    maximum_skipped_pulses: 4,
    minimum_sample_offset_us: 10,
    maximum_sample_offset_us: 200,
    maximum_octets: 8,
    maximum_capture_timeout_us: 10_000,
};
pub struct Esp32PulseCapture<'d> {
    pin: Flex<'d>,
    delay: Delay,
}
impl<'d> Esp32PulseCapture<'d> {
    /// Consume actual pin ownership. Construction is not a Host offer or grant.
    pub fn new(pin: impl Pin + 'd) -> Self {
        let mut pin = Flex::new(pin);
        pin.set_output_enable(false);
        pin.set_low();
        pin.set_input_enable(true);
        pin.apply_input_config(&InputConfig::default().with_pull(Pull::Up));
        Self {
            pin,
            delay: Delay::new(),
        }
    }
    /// One bounded poll, with no allocation. The capture phase excludes
    /// interrupts for at most the selected timeout (maximum 10 ms); hold-low
    /// runs with the caller's interrupt state. No async wait is hidden here.
    pub async fn read(
        &mut self,
        checked: CheckedPulseRead,
    ) -> Result<PulseReadCapture, PulseReadRefusal> {
        let request = CheckedPulseRead::admit(checked.request(), LIMITS)?.request();
        let pin = ReleaseOnDrop(&mut self.pin);
        pin.0.set_output_enable(true);
        self.delay.delay_micros(request.hold_low_us);
        let capture = critical_section::with(|_| capture(pin.0, request));
        drop(pin);
        Ok(capture)
    }
}
// The release handshake must not depend on a flash-cache miss. Keep the
// finite timing loop resident and prepare its deadline before releasing low.
#[esp_hal::ram]
fn capture(
    pin: &mut Flex<'_>,
    request: conduit_pulse_capture::PulseReadRequest,
) -> PulseReadCapture {
    let start = micros();
    let deadline = start.saturating_add(u64::from(request.capture_timeout_us));
    pin.set_output_enable(false);
    let release_high = pin.is_high();
    let mut result = PulseReadCapture {
        disposition: D::TimedOut,
        length: 0,
        wire: [0; 8],
    };
    // Observe release-high before clock bookkeeping can consume its
    // short interval. Preserve high-then-low synchronization.
    if (!release_high && !wait(pin, true, deadline)) || !wait(pin, false, deadline) {
        return result;
    }
    for _ in 0..request.skip_high_pulses {
        if !wait(pin, true, deadline) || !wait(pin, false, deadline) {
            return result;
        }
    }
    let mut octet = 0;
    let total = usize::from(request.octets) * 8;
    for sample in 0..total {
        if !wait(pin, true, deadline) {
            return result;
        }
        let at = micros().saturating_add(u64::from(request.sample_after_rise_us));
        while micros() < at {
            if micros() >= deadline {
                return result;
            }
        }
        if micros() >= deadline {
            return result;
        }
        octet = (octet << 1) | u8::from(pin.is_high());
        if sample % 8 == 7 {
            result.wire[usize::from(result.length)] = octet;
            result.length += 1;
            octet = 0;
        }
        if sample + 1 == total {
            result.disposition = D::Complete;
            return result;
        }
        if !wait(pin, false, deadline) {
            return result;
        }
    }
    result
}
#[inline(always)]
fn micros() -> u64 {
    Instant::now().duration_since_epoch().as_micros()
}
#[inline(always)]
fn wait(pin: &Flex<'_>, high: bool, deadline: u64) -> bool {
    while micros() < deadline {
        if pin.is_high() == high {
            return true;
        }
    }
    false
}
struct ReleaseOnDrop<'a, 'd>(&'a mut Flex<'d>);
impl Drop for ReleaseOnDrop<'_, '_> {
    fn drop(&mut self) {
        self.0.set_output_enable(false);
    }
}
impl Drop for Esp32PulseCapture<'_> {
    fn drop(&mut self) {
        self.pin.set_output_enable(false);
    }
}
#[cfg(feature = "host-call")]
impl conduit_pulse_capture::owner::PulseReadProvider for Esp32PulseCapture<'_> {
    async fn read(
        &mut self,
        request: CheckedPulseRead,
    ) -> Result<PulseReadCapture, PulseReadRefusal> {
        self.read(request).await
    }
}
