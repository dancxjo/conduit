//! Borrowed evidence minted only after an entirely successful scanout frame.

use super::{CompositionReceipt, NativeCompositor};

/// Evidence of actual, unobscured surface pixels written in one completed frame.
///
/// This is not a visibility guarantee for every pixel, a hardware vblank receipt,
/// or human observation. Private fields prevent callers constructing an ack;
/// borrowing its compositor prevents mutation while the acknowledgement is used.
pub struct ScanoutAcknowledgement<'a> {
    composition: &'a CompositionReceipt,
    presentation_revision: u64,
    frame_sequence: u64,
    pixels_written: u32,
}

impl ScanoutAcknowledgement<'_> {
    pub fn composition(&self) -> &CompositionReceipt {
        self.composition
    }
    pub const fn presentation_revision(&self) -> u64 {
        self.presentation_revision
    }
    pub const fn frame_sequence(&self) -> u64 {
        self.frame_sequence
    }
    pub const fn pixels_written(&self) -> u32 {
        self.pixels_written
    }
}

impl NativeCompositor {
    /// Look up an exact retained revision in the last successful frame.
    /// Pending scene damage conservatively refuses all acknowledgements until
    /// composition succeeds. No-op frames do not re-acknowledge previous writes.
    pub fn scanout_acknowledgement(
        &self,
        expected: &CompositionReceipt,
        presentation_revision: u64,
    ) -> Option<ScanoutAcknowledgement<'_>> {
        if !self.damage.pending().is_empty() {
            return None;
        }
        let (index, surface) = self
            .surfaces
            .iter()
            .enumerate()
            .find(|(_, surface)| surface.surface_id == expected.surface_id)?;
        if !surface.visible || !surface.is_ready() || self.scanout_pixels[index] == 0 {
            return None;
        }
        let composition = surface.receipt.as_ref()?;
        if composition != expected
            || surface.binding.as_ref()?.last_revision != presentation_revision
        {
            return None;
        }
        Some(ScanoutAcknowledgement {
            composition,
            presentation_revision,
            frame_sequence: self.frame_sequence,
            pixels_written: self.scanout_pixels[index],
        })
    }
}
