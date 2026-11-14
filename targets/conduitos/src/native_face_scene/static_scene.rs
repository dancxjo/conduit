//! Exact initial semantic reading without any installed local-input entrance.
use super::*;

/// Retains the complete semantic Face and ordinary primary/details documents.
/// No mutable Scene/input/navigation API escapes this read-only owner. The
/// input-only Patchbay navigation projection is never allocated or claimed.
pub struct PreparedStaticNativeFaceScene {
    scene: NativeFaceScene,
}
impl PreparedStaticNativeFaceScene {
    pub fn prepare(face: Face, width: u16, height: u16) -> Result<Self, FaceSceneError> {
        NativeFaceScene::prepare_with_local_input(face, width, height, false, false)
            .map(|scene| Self { scene })
    }
    pub fn frame(&self) -> Result<FaceFrame<'_>, FaceSceneError> {
        self.scene.frame()
    }
    /// Actual owned heap only; default keymap is borrowed and GraphReading/input
    /// buffers are absent by construction. Inline frame graphics/stack excluded.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        let mut bytes = self.scene.face.retained_backing_bytes()?;
        for rows in [&self.scene.primary, &self.scene.details] {
            bytes = bytes.checked_add(
                rows.capacity()
                    .checked_mul(core::mem::size_of::<layout::Row>())?,
            )?;
            for row in rows {
                bytes = bytes.checked_add(row.text.capacity())?;
            }
        }
        Some(bytes)
    }
}
impl FaceFrame<'_> {
    /// GraphicsScene and hits' references are inline; only the hit vector owns
    /// heap storage. Borrowed Face actions are not counted a second time.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        self.hits
            .capacity()
            .checked_mul(core::mem::size_of::<FaceHit<'_>>())
    }
}

#[cfg(test)]
#[path = "static_scene/tests.rs"]
mod tests;
