//! Selected Look geometry below Mask layout and hit authority.
use super::FaceSceneError;
use conduit_face::{GraphicsCommand, GraphicsError, LookVisualTokensInfo};

#[derive(Clone, Copy)]
pub(super) struct LookCorners {
    pub(super) control: u8,
    pub(super) panel: u8,
}
impl LookCorners {
    pub(super) fn from_tokens(tokens: &LookVisualTokensInfo) -> Result<Self, FaceSceneError> {
        let (control, panel) = (*tokens.radius_control_px(), *tokens.radius_panel_px());
        if control > 32 || panel > 32 {
            return Err(FaceSceneError::InvalidExtent);
        }
        Ok(Self {
            control: control as u8,
            panel: panel as u8,
        })
    }
}
/// Explicit zero stays square; absence preserves the legacy raster path.
/// One-pixel ordinary borders and the separate focus marker keep distinct jobs.
pub(super) fn apply(
    command: GraphicsCommand,
    radius: Option<u8>,
) -> Result<GraphicsCommand, GraphicsError> {
    match radius {
        Some(radius) => command.with_rect_metrics(radius, 1),
        None => Ok(command),
    }
}
