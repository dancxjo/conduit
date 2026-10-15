//! Built-in Look data admitted by the canonical checked Source contract.
//! Device/provider selection and component topology are not Look resources.
use crate::*;
use conduit_plot::rust_binding::NativeBindingRefusal;

pub const MAX_LOOK_BYTES: usize = 65_536;
pub const PHOSPHOR_LOOK_ID: &str = "conduit.look/phosphor@1";
pub const LOOK_SOURCE: &str = include_str!("../look.conduit");

pub fn phosphor_look() -> Result<LookInfo, NativeBindingRefusal> {
    let expression = VisualExpressionInfo::new(
        VisualDensityPreference::Compact,
        PHOSPHOR_LOOK_ID.into(),
        VisualMotionPreference::None,
        VisualPalettePreference::Phosphor,
        100,
        1,
    )?;
    let visual = LookVisualFacetOption::some(expression, phosphor_tokens()?)?;
    let spoken = LookSpokenFacetOption::some(LookSpokenDelivery::Neutral, LookVerbosity::Brief)?;
    let generative =
        LookGenerativeFacetOption::some(LookGenerativeRegister::Neutral, LookVerbosity::Brief)?;
    LookInfo::new(generative, PHOSPHOR_LOOK_ID.into(), spoken, 1, visual)
}

fn phosphor_tokens() -> Result<LookVisualTokensInfo, NativeBindingRefusal> {
    LookVisualTokensInfo::new(
        ThemeColor::from_rgb(0x05, 0x07, 0x0b), // background
        ThemeColor::from_rgb(0x09, 0x0d, 0x16), // bootstrap_surface
        ThemeColor::from_rgb(0xe9, 0xa3, 0x25), // emphasis
        ThemeColor::from_rgb(0xff, 0x72, 0x72), // failure
        ThemeColor::from_rgb(0xf4, 0xc4, 0x00), // focus
        3,                                      // focus_width_px
        150,                                    // line_height_percent
        ThemeColor::from_rgb(0x8c, 0x4c, 0x19), // muted
        6,                                      // radius_control_px
        9,                                      // radius_panel_px
        ThemeColor::from_rgb(0x0c, 0x12, 0x1c), // reading_paper
        7,                                      // space_control_block_px
        10,                                     // space_control_inline_px
        13,                                     // space_panel_px
        4,                                      // space_unit_px
        ThemeColor::from_rgb(0x0d, 0xd8, 0xf6), // structure_primary
        ThemeColor::from_rgb(0x0a, 0x1f, 0x87), // structure_secondary
        ThemeColor::from_rgb(0x63, 0xd6, 0x9b), // success
        ThemeColor::from_rgb(0x09, 0x0d, 0x16), // surface
        ThemeColor::from_rgb(0x93, 0xd2, 0xf7), // text_primary
        ThemeColor::from_rgb(0x57, 0x8e, 0xc9), // text_secondary
        16,                                     // type_body_px
        14,                                     // type_small_px
        ThemeColor::from_rgb(0xf2, 0xbd, 0x71), // warning
        ThemeColor::from_rgb(0x05, 0x07, 0x0b), // workbench_canvas
    )
}
