//! Native row rhythm, separate from source-owned structural gaps and padding.
use super::*;

#[derive(Clone, Copy)]
pub(in crate::native_face_scene) struct RowSpacing {
    pub padding: u16,
    pub gap: u16,
}
impl RowSpacing {
    pub fn legacy(padding: u16) -> Self {
        Self {
            padding,
            gap: SPACE_SM,
        }
    }
    pub fn validate(self) -> Result<Self, FaceSceneError> {
        if !(SPACE_SM..=SPACE_SM * 2).contains(&self.padding) || self.gap > 32 {
            return Err(FaceSceneError::InvalidExtent);
        }
        Ok(self)
    }
}
