use super::*;
use crate::display::typography::{TextLayout, TextRole, Typography};
use alloc::format;
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

#[test]
fn whole_item_demand_uses_one_selected_cursor_across_paint_boundaries() {
    for intent in [VisualTypography::Editorial, VisualTypography::Engineering] {
        for role in [TextRole::Body, TextRole::Title, TextRole::Code] {
            let style = Typography::new(intent).style(role);
            for percent in [100, 125, 200] {
                for value in [
                    "W".repeat(193),
                    format!("{}e\u{301}J", "a".repeat(191)),
                    format!("{}\tJ café\n{}\n", "J".repeat(192), "é".repeat(110)),
                    "長い名前 e\u{301} J ".repeat(30),
                ] {
                    let demand = intrinsic_item_text_metrics(&value, style, percent, 4096).unwrap();
                    let scale = TextScale::new(percent).unwrap();
                    let expected = value
                        .split('\n')
                        .map(|line| ink::span(line, style, scale, 0).fresh_width())
                        .max()
                        .unwrap();
                    assert_eq!(
                        demand.preferred_inline, expected,
                        "{intent:?} {role:?} {percent}"
                    );
                    assert_eq!(demand.explicit_lines as usize, value.split('\n').count());
                    let mut remaining = value.as_str();
                    let mut minimum = 0;
                    while !remaining.is_empty() {
                        let bytes = ink::cluster_bytes(remaining, style, scale);
                        if &remaining[..bytes] != "\n" {
                            minimum = minimum
                                .max(ink::span(&remaining[..bytes], style, scale, 0).fresh_width());
                        }
                        remaining = &remaining[bytes..];
                    }
                    assert_eq!(demand.minimum_inline, minimum);
                    // Preparation support must not change the paint API limit.
                    assert_eq!(
                        intrinsic_text_metrics(&value, style, percent),
                        Err(DisplayError::BufferTooSmall)
                    );
                }
                let value = "W".repeat(384);
                let whole = intrinsic_item_text_metrics(&value, style, percent, 4096).unwrap();
                let chunk = intrinsic_text_metrics(&value[..192], style, percent).unwrap();
                assert!(whole.preferred_inline > chunk.preferred_inline);
            }
        }
    }
}

#[test]
fn whole_item_admission_and_checked_extent_fail_without_relaxing_commands() {
    let style = Typography::EDITORIAL.style(TextRole::Body);
    assert!(intrinsic_item_text_metrics(&"a".repeat(4096), style, 100, 4096).is_ok());
    assert_eq!(
        intrinsic_item_text_metrics(&"a".repeat(4097), style, 100, 4096),
        Err(DisplayError::BufferTooSmall)
    );
    assert_eq!(
        intrinsic_item_text_metrics("x", style, 0, 4096),
        Err(DisplayError::InvalidExtent)
    );
    assert_eq!(
        intrinsic_item_text_metrics("", style, 100, 0)
            .unwrap()
            .explicit_lines,
        1
    );
}
