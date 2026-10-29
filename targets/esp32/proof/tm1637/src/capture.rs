//! Attended electrical test vector matching dht11-capture-request. Production
//! must obtain geometry from checked Source and invoke its admitted Host Call.
use conduit_esp32_pulse_capture::Esp32PulseCapture;
#[cfg(not(feature = "live"))]
use conduit_esp32_pulse_capture::LIMITS;
#[cfg(not(feature = "live"))]
use conduit_pulse_capture::PulseReadRequest;
use conduit_pulse_capture::{CheckedPulseRead, PulseReadCapture, PulseReadRefusal};
use core::{
    future::Future,
    task::{Context, Poll, Waker},
};
#[cfg(not(feature = "live"))]
pub fn probe(sensor: &mut Esp32PulseCapture<'_>) {
    let request = CheckedPulseRead::admit(
        PulseReadRequest {
            hold_low_us: 20_000,
            skip_high_pulses: 1,
            sample_after_rise_us: 40,
            octets: 5,
            capture_timeout_us: 10_000,
        },
        LIMITS,
    )
    .unwrap();
    match read(sensor,request) {
        Ok(capture) => esp_println::println!("{{\"event\":\"pulse-capture\",\"fixture\":true,\"gpio\":27,\"disposition\":\"{:?}\",\"length\":{},\"wire\":{:?}}}",capture.disposition,capture.length,capture.wire),
        Err(error) => esp_println::println!("{{\"event\":\"pulse-refusal\",\"error\":\"{:?}\"}}",error),
    }
}
pub fn read(
    sensor: &mut Esp32PulseCapture<'_>,
    request: CheckedPulseRead,
) -> Result<PulseReadCapture, PulseReadRefusal> {
    let mut future = core::pin::pin!(sensor.read(request));
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(result) => result,
        Poll::Pending => panic!("cooperative capture unexpectedly yielded"),
    }
}

// The linked semantic crates use alloc, but this electrical fixture admits no
// heap. Any unexpected allocation fails instead of hiding capture-time growth.
#[cfg(not(feature = "live"))]
struct NoHeap;
#[cfg(not(feature = "live"))]
unsafe impl core::alloc::GlobalAlloc for NoHeap {
    unsafe fn alloc(&self, _: core::alloc::Layout) -> *mut u8 {
        core::ptr::null_mut()
    }
    unsafe fn dealloc(&self, _: *mut u8, _: core::alloc::Layout) {}
}
#[cfg(not(feature = "live"))]
#[global_allocator]
static NO_HEAP: NoHeap = NoHeap;
