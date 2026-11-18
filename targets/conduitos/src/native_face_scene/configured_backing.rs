//! Scene-only requested backing for checked mechanical layout and admitted Look.
//! Source/configuration decode, prepared structure/Look, incoming Face and their
//! independent captures remain caller-owned. No counter or Show is established.
use super::*;
use core::mem::size_of;

#[path = "configured_backing/image_schema.rs"]
mod image_schema;

impl NativeFaceScene {
    /// Allocation-free preflight for prepare_structured_look. The existing
    /// formatter/input/frame profile stays unchanged; Graph/preferences remain
    /// unsupported. LookCorners and GraphicsImageKey are inline Copy state.
    pub fn configured_interactive_memory_bounds(
        structure: &conduit_face::PreparedComponentLayout<'_>,
        _look: &conduit_face::AdmittedVisualLook,
    ) -> Result<StandardNativeSceneBounds, FaceSceneError> {
        let face = structure.face();
        // Scale, spacing and corners affect geometry, not the byte-per-row
        // envelope. Constructor-owned Look/typography validation is unchanged;
        // this arithmetic preflight does not grant Look validity or readiness.
        let mut bounds = Self::standard_interactive_memory_bounds(face)?;
        let (primary, _) = interactive_backing::document_text_bytes(face)?;
        let mechanical = layout::mechanical_memory_bounds(structure, primary)?;
        let old_rows = interactive_backing::row_vector_backing_bytes(primary)?;
        bounds.retained_bytes = bounds
            .retained_bytes
            .checked_sub(old_rows)
            .and_then(|n| n.checked_add(mechanical.retained_rows))
            .ok_or(FaceSceneError::DocumentBound)?;
        // The standard bound already accounts for semantic formatting and its
        // documents. Add the mechanical containers/text while those input Items
        // still exist; candidates are scored/discarded, never retained together.
        let layout_peak = bounds
            .preparation_peak_bytes
            .checked_add(mechanical.preparation_peak)
            .ok_or(FaceSceneError::DocumentBound)?;
        // image::prepare executes after document Items/layout temporaries drop.
        // Its fixed schema factory overlaps only the finished row pair/Scene.
        let image_peak = bounds
            .retained_bytes
            .checked_add(image_schema::preparation_bytes(face)?)
            .ok_or(FaceSceneError::DocumentBound)?;
        bounds.preparation_peak_bytes = layout_peak.max(image_peak);
        Ok(bounds)
    }

    /// Actual owned heap payload only, including spare capacities. Inline Self,
    /// LookCorners, GraphicsImageKey, allocator overhead and borrowed keymap are
    /// excluded. Source/decode/Mask/Show ownership remains separately charged.
    pub fn configured_interactive_retained_backing_bytes(&self) -> Option<usize> {
        if self.graph_reading.is_some() || self.local_creche_preferences {
            return None;
        }
        let mut bytes = self.face.retained_backing_bytes()?;
        for rows in [&self.primary, &self.details] {
            bytes = bytes.checked_add(rows.capacity().checked_mul(size_of::<layout::Row>())?)?;
            for row in rows {
                bytes = bytes.checked_add(row.text.capacity())?;
            }
        }
        bytes = bytes.checked_add(self.input.retained_backing_bytes()?)?;
        bytes.checked_add(self.local_notice.as_ref().map_or(0, String::capacity))
    }
}

#[cfg(test)]
#[path = "configured_backing/tests.rs"]
mod tests;
