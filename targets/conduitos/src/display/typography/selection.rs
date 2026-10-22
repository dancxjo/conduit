//! Immutable Host profile selection; five text purposes remain portable.
use super::{ProfileMetrics, TextRole};
use conduit_face::VisualTypography;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Typography(VisualTypography);
impl Typography {
    pub const EDITORIAL: Self = Self(VisualTypography::Editorial);
    pub const fn new(intent: VisualTypography) -> Self {
        Self(intent)
    }
    pub const fn style(self, role: TextRole) -> TextStyle {
        let profile = match (role, self.0) {
            (TextRole::Title, VisualTypography::Engineering) => 5,
            _ => role as u8,
        };
        TextStyle { profile }
    }
}

/// An exact private glyph resource, resolved before measurement or paint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextStyle {
    pub(super) profile: u8,
}
impl From<TextRole> for TextStyle {
    fn from(role: TextRole) -> Self {
        Typography::EDITORIAL.style(role)
    }
}
pub const fn selected_metrics(style: TextStyle) -> &'static ProfileMetrics {
    &super::PROFILES[style.profile as usize]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::typography::{TextLayout, selected_glyph};

    #[test]
    fn intent_selects_two_title_resources_without_changing_other_purposes() {
        let engineering = Typography::new(VisualTypography::Engineering);
        for role in [
            TextRole::Label,
            TextRole::Body,
            TextRole::Heading,
            TextRole::Code,
        ] {
            assert_eq!(Typography::EDITORIAL.style(role), engineering.style(role));
        }
        let editorial = Typography::EDITORIAL.style(TextRole::Title);
        let technical = engineering.style(TextRole::Title);
        assert_ne!(editorial, technical);
        assert!(
            ['i', 'W', 'J', 'é', 'A']
                .into_iter()
                .any(|c| selected_glyph(editorial, c).advance()
                    != selected_glyph(technical, c).advance())
        );
        for style in [editorial, technical] {
            for scale in [100, 125, 200] {
                let value = "WWW J e\u{301} café";
                let layout = TextLayout::new_selected(value, style, 300, scale).unwrap();
                let height = TextLayout::new_selected(value, style, 300, scale)
                    .unwrap()
                    .finish_height();
                for positioned in layout {
                    let (w, h) = positioned.glyph.extent();
                    if w > 0 && h > 0 {
                        let (x, y) = positioned.glyph.bearing();
                        assert!(positioned.pen.0 + i32::from(x) >= 0);
                        assert!(positioned.pen.0 + i32::from(x) + i32::from(w) <= 300);
                        assert!(positioned.pen.1 + i32::from(y) >= 0);
                        assert!(positioned.pen.1 + i32::from(y) + i32::from(h) <= height as i32);
                    }
                }
            }
        }
    }
}
