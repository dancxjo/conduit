//! Intrinsic demand from the same selected scalar ink used by the paint cursor.
//! This is the native hard-wrap policy, not general shaping or CSS conformance.
use super::{TextScale, TextStyle, ink, selected_metrics};
use crate::display::DisplayError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IntrinsicTextMetrics {
    /// Smallest width that preserves every indivisible base/mark ink cluster.
    pub minimum_inline: u32,
    /// Longest explicit line without soft wrapping, including ink overhang.
    pub preferred_inline: u32,
    pub line_height: u32,
    pub explicit_lines: u32,
}

/// At most 192 bytes, two linear scans, no allocation. Maximum usable extents
/// remain a separate admitted container constraint, not a property of text.
pub fn intrinsic_text_metrics(
    value: &str,
    style: TextStyle,
    percent: u16,
) -> Result<IntrinsicTextMetrics, DisplayError> {
    intrinsic_item_text_metrics(value, style, percent, conduit_face::MAX_GRAPHICS_TEXT_BYTES)
}

/// Whole-item preparation demand under the caller's existing document byte
/// admission. This is independent of paint-command partitioning and does not
/// relax the 192-byte command API. Two linear scans, no retained storage.
pub fn intrinsic_item_text_metrics(
    value: &str,
    style: TextStyle,
    percent: u16,
    maximum_bytes: usize,
) -> Result<IntrinsicTextMetrics, DisplayError> {
    let scale = TextScale::new(percent)?;
    if value.len() > maximum_bytes {
        return Err(DisplayError::BufferTooSmall);
    }
    let mut minimum_inline = 0;
    let mut remaining = value;
    while !remaining.is_empty() {
        let bytes = ink::cluster_bytes(remaining, style, scale);
        let cluster = &remaining[..bytes];
        if cluster != "\n" {
            minimum_inline = minimum_inline
                .max(ink::checked_span(cluster, style, scale).ok_or(DisplayError::InvalidExtent)?);
        }
        remaining = &remaining[bytes..];
    }
    let mut preferred_inline = 0;
    let mut explicit_lines = 0_u32;
    for line in value.split('\n') {
        preferred_inline = preferred_inline
            .max(ink::checked_span(line, style, scale).ok_or(DisplayError::InvalidExtent)?);
        explicit_lines = explicit_lines
            .checked_add(1)
            .ok_or(DisplayError::InvalidExtent)?;
    }
    Ok(IntrinsicTextMetrics {
        minimum_inline,
        preferred_inline,
        line_height: scale.unsigned(u32::from(selected_metrics(style).line_height)),
        explicit_lines,
    })
}

/// Resolve a portable em extent against the exact selected font em, with a single
/// outward rounding step. Line spacing is deliberately not used as an em proxy.
pub fn resolve_em_sixteenths(
    style: TextStyle,
    percent: u16,
    units: u16,
) -> Result<u16, DisplayError> {
    TextScale::new(percent)?;
    if units > 4096 {
        return Err(DisplayError::InvalidExtent);
    }
    let pixels =
        (u32::from(selected_metrics(style).em_size) * u32::from(percent) * u32::from(units))
            .div_ceil(1600);
    u16::try_from(pixels).map_err(|_| DisplayError::InvalidExtent)
}

#[cfg(test)]
#[path = "intrinsic_tests.rs"]
mod tests;
