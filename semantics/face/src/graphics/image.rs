//! An exact immutable image generation key, independent of its residence.
use super::{GraphicsCommand, GraphicsError, LayoutRect};
use crate::{GraphicsCommandKind, GraphicsPaintRole, GraphicsShapeStyle};

pub const GRAPHICS_IMAGE_KEY_BYTES: usize = 64;

/// Exact resource identity plus immutable version. This value is neither
/// possession nor authority to read or display the resource it identifies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphicsImageKey {
    identity: [u8; 32],
    version: [u8; 32],
}

impl GraphicsImageKey {
    pub fn new(identity: [u8; 32], version: [u8; 32]) -> Result<Self, GraphicsError> {
        if identity == [0; 32] || version == [0; 32] {
            return Err(GraphicsError::InvalidImageKey);
        }
        Ok(Self { identity, version })
    }

    pub const fn identity(self) -> [u8; 32] {
        self.identity
    }

    pub const fn version(self) -> [u8; 32] {
        self.version
    }

    pub fn encode(self) -> [u8; GRAPHICS_IMAGE_KEY_BYTES] {
        let mut bytes = [0; GRAPHICS_IMAGE_KEY_BYTES];
        bytes[..32].copy_from_slice(&self.identity);
        bytes[32..].copy_from_slice(&self.version);
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, GraphicsError> {
        let bytes: &[u8; GRAPHICS_IMAGE_KEY_BYTES] = bytes
            .try_into()
            .map_err(|_| GraphicsError::InvalidImageKey)?;
        let mut identity = [0; 32];
        let mut version = [0; 32];
        identity.copy_from_slice(&bytes[..32]);
        version.copy_from_slice(&bytes[32..]);
        Self::new(identity, version)
    }
}

pub(super) fn validate(
    paint: GraphicsPaintRole,
    style: GraphicsShapeStyle,
    payload: &[u8],
) -> Result<GraphicsImageKey, GraphicsError> {
    if paint != GraphicsPaintRole::Foreground || style != GraphicsShapeStyle::Fill {
        return Err(GraphicsError::NonCanonicalEncoding);
    }
    GraphicsImageKey::decode(payload)
}

impl GraphicsCommand {
    /// Draw the exact generation using separately admitted image residence.
    /// Only its 64-byte key is retained. No pixels, locator or access grant is
    /// embedded. Scene encoding version four fences this new graphical leaf.
    pub fn image(
        bounds: LayoutRect,
        clip: LayoutRect,
        key: GraphicsImageKey,
    ) -> Result<Self, GraphicsError> {
        Self::new(
            GraphicsCommandKind::Image,
            bounds,
            clip,
            GraphicsPaintRole::Foreground,
            GraphicsShapeStyle::Fill,
            &key.encode(),
        )
    }

    pub fn image_key(&self) -> Option<GraphicsImageKey> {
        (self.kind == GraphicsCommandKind::Image)
            .then(|| {
                validate(
                    self.paint,
                    self.style,
                    &self.payload[..usize::from(self.payload_len)],
                )
                .ok()
            })
            .flatten()
    }
}

#[cfg(test)]
#[path = "image_tests.rs"]
mod tests;
