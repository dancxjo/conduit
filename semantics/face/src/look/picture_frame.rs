//! Checked picture-frame appearance, available as a whole Look or fixed native projection.
use crate::*;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};

include!(concat!(env!("OUT_DIR"), "/picture-frame-style.rs"));

pub fn picture_frame_look() -> Result<LookInfo, NativeBindingRefusal> {
    LookInfo::decode(include_bytes!(concat!(
        env!("OUT_DIR"),
        "/picture-frame.look"
    )))
}

pub fn picture_frame_expression() -> Result<AdmittedVisualExpression, VisualExpressionRefusal> {
    admit_visual_expression(
        Ok(VisualExpression {
            identity: PICTURE_FRAME_LOOK_ID.into(),
            version: VISUAL_EXPRESSION_VERSION,
            density: DENSITY,
            palette: PALETTE,
            text_scale_percent: SCALE,
            typography: TYPOGRAPHY,
            motion: MOTION,
        }),
        VisualUserConstraints {
            forced_colors: false,
            reduced_motion: true,
            minimum_text_scale_percent: 100,
            maximum_supported_text_scale_percent: 200,
        },
    )
}

pub const fn picture_frame_text_metrics() -> (u16, u16, u16) {
    (TYPE_BODY_PX, TYPE_SMALL_PX, LINE_HEIGHT_PERCENT)
}

pub const fn picture_frame_color(role: GraphicsPaintRole) -> [u8; 3] {
    use GraphicsPaintRole as R;
    match role {
        R::Background => BACKGROUND,
        R::Surface => SURFACE,
        R::Foreground => TEXT_PRIMARY,
        R::Muted => TEXT_SECONDARY,
        R::Accent | R::Selected => EMPHASIS,
        R::Status | R::Warning => WARNING,
        R::Success => SUCCESS,
        R::Danger => FAILURE,
        R::Focus | R::Hovered => FOCUS,
        R::Border => STRUCTURE_PRIMARY,
    }
}
