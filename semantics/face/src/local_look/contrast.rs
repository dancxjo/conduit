//! Nominal sRGB token checks, not a claim of whole-Mask WCAG conformance.
//! W3C SC1.4.3/1.4.6 text thresholds and SC1.4.11 non-text threshold:
//! https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html
//! https://www.w3.org/WAI/WCAG22/Understanding/contrast-enhanced.html
//! https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html
use crate::{LocalLookContrast, LookVisualTokensInfo, ThemeColor};
include!(concat!(env!("OUT_DIR"), "/local_look_srgb.rs"));

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalLookContrastFailure {
    pub foreground: &'static str,
    pub background: &'static str,
    pub required_tenths: u16,
    /// Conservative lower bound, not a rounded pass/fail comparison.
    pub lower_ratio_milli: u32,
}
fn luminance(color: &ThemeColor) -> (u64, u64) {
    let channel = |value: u8| -> (u64, u64) {
        match value {
            0 => (0, 0),
            255 => (1_000_000_000, 1_000_000_000),
            _ => {
                let floor = u64::from(SRGB_LINEAR_FLOOR[usize::from(value)]);
                // Enclose table-generation rounding instead of rounding a
                // marginal color pair upward across an accessibility threshold.
                (floor.saturating_sub(1), (floor + 2).min(1_000_000_000))
            }
        }
    };
    let (r, g, b) = (
        channel(color.red()),
        channel(color.green()),
        channel(color.blue()),
    );
    (
        r.0 * 2126 + g.0 * 7152 + b.0 * 722,
        r.1 * 2126 + g.1 * 7152 + b.1 * 722,
    )
}
fn lower_ratio_milli(a: &ThemeColor, b: &ThemeColor) -> u32 {
    let (a, b) = (luminance(a), luminance(b));
    let offset = 500_000_000_000u64;
    (((a.0.max(b.0) + offset) * 1000 / (a.1.min(b.1) + offset)).max(1000)) as u32
}
pub(super) fn check(
    tokens: &LookVisualTokensInfo,
    required: &LocalLookContrast,
) -> Result<(), LocalLookContrastFailure> {
    let minimum = if matches!(required, LocalLookContrast::Enhanced) {
        70
    } else {
        45
    };
    for (background, paper) in [
        ("background", tokens.background()),
        ("surface", tokens.surface()),
        ("reading_paper", tokens.reading_paper()),
        ("workbench_canvas", tokens.workbench_canvas()),
        ("bootstrap_surface", tokens.bootstrap_surface()),
    ] {
        for (foreground, ink, tenths) in [
            ("text_primary", tokens.text_primary(), minimum),
            ("text_secondary", tokens.text_secondary(), minimum),
            ("emphasis", tokens.emphasis(), minimum),
            ("warning", tokens.warning(), minimum),
            ("failure", tokens.failure(), minimum),
            ("success", tokens.success(), minimum),
            ("focus", tokens.focus(), 30),
            ("structure_primary", tokens.structure_primary(), 30),
        ] {
            let (a, b) = (luminance(ink), luminance(paper));
            let offset = 500_000_000_000u64;
            let passes = (a.0 + offset) * 10 >= (b.1 + offset) * u64::from(tenths)
                || (b.0 + offset) * 10 >= (a.1 + offset) * u64::from(tenths);
            if !passes {
                return Err(LocalLookContrastFailure {
                    foreground,
                    background,
                    required_tenths: tenths,
                    lower_ratio_milli: lower_ratio_milli(ink, paper),
                });
            }
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    #[test]
    fn transfer_bounds_enclose_reference_for_all_channels_and_do_not_round_thresholds_up() {
        for value in 0..=255u8 {
            let c = f64::from(value) / 255.0;
            let reference = if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            };
            let (lo, hi) = luminance(&ThemeColor::from_rgb(value, value, value));
            assert!(lo as f64 <= reference * 10_000_000_000_000.0);
            assert!(hi as f64 >= reference * 10_000_000_000_000.0);
        }
        let white = ThemeColor::from_rgb(255, 255, 255);
        assert_eq!(
            lower_ratio_milli(&white, &ThemeColor::from_rgb(0, 0, 0)),
            21000
        );
        assert!(lower_ratio_milli(&white, &ThemeColor::from_rgb(118, 118, 118)) >= 4500);
        assert!(lower_ratio_milli(&white, &ThemeColor::from_rgb(119, 119, 119)) < 4500);
    }
}
