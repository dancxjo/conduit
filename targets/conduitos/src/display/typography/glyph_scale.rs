//! One exact size ratio shared by glyph advance, bearings and coverage.
use super::{PROFILES, TextScale, TextStyle};
#[derive(Clone, Copy)]
pub(super) struct GlyphScale {
    pub numerator: u32,
    pub denominator: u32,
}
impl GlyphScale {
    pub fn selected(style: TextStyle, user: TextScale) -> Self {
        let base = u32::from(PROFILES[style.profile as usize].em_size);
        let wanted = if style.em_size == 0 {
            base
        } else {
            u32::from(style.em_size)
        };
        Self {
            numerator: wanted * u32::from(user.0),
            denominator: base * 100,
        }
    }
    pub fn unsigned(self, value: u32) -> u32 {
        (value * self.numerator).div_ceil(self.denominator)
    }
    pub fn signed(self, value: i16) -> i16 {
        (i32::from(value) * self.numerator as i32).div_euclid(self.denominator as i32) as i16
    }
    pub fn is_identity(self) -> bool {
        self.numerator == self.denominator
    }
}
