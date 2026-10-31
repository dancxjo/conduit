//! Admission of existing typed Look metrics over fixed made glyph resources.
//! No font parser/loader, runtime rasterization, or larger per-glyph work budget.
use super::{PROFILE_GLYPH_EXTENTS, PROFILES, TextRole, Typography};
use conduit_face::{LookVisualTokensInfo, VisualTypography};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LookTypographyRefusal {
    MetricBounds,
    FallbackResource,
    LineBox { role: TextRole },
    GlyphBudget { role: TextRole },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct LookTextMetrics {
    body: u16,
    small: u16,
    line_percent: u16,
    pub(super) maximum_scale: u16,
}
impl LookTextMetrics {
    pub const fn em_size(self, role: TextRole) -> u16 {
        match role {
            TextRole::Label => self.small,
            TextRole::Body | TextRole::Code => self.body,
            TextRole::Heading => (self.body * 5).div_ceil(4),
            TextRole::Title => (self.body * 3).div_ceil(2),
        }
    }
    pub const fn line_height(self, em: u16) -> u16 {
        (em * self.line_percent).div_ceil(100)
    }
}
impl Typography {
    /// Realize the authored body/small sizes and leading. The complete admitted
    /// user scale range is checked before any scene or hit geometry is prepared.
    pub fn with_look_metrics(
        intent: VisualTypography,
        tokens: &LookVisualTokensInfo,
        maximum_user_scale: u16,
    ) -> Result<Self, LookTypographyRefusal> {
        let metrics = LookTextMetrics {
            body: *tokens.type_body_px(),
            small: *tokens.type_small_px(),
            line_percent: *tokens.line_height_percent(),
            maximum_scale: maximum_user_scale,
        };
        if !(8..=64).contains(&metrics.body)
            || !(8..=64).contains(&metrics.small)
            || !(100..=250).contains(&metrics.line_percent)
            || !(100..=200).contains(&maximum_user_scale)
        {
            return Err(LookTypographyRefusal::MetricBounds);
        }
        let selected = Self {
            intent,
            metrics: Some(metrics),
        };
        for role in [
            TextRole::Label,
            TextRole::Body,
            TextRole::Heading,
            TextRole::Title,
            TextRole::Code,
        ] {
            let style = selected.style(role);
            let base = &PROFILES[style.profile as usize];
            let minimum_line = (u32::from(base.line_height) * u32::from(style.em_size))
                .div_ceil(u32::from(base.em_size));
            if u32::from(style.line_height) < minimum_line {
                return Err(LookTypographyRefusal::LineBox { role });
            }
            let (width, height) = PROFILE_GLYPH_EXTENTS[style.profile as usize];
            for edge in [width, height] {
                let realized =
                    (u32::from(edge) * u32::from(style.em_size) * u32::from(maximum_user_scale))
                        .div_ceil(u32::from(base.em_size) * 100);
                if realized > 128 {
                    return Err(LookTypographyRefusal::GlyphBudget { role });
                }
            }
        }
        Ok(selected)
    }
}
#[cfg(test)]
#[path = "look_metrics_tests.rs"]
mod tests;
