//! The scheduling guard encloses the actual provider future and its GPIO release.
//! It adds no pulse protocol, source decoder, ownership or capture budget.
use crate::native_flash_window::Gate;
use conduit_pulse_capture::{
    owner::PulseReadProvider, CheckedPulseRead, PulseReadCapture, PulseReadRefusal,
};
pub(crate) struct GatedPulse<P> {
    inner: P,
    gate: &'static Gate,
}
impl<P> GatedPulse<P> {
    pub(crate) fn new(inner: P, gate: &'static Gate) -> Self {
        Self { inner, gate }
    }
}
impl<P: PulseReadProvider> PulseReadProvider for GatedPulse<P> {
    async fn read(
        &mut self,
        checked: CheckedPulseRead,
    ) -> Result<PulseReadCapture, PulseReadRefusal> {
        let request = checked.request();
        let duration =
            (u64::from(request.hold_low_us) + u64::from(request.capture_timeout_us)).div_ceil(1000);
        let now = embassy_time::Instant::now().as_millis();
        let deadline = now
            .checked_add(duration)
            .ok_or(PulseReadRefusal::DeadlineExtent)?;
        let guard = self
            .gate
            .begin_pulse(now, deadline)
            .map_err(|_| PulseReadRefusal::DeadlineExtent)?;
        // The inner GPIO provider releases on drop, including cancellation.
        // Its future is destroyed before this outer guard on every exit path.
        let result = self.inner.read(checked).await;
        drop(guard);
        result
    }
}
