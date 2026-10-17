//! Fixed graphical font resources. Lookup and coverage sampling allocate nothing.
//!
//! Roles belong to the graphical Presenter, not authored Face identity.
//! Missing front coverage uses the pinned rescue subset; unsupported scalars use
//! its explicit replacement glyph. This is scalar rendering, not general shaping.

mod raster;
pub use raster::{render_glyph, render_text, render_text_scaled};
mod preview;
pub use preview::single_line_preview;
mod layout;
pub use layout::{PositionedGlyph, TextLayout};
mod scene;
pub use scene::{render_scene, render_scene_configured, render_scene_scaled};

/// Text purpose selects a fixed made profile, never an ambient font.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum TextRole {
    Label,
    Body,
    Heading,
    Title,
    Code,
}

impl From<conduit_face::GraphicsTextRole> for TextRole {
    fn from(role: conduit_face::GraphicsTextRole) -> Self {
        use conduit_face::GraphicsTextRole;
        match role {
            GraphicsTextRole::Body => Self::Body,
            GraphicsTextRole::Label => Self::Label,
            GraphicsTextRole::Heading => Self::Heading,
            GraphicsTextRole::Title => Self::Title,
            GraphicsTextRole::Code => Self::Code,
            GraphicsTextRole::Status | GraphicsTextRole::Warning | GraphicsTextRole::Action => {
                Self::Label
            }
            GraphicsTextRole::Muted => Self::Body,
        }
    }
}

pub(super) struct Glyph {
    profile: u8,
    codepoint: u32,
    x: i16,
    y: i16,
    width: u8,
    height: u8,
    advance: u16,
    offset: usize,
}

/// Vertical metrics in physical pixels for one fixed Presenter profile.
pub struct ProfileMetrics {
    pub ascent: i16,
    pub line_height: u16,
}

include!(concat!(env!("OUT_DIR"), "/native_typography.rs"));
static COVERAGE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/native_coverage.bin"));

enum Raster {
    Smooth(&'static Glyph),
    Rescue(&'static super::font::GlyphRecord),
}

/// Borrowed glyph with explicit unsupported-scalar reporting.
pub struct GlyphRaster {
    raster: Raster,
    scale: TextScale,
    pub unsupported: bool,
}

/// Per-render scale admission; no ambient or mutable font configuration.
#[derive(Clone, Copy)]
pub(super) struct TextScale(u16);
impl TextScale {
    pub(super) fn new(percent: u16) -> Result<Self, crate::display::DisplayError> {
        if !(100..=200).contains(&percent) {
            return Err(crate::display::DisplayError::InvalidExtent);
        }
        Ok(Self(percent))
    }
    pub(super) fn unsigned(self, value: u32) -> u32 {
        (value * u32::from(self.0)).div_ceil(100)
    }
    fn signed(self, value: i16) -> i16 {
        (i32::from(value) * i32::from(self.0)).div_euclid(100) as i16
    }
}
pub(super) fn scaled_glyph(role: TextRole, character: char, scale: TextScale) -> GlyphRaster {
    let mut raster = glyph(role, character);
    raster.scale = scale;
    raster
}

pub fn metrics(role: TextRole) -> &'static ProfileMetrics {
    &PROFILES[role as usize]
}

pub fn glyph(role: TextRole, character: char) -> GlyphRaster {
    let key = (role as u8, u32::from(character));
    match GLYPHS.binary_search_by_key(&key, |glyph| (glyph.profile, glyph.codepoint)) {
        Ok(index) => GlyphRaster {
            raster: Raster::Smooth(&GLYPHS[index]),
            scale: TextScale(100),
            unsupported: false,
        },
        Err(_) => {
            let (glyph, unsupported) = super::font::glyph(character);
            GlyphRaster {
                raster: Raster::Rescue(glyph),
                scale: TextScale(100),
                unsupported,
            }
        }
    }
}

impl GlyphRaster {
    /// Horizontal advance in 1/64 pixel units; zero-advance marks stay zero.
    pub fn advance(&self) -> u16 {
        self.scale.unsigned(u32::from(match self.raster {
            Raster::Smooth(glyph) => glyph.advance,
            Raster::Rescue(glyph) => u16::from(glyph.width) * 64,
        })) as u16
    }

    fn base_extent(&self) -> (u8, u8) {
        match self.raster {
            Raster::Smooth(glyph) => (glyph.width, glyph.height),
            Raster::Rescue(glyph) => (glyph.width, super::font::GLYPH_HEIGHT as u8),
        }
    }

    /// Top-left offset relative to the pen at the baseline; y grows downward.
    fn base_bearing(&self) -> (i16, i16) {
        match self.raster {
            Raster::Smooth(glyph) => (glyph.x, -glyph.y - i16::from(glyph.height)),
            Raster::Rescue(_) => (0, -14),
        }
    }

    pub fn extent(&self) -> (u8, u8) {
        let (width, height) = self.base_extent();
        (
            self.scale.unsigned(u32::from(width)) as u8,
            self.scale.unsigned(u32::from(height)) as u8,
        )
    }
    pub fn bearing(&self) -> (i16, i16) {
        let (x, y) = self.base_bearing();
        (self.scale.signed(x), self.scale.signed(y))
    }
    /// Coverage outside the glyph is transparent, including at clipped edges.
    pub fn coverage(&self, x: u8, y: u8) -> u8 {
        let (width, height) = self.extent();
        if x >= width || y >= height {
            return 0;
        }
        let x = (u32::from(x) * 100 / u32::from(self.scale.0)) as u8;
        let y = (u32::from(y) * 100 / u32::from(self.scale.0)) as u8;
        let (width, _) = self.base_extent();
        match self.raster {
            Raster::Smooth(glyph) => {
                COVERAGE[glyph.offset + usize::from(y) * usize::from(width) + usize::from(x)]
            }
            Raster::Rescue(glyph) => {
                let index = usize::from(y) * usize::from(width / 8) + usize::from(x / 8);
                if glyph.bitmap[index] & (0x80 >> (x % 8)) != 0 {
                    255
                } else {
                    0
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doubled_glyph_preserves_every_coverage_sample_in_four_pixels() {
        for character in ['W', 'i', 'ề', '\u{10ffff}'] {
            let original = glyph(TextRole::Body, character);
            let doubled = scaled_glyph(TextRole::Body, character, TextScale::new(200).unwrap());
            let (width, height) = original.extent();
            assert_eq!(doubled.extent(), (width * 2, height * 2));
            for y in 0..height {
                for x in 0..width {
                    for dy in 0..2 {
                        for dx in 0..2 {
                            assert_eq!(
                                doubled.coverage(x * 2 + dx, y * 2 + dy),
                                original.coverage(x, y)
                            );
                        }
                    }
                }
            }
            assert_eq!(doubled.unsupported, original.unsupported);
        }
    }

    #[test]
    fn fixed_tables_are_ordered_bounded_and_have_smooth_coverage() {
        assert!(COVERAGE.len() <= 4 * 1_048_576);
        assert!(GLYPHS.len() <= 5 * 1_024);
        assert!(GLYPHS.windows(2).all(
            |pair| (pair[0].profile, pair[0].codepoint) < (pair[1].profile, pair[1].codepoint)
        ));
        for glyph in GLYPHS {
            assert!(glyph.width <= 64 && glyph.height <= 64);
            assert!(
                glyph.offset + usize::from(glyph.width) * usize::from(glyph.height)
                    <= COVERAGE.len()
            );
        }
        assert!(COVERAGE.iter().any(|&value| value > 0 && value < 255));
    }

    #[test]
    fn prose_is_proportional_and_code_is_monospace() {
        assert_ne!(
            glyph(TextRole::Body, 'i').advance(),
            glyph(TextRole::Body, 'W').advance()
        );
        assert_eq!(
            glyph(TextRole::Code, 'i').advance(),
            glyph(TextRole::Code, 'W').advance()
        );
    }

    #[test]
    fn naming_catalog_and_uppercase_have_admitted_coverage() {
        let names = include_str!("../../../../plots/birth/names/catalog.mjs");
        for role in [
            TextRole::Label,
            TextRole::Body,
            TextRole::Heading,
            TextRole::Title,
            TextRole::Code,
        ] {
            for character in names
                .chars()
                .chain(names.chars().flat_map(char::to_uppercase))
                .filter(|character| !character.is_control())
            {
                let raster = glyph(role, character);
                assert!(!raster.unsupported, "{role:?} missing {character:?}");
                let (width, height) = raster.extent();
                for y in 0..height {
                    for x in 0..width {
                        let _ = raster.coverage(x, y);
                    }
                }
            }
        }
    }

    #[test]
    fn unknown_scalar_has_explicit_deterministic_replacement() {
        let unknown = glyph(TextRole::Body, '\u{10ffff}');
        let (_, expected_unknown) = super::super::font::glyph('\u{10ffff}');
        assert!(unknown.unsupported && expected_unknown);
        assert_eq!(unknown.coverage(255, 255), 0);
        let again = glyph(TextRole::Body, '\u{10ffff}');
        for y in 0..16 {
            for x in 0..16 {
                assert_eq!(unknown.coverage(x, y), again.coverage(x, y));
            }
        }
    }
}
