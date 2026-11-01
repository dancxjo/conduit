//! Preparation checkpoint only: no Mask route, output Play, or GPIO write.
use alloc::boxed::Box;
use conduit_environment_runtime::display_mask::PreparedDisplayMask;
use conduit_seven_segment_mask::{DigitWiring, SevenSegmentLook, Tm1637Profile, WireBit};
pub fn prepare() -> Result<Box<PreparedDisplayMask>, &'static str> {
    PreparedDisplayMask::prepare(
        Tm1637Profile {
            digits: core::array::from_fn(|index| DigitWiring {
                address: index as u8,
                segment_bits: [0, 1, 2, 3, 4, 5, 6],
            }),
            decimal_points: [None; 4],
            colon: Some(WireBit { address: 1, bit: 7 }),
        },
        SevenSegmentLook::fahrenheit().with_colon_fraction(),
        3_000,
        24 * 60 * 60 * 1_000,
        3,
    )
    .map(Box::new)
    .map_err(|_| "display-fit-mask")
}
#[derive(serde::Serialize)]
pub struct Telemetry {
    software_retained: bool,
    mask_bytes: usize,
    physical_write: Option<bool>,
    mask_play_installed: bool,
    additional_kernel_preparation_bytes: Option<usize>,
    free_root_bytes: usize,
    kernel_preparation_fits_current_headroom: bool,
}
pub fn telemetry(display: &super::NativeDisplay) -> Telemetry {
    let additional = super::kernel_preparation_bound().ok();
    let stats = esp_alloc::HEAP.stats();
    let free = stats.size.saturating_sub(stats.current_usage);
    #[allow(unused_mut)]
    let mut mask_bytes = display.fit.as_deref().map_or(0, core::mem::size_of_val);
    #[cfg(feature = "display-physical-check")]
    let physical_write = display.task_status().map_or(Some(false), |status| {
        if mask_bytes == 0 && status.task_active {
            mask_bytes = core::mem::size_of::<PreparedDisplayMask>();
        }
        // Running counters are intentionally unavailable until the worker has
        // returned custody; null does not assert absence of in-flight effects.
        status.progress_current.then_some(status.native_calls > 0)
    });
    #[cfg(not(feature = "display-physical-check"))]
    let physical_write = Some(false);
    Telemetry {
        software_retained: display.software_ready() || display.kernel_prepared(),
        mask_bytes,
        physical_write,
        mask_play_installed: false,
        additional_kernel_preparation_bytes: additional,
        free_root_bytes: free,
        kernel_preparation_fits_current_headroom: additional.is_some_and(|needed| needed <= free),
    }
}
