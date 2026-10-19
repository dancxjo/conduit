//! Cursor motion shares this workspace's admitted Mask and current Show.
use super::*;
impl FaceWorkspace {
    pub(in crate::product_front_door) fn pointer_motion(
        &mut self,
        x: u32,
        y: u32,
        display: &mut impl PixelTarget,
    ) -> Result<(), &'static str> {
        if !self.active {
            return Err("workspace-face-inactive");
        }
        self.mask
            .pointer_motion(x, y, display)
            .map_err(|error| error.as_str())
    }
}
