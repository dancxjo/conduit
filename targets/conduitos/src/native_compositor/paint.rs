//! Shared native paint realization for both renderer backends.
#[cfg(feature = "native-compositor")]
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
            Role::Background => (12, 24, 43),
            Role::Foreground => (231, 236, 243),
            Role::Muted => (153, 173, 199),
            Role::Accent | Role::Selected => (114, 180, 255),
            Role::Status | Role::Warning => (243, 198, 111),
            Role::Success => (111, 218, 169),
            Role::Danger => (255, 120, 135),
            Role::Focus | Role::Hovered => (255, 204, 51),
            Role::Surface => (19, 35, 55),
            Role::Border => (84, 113, 150),
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
