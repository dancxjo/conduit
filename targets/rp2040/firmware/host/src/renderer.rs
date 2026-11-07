//! Checked semantic renderer and distinct monotonic read slot; GPIO remains in
//! the separately owned native display-output provider.
use alloc::format;
use conduit_core::{HostAdvertisement, ResourceGenerationId};
use conduit_mcu_host_runtime::execution_mask::{Bounds, Identity, ReadyRenderer, RendererClock};
use conduit_seven_segment_mask::{DigitWiring, SevenSegmentLook, Tm1637Profile, WireBit};
pub struct Clock {
    _slot: usize,
}
impl RendererClock for Clock {
    fn now_ms(&self) -> u64 {
        crate::runtime::now_ms()
    }
    fn retained_backing_bytes(&self) -> Option<usize> {
        Some(0)
    }
}
pub fn initialize(actual: &HostAdvertisement) -> Option<ReadyRenderer<Clock>> {
    use crate::workset::renderer as config;
    let remaining = crate::config::HEAP_BYTES.checked_sub(crate::heap::usage().0)?;
    if config::RENDERER_RESOURCE_BYTES > remaining as u64 {
        return None;
    }
    let look = if config::RENDERER_FAHRENHEIT {
        SevenSegmentLook::fahrenheit()
    } else {
        SevenSegmentLook::celsius()
    };
    // Exact checked "seven4" physical bit wiring; this maps an actual Show and
    // never performs an electrical write or substitutes for the output Plot.
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
        config::RENDERER_PAGE_PERIOD_MS,
        config::RENDERER_SHOW_LIFETIME_MS,
        crate::config::BRIGHTNESS,
    )
    .ok()?;
    let identity = Identity {
        host_id: actual.host_id.clone(),
        boot_id: actual.boot_id.clone(),
        base_id: "rp2040/face-renderer".into(),
        provider_instance_id: format!("{}/face-renderer", actual.host_id.as_str()).into(),
        provider_generation: 1,
        resource_pool_id: "pool/rp2040/face-renderer".into(),
        resource_generation_id: ResourceGenerationId(format!(
            "{}/face-renderer",
            actual.boot_id.as_str()
        )),
        envelope_id: "envelope/rp2040/face-renderer".into(),
        artifact_id: crate::config::IMAGE_ID.into(),
        implementation_id: "rp2040/seven-segment-renderer@1".into(),
        base_implementation_id: "rp2040/seven-segment-renderer@1".into(),
    };
    // SAFETY: synchronous checked construction owns logical clock slot0; wait
    // providers use disjoint slots1..N. Finite wire backing is funded above in
    // the unchanged Host arena. Complete Play admission remains separate.
    unsafe {
        ReadyRenderer::new(
            identity,
            Clock { _slot: 0 },
            mask,
            Bounds {
                face_bytes: config::RENDERER_FACE_BYTES,
                show_bytes: config::RENDERER_SHOW_BYTES,
                resource_bytes: config::RENDERER_RESOURCE_BYTES,
                maximum_frames: config::RENDERER_MAXIMUM_FRAMES,
            },
        )
    }
    .ok()
}
