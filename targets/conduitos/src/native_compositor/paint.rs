//! Shared native paint realization for both renderer backends.
#[cfg(feature = "native-compositor-core")]
pub(super) fn visual_paint(
    format: crate::display::DisplayFormat,
    role: conduit_face::GraphicsPaintRole,
    expression: Option<&conduit_face::AdmittedVisualExpression>,
) -> u32 {
    use crate::display::{ACCENT, BACKGROUND, DANGER, FOREGROUND, MUTED, SUCCESS, WARNING};
    use conduit_face::{GraphicsPaintRole as Role, VisualPalette};
    let color = if expression.is_some_and(|value| value.platform_forced_colors) {
        match role {
            Role::Background | Role::Surface => (0, 0, 0),
            Role::Focus | Role::Hovered | Role::Selected => (255, 255, 0),
            _ => (255, 255, 255),
        }
    } else if expression.is_some_and(|value| value.expression.palette == VisualPalette::Editorial) {
        match role {
            Role::Background => (243, 240, 232),
            Role::Foreground => (34, 54, 76),
            Role::Muted => (83, 100, 119),
            Role::Accent | Role::Selected => (41, 79, 120),
            Role::Status | Role::Warning => (128, 85, 19),
            Role::Success => (36, 96, 74),
            Role::Danger => (160, 46, 64),
            Role::Focus | Role::Hovered => (145, 96, 25),
            Role::Surface => (255, 252, 246),
            Role::Border => (129, 124, 115),
        }
    } else {
        match role {
            Role::Background => BACKGROUND,
            Role::Foreground => FOREGROUND,
            Role::Accent | Role::Selected => ACCENT,
            Role::Muted => MUTED,
            Role::Status | Role::Warning => WARNING,
            Role::Success => SUCCESS,
            Role::Danger => DANGER,
            Role::Focus | Role::Hovered => (255, 204, 51),
            Role::Surface => crate::display::SURFACE,
            Role::Border => crate::display::BORDER_PAINT,
        }
    };
    format.pixel(color.0, color.1, color.2)
}

#[cfg(all(test, feature = "native-compositor-core"))]
#[path = "paint_contrast_tests.rs"]
mod contrast_tests;

/// Complete Look tokens lower to the existing finite semantic paint roles.
/// Platform accessibility colors always take precedence over authored tokens.
#[cfg(feature = "native-compositor-core")]
pub(super) fn visual_look_paint(
    format: crate::display::DisplayFormat,
    role: conduit_face::GraphicsPaintRole,
    expression: Option<&conduit_face::AdmittedVisualExpression>,
    look: Option<&conduit_face::LookInfo>,
) -> u32 {
    use conduit_face::{GraphicsPaintRole as Role, LookVisualFacetOption};
    if expression.is_some_and(|value| value.platform_forced_colors) {
        return visual_paint(format, role, expression);
    }
    let Some(LookVisualFacetOption::Some(visual)) = look.map(|look| look.visual()) else {
        return visual_paint(format, role, expression);
    };
    let tokens = visual.tokens();
    let color = match role {
        Role::Background => tokens.background(),
        Role::Surface => tokens.surface(),
        Role::Foreground => tokens.text_primary(),
        Role::Muted => tokens.text_secondary(),
        Role::Accent | Role::Selected => tokens.emphasis(),
        Role::Status | Role::Warning => tokens.warning(),
        Role::Success => tokens.success(),
        Role::Danger => tokens.failure(),
        Role::Focus | Role::Hovered => tokens.focus(),
        Role::Border => tokens.structure_primary(),
    };
    format.pixel(color.red(), color.green(), color.blue())
}
