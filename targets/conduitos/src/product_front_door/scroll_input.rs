//! Focused keyboard scrolling for presenter-owned native surface state.

use alloc::format;

use crate::{
    arch,
    display::PixelTarget,
    tour_shell::{ScrollDirection, ScrollOutcome, TourShellPresenter},
};

const HOME: u8 = 74;
const PAGE_UP: u8 = 75;
const END: u8 = 77;
const PAGE_DOWN: u8 = 78;

pub(super) fn accept(
    usage: u8,
    shell: &mut TourShellPresenter,
    display: &mut impl PixelTarget,
) -> Result<bool, &'static str> {
    let direction = match usage {
        HOME => ScrollDirection::Start,
        PAGE_UP => ScrollDirection::Backward,
        END => ScrollDirection::End,
        PAGE_DOWN => ScrollDirection::Forward,
        _ => return Ok(false),
    };
    match shell
        .scroll_focused(direction, display)
        .map_err(|error| error.as_str())?
    {
        ScrollOutcome::Updated(receipt) => {
            let line = format!(
                "CONDUIT_SCROLL_SIGN {{\"schema\":\"conduit.conduitos.surface-scroll/v1\",\"surface_id\":\"{}\",\"previous_offset\":{},\"current_offset\":{},\"manifestation_id\":\"{}\",\"frame_sequence\":{},\"damage_count\":{},\"bounded\":true}}\n",
                receipt.surface_id,
                receipt.previous_offset,
                receipt.current_offset,
                receipt.composition.manifestation_id.as_str(),
                receipt.frame.frame_sequence,
                receipt.frame.damage_count,
            );
            arch::early_write(line.as_bytes());
            arch::early_write(b"CONDUIT_TOUR_CHECKPOINT focused-surface-scrolled\n");
        }
        ScrollOutcome::Boundary => {
            arch::early_write(b"CONDUIT_TOUR_CHECKPOINT focused-scroll-boundary\n");
        }
        ScrollOutcome::Ineligible => {}
    }
    Ok(true)
}
