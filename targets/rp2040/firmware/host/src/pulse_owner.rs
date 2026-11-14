//! Exact electrical capture custody; possession alone grants no Play authority.
use core::{
    cell::RefCell,
    ops::{Deref, DerefMut},
};
use critical_section::Mutex;
use static_cell::StaticCell;
pub type Capture =
    conduit_rp2040_pulse_capture::PicoPulseCapture<'static, embassy_rp::peripherals::PIO1, 0>;
static OWNER: StaticCell<Capture> = StaticCell::new();
static SLOT: Mutex<RefCell<Option<&'static mut Capture>>> = Mutex::new(RefCell::new(None));
pub fn initialize(capture: Capture) {
    let capture = OWNER.init(capture);
    critical_section::with(|cs| *SLOT.borrow(cs).borrow_mut() = Some(capture));
}
pub struct CaptureLease(Option<&'static mut Capture>);
impl Deref for CaptureLease {
    type Target = Capture;
    fn deref(&self) -> &Capture {
        self.0.as_deref().expect("live capture lease")
    }
}
impl DerefMut for CaptureLease {
    fn deref_mut(&mut self) -> &mut Capture {
        self.0.as_deref_mut().expect("live capture lease")
    }
}
impl Drop for CaptureLease {
    fn drop(&mut self) {
        if let Some(capture) = self.0.take() {
            critical_section::with(|cs| {
                let mut slot = SLOT.borrow(cs).borrow_mut();
                debug_assert!(slot.is_none());
                *slot = Some(capture);
            });
        }
    }
}
pub fn try_lease() -> Option<CaptureLease> {
    critical_section::with(|cs| {
        SLOT.borrow(cs)
            .borrow_mut()
            .take()
            .map(|capture| CaptureLease(Some(capture)))
    })
}
impl conduit_pulse_capture::owner::PulseReadProvider for CaptureLease {
    async fn read(
        &mut self,
        request: conduit_pulse_capture::CheckedPulseRead,
    ) -> Result<conduit_pulse_capture::PulseReadCapture, conduit_pulse_capture::PulseReadRefusal>
    {
        let geometry = request.request();
        if geometry.hold_low_us != crate::config::DHT_START_LOW_US {
            return Err(conduit_pulse_capture::PulseReadRefusal::DriveExtent);
        }
        if geometry
            .hold_low_us
            .saturating_add(geometry.capture_timeout_us)
            > crate::config::DHT_TRANSACTION_US
        {
            return Err(conduit_pulse_capture::PulseReadRefusal::DeadlineExtent);
        }
        self.deref_mut().read(request).await
    }
}
