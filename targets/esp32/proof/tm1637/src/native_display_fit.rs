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
    physical_write: bool,
    mask_play_installed: bool,
    additional_kernel_preparation_bytes: Option<usize>,
    free_root_bytes: usize,
    kernel_preparation_fits_current_headroom: bool,
}
pub fn telemetry(mask: &PreparedDisplayMask, software_retained: bool) -> Telemetry {
    let additional = super::kernel_preparation_bound().ok();
    let stats = esp_alloc::HEAP.stats();
    let free = stats.size.saturating_sub(stats.current_usage);
    Telemetry {
        software_retained,
        mask_bytes: core::mem::size_of_val(mask),
        physical_write: false,
        mask_play_installed: false,
        additional_kernel_preparation_bytes: additional,
        free_root_bytes: free,
        kernel_preparation_fits_current_headroom: additional.is_some_and(|needed| needed <= free),
    }
}
