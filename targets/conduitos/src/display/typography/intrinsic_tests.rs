use super::*;
use crate::display::typography::{TextLayout, TextRole, Typography};
use conduit_face::VisualTypography;

#[test]
fn intrinsic_bounds_match_real_cursor_ink_and_height_for_width() {
    for intent in [VisualTypography::Editorial, VisualTypography::Engineering] {
        for role in [TextRole::Body, TextRole::Title, TextRole::Code] {
            let style = Typography::new(intent).style(role);
            for percent in [100, 125, 200] {
                for value in [
                    "",
                    "J W café",
                    "e\u{301}J",
                    "a\tb\nWWW\n",
                    "長い名前",
                    "\u{10ffff}",
                ] {
                    let demand = intrinsic_text_metrics(value, style, percent).unwrap();
                    assert!(demand.minimum_inline <= demand.preferred_inline);
                    let preferred = u16::try_from(demand.preferred_inline.max(1)).unwrap();
                    assert_eq!(
                        TextLayout::new_selected(value, style, preferred, percent)
                            .unwrap()
                            .finish_height(),
                        demand.line_height * demand.explicit_lines,
                        "{intent:?} {role:?} {percent} {value:?}",
                    );
                    let minimum = u16::try_from(demand.minimum_inline.max(1)).unwrap();
                    for glyph in TextLayout::new_selected(value, style, minimum, percent).unwrap() {
                        let (w, h) = glyph.glyph.extent();
                        if w != 0 && h != 0 {
                            let left = glyph.pen.0 + i32::from(glyph.glyph.bearing().0);
                            assert!(left >= 0);
                            assert!(left + i32::from(w) <= i32::from(minimum));
                        }
                    }
                    if demand.minimum_inline > 1 {
                        assert!(
                            TextLayout::new_selected(value, style, minimum - 1, percent).is_err()
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn portable_spacing_uses_actual_em_and_one_outward_rounding() {
    let body = Typography::EDITORIAL.style(TextRole::Body);
    let label = Typography::EDITORIAL.style(TextRole::Label);
    assert_eq!(selected_metrics(body).em_size, 16);
    assert_eq!(selected_metrics(label).em_size, 14);
    assert_eq!(resolve_em_sixteenths(body, 100, 16), Ok(16));
    assert_eq!(resolve_em_sixteenths(label, 125, 16), Ok(18));
    assert_eq!(resolve_em_sixteenths(label, 125, 1), Ok(2));
    assert_eq!(resolve_em_sixteenths(body, 200, 0), Ok(0));
    assert_eq!(resolve_em_sixteenths(body, 100, 4096), Ok(4096));
    assert!(resolve_em_sixteenths(body, 100, 4097).is_err());
}

#[test]
fn intrinsic_preparation_retains_payload_and_scale_refusals() {
    let style = Typography::EDITORIAL.style(TextRole::Body);
    assert!(intrinsic_text_metrics(&"a".repeat(192), style, 100).is_ok());
    assert_eq!(
        intrinsic_text_metrics(&"a".repeat(193), style, 100),
        Err(DisplayError::BufferTooSmall)
    );
    for percent in [0, 99, 201, u16::MAX] {
        assert!(intrinsic_text_metrics("a", style, percent).is_err());
        assert!(resolve_em_sixteenths(style, percent, 16).is_err());
    }
}
