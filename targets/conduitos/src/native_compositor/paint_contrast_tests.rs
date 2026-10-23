//! Contrast of resolved native paint tokens, not raster/accessibility certification.
use super::visual_paint;
use crate::display::DisplayFormat;
use conduit_face::{
    AdmittedVisualExpression, GraphicsPaintRole as Role, VisualExpression, VisualPalette,
    VisualUserConstraints, admit_visual_expression,
};

fn expression(palette: VisualPalette, forced: bool, missing: bool) -> AdmittedVisualExpression {
    let mut request = VisualExpression::phosphor();
    request.palette = palette;
    admit_visual_expression(
        if missing {
            Err(conduit_face::VisualExpressionRefusal::Encoding)
        } else {
            Ok(request)
        },
        VisualUserConstraints {
            forced_colors: forced,
            reduced_motion: true,
            minimum_text_scale_percent: 200,
            maximum_supported_text_scale_percent: 200,
        },
    )
    .unwrap()
}

fn luminance(pixel: u32, format: DisplayFormat) -> f64 {
    let linear = |shift| {
        let channel = f64::from((pixel >> shift) & 255) / 255.0;
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(format.red_shift)
        + 0.7152 * linear(format.green_shift)
        + 0.0722 * linear(format.blue_shift)
}

fn check(expression: Option<&AdmittedVisualExpression>) {
    // Both actual RGB/BGR channel layouts must preserve the selected colors.
    for (red_shift, blue_shift) in [(16, 0), (0, 16)] {
        let format = DisplayFormat {
            width: 640,
            height: 480,
            pitch: 2560,
            bits_per_pixel: 32,
            red_shift,
            green_shift: 8,
            blue_shift,
        };
        for background in [Role::Background, Role::Surface] {
            let backdrop = luminance(visual_paint(format, background, expression), format);
            for (role, minimum) in [
                (Role::Foreground, 4.5),
                (Role::Muted, 4.5),
                (Role::Accent, 4.5),
                (Role::Status, 4.5),
                (Role::Warning, 4.5),
                (Role::Success, 4.5),
                (Role::Danger, 4.5),
                (Role::Focus, 3.0),
                (Role::Hovered, 3.0),
                (Role::Selected, 3.0),
                (Role::Border, 3.0),
            ] {
                let ink = luminance(visual_paint(format, role, expression), format);
                let ratio = (ink.max(backdrop) + 0.05) / (ink.min(backdrop) + 0.05);
                assert!(ratio >= minimum, "{role:?} on {background:?}: {ratio}");
            }
        }
    }
}

#[test]
fn both_native_palettes_keep_text_and_control_contrast_on_actual_surfaces() {
    check(None);
    for palette in [VisualPalette::Phosphor, VisualPalette::Editorial] {
        check(Some(&expression(palette, false, false)));
    }
}

#[test]
fn forced_colors_and_missing_look_fallback_keep_the_same_contrast_contract() {
    for palette in [VisualPalette::Phosphor, VisualPalette::Editorial] {
        for forced in [false, true] {
            for missing in [false, true] {
                let admitted = expression(palette, forced, missing);
                assert_eq!(admitted.platform_forced_colors, forced);
                assert_eq!(admitted.expression.text_scale_percent, 200);
                check(Some(&admitted));
            }
        }
    }
}
