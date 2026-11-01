//! Finite resolved rectangle geometry, independent of Look selection or Face meaning.
use super::*;

pub(super) fn validate(style: GraphicsShapeStyle, payload: &[u8]) -> Result<(), GraphicsError> {
    if payload.is_empty() {
        return Ok(());
    }
    if payload.len() != 2
        || payload[0] > 32
        || !(1..=8).contains(&payload[1])
        || !matches!(
            style,
            GraphicsShapeStyle::RoundedFill | GraphicsShapeStyle::RoundedStroke
        )
    {
        return Err(GraphicsError::NonCanonicalEncoding);
    }
    Ok(())
}

impl GraphicsCommand {
    /// Resolve a rounded rectangle's radius (0..=32px) and inset stroke (1..=8px).
    /// Zero radius is square. Rasterizers clamp radius to half the short side;
    /// a stroke covering the interior fills it. Clipping never creates a border.
    /// Storage and the scene's maximum command/payload/encoded-byte bounds stay
    /// unchanged. A scene containing these metrics uses encoding version 3.
    pub fn with_rect_metrics(mut self, radius: u8, stroke: u8) -> Result<Self, GraphicsError> {
        if self.kind != GraphicsCommandKind::Rect {
            return Err(GraphicsError::NonCanonicalEncoding);
        }
        validate(self.style, &[radius, stroke])?;
        self.payload[..2].copy_from_slice(&[radius, stroke]);
        self.payload_len = 2;
        Ok(self)
    }

    /// Explicit (radius, inset stroke), or no override for a legacy rectangle.
    /// This binary geometry is never exposed as textual content by `payload`.
    pub fn rect_metrics(&self) -> Option<(u8, u8)> {
        (self.kind == GraphicsCommandKind::Rect && self.payload_len == 2)
            .then(|| (self.payload[0], self.payload[1]))
    }
}

#[cfg(test)]
#[path = "rectangle_tests.rs"]
mod tests;
