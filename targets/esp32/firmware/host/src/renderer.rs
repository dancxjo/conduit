//! Checked local renderer construction, distinct from GPIO output execution.
use alloc::format;
use conduit_core::{HostAdvertisement, ResourceGenerationId};
use conduit_mcu_host_runtime::execution_mask::{Bounds, Identity, ReadyRenderer, RendererClock};
use conduit_seven_segment_mask::{DigitWiring, SevenSegmentLook, Tm1637Profile, WireBit};

/// Owns clock slot zero; it is never also published as a wait provider.
pub struct Clock(pub(crate) crate::runtime::MonotonicTimer);
impl RendererClock for Clock {
    fn now_ms(&self) -> u64 {
        conduit_esp32_electrical::platform::monotonic_millis()
    }
    fn retained_backing_bytes(&self) -> Option<usize> {
        Some(0)
    }
}
pub fn initialize(actual: &HostAdvertisement) -> ReadyRenderer<Clock> {
    let bounds = Bounds {
        face_bytes: crate::config::RENDERER_FACE_BYTES,
        show_bytes: crate::config::RENDERER_SHOW_BYTES,
        resource_bytes: crate::config::RENDERER_RESOURCE_BYTES,
        maximum_frames: crate::config::RENDERER_MAXIMUM_FRAMES,
    };
    let look = if crate::config::RENDERER_FAHRENHEIT {
        SevenSegmentLook::fahrenheit()
    } else {
        SevenSegmentLook::celsius()
    };
    // This BSP's checked four-digit TM1637 layout uses A..G bits0..6 and
    // the second register's bit7 colon. GPIO ownership remains in ReadyWrite.
    let profile = Tm1637Profile {
        digits: core::array::from_fn(|address| DigitWiring {
            address: address as u8,
            segment_bits: [0, 1, 2, 3, 4, 5, 6],
        }),
        decimal_points: [None; 4],
        colon: Some(WireBit { address: 1, bit: 7 }),
    };
    let mask = conduit_environment_runtime::display_mask::PreparedDisplayMask::prepare(
        profile,
        look,
        crate::config::RENDERER_PAGE_PERIOD_MS,
        crate::config::RENDERER_SHOW_LIFETIME_MS,
        crate::config::TM_BRIGHTNESS,
    )
    .expect("checked Look, layout and finite Show timing");
    let identity = Identity {
        host_id: actual.host_id.clone(),
        boot_id: actual.boot_id.clone(),
        base_id: "esp32/face-renderer".into(),
        provider_instance_id: format!("{}/face-renderer", actual.host_id.as_str()).into(),
        provider_generation: 1,
        resource_pool_id: "pool/esp32/face-renderer".into(),
        resource_generation_id: ResourceGenerationId(format!(
            "{}/face-renderer",
            actual.boot_id.as_str()
        )),
        envelope_id: "envelope/esp32/face-renderer".into(),
        artifact_id: crate::config::IMAGE_ID.into(),
        implementation_id: "esp32/seven-segment-renderer@1".into(),
        base_implementation_id: "esp32/seven-segment-renderer@1".into(),
    };
    // SAFETY: checked construction, original physical Host/Boot and distinct
    // owned slot0. No Plan may enlarge these buffers or create GPIO custody.
    unsafe {
        ReadyRenderer::new(
            identity,
            Clock(crate::runtime::MonotonicTimer::new(0)),
            mask,
            bounds,
        )
    }
    .expect("finite initialized renderer backing")
}
