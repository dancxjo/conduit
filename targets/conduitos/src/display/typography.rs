//! Fixed graphical font resources. Lookup and coverage sampling allocate nothing.
//!
//! Roles belong to the graphical Presenter, not authored Presentation identity.
//! Missing face coverage uses the pinned rescue subset; unsupported scalars use
//! its explicit replacement glyph. This is scalar rendering, not general shaping.

mod raster;
pub use raster::{render_glyph, render_text};
mod layout;
pub use layout::{PositionedGlyph, TextLayout};
mod scene;
pub use scene::render_scene;

/// Text purpose selects a fixed fabricated profile, never an ambient font.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum TextRole {
    Label,
    Body,
    Heading,
    Title,
    Code,
}

impl From<conduit_presentation::GraphicsTextRole> for TextRole {
    fn from(role: conduit_presentation::GraphicsTextRole) -> Self {
        use conduit_presentation::GraphicsTextRole;
        match role {
            GraphicsTextRole::Body => Self::Body,
            GraphicsTextRole::Label => Self::Label,
            GraphicsTextRole::Heading => Self::Heading,
            GraphicsTextRole::Title => Self::Title,
            GraphicsTextRole::Code => Self::Code,
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
    pub unsupported: bool,
}

pub fn metrics(role: TextRole) -> &'static ProfileMetrics {
    &PROFILES[role as usize]
}

pub fn glyph(role: TextRole, character: char) -> GlyphRaster {
    let key = (role as u8, u32::from(character));
    match GLYPHS.binary_search_by_key(&key, |glyph| (glyph.profile, glyph.codepoint)) {
        Ok(index) => GlyphRaster {
            raster: Raster::Smooth(&GLYPHS[index]),
            unsupported: false,
        },
        Err(_) => {
            let (glyph, unsupported) = super::font::glyph(character);
            GlyphRaster {
                raster: Raster::Rescue(glyph),
                unsupported,
            }
        }
    }
}

impl GlyphRaster {
    /// Horizontal advance in 1/64 pixel units; zero-advance marks stay zero.
    pub fn advance(&self) -> u16 {
        match self.raster {
            Raster::Smooth(glyph) => glyph.advance,
            Raster::Rescue(glyph) => u16::from(glyph.width) * 64,
        }
    }

    pub fn extent(&self) -> (u8, u8) {
        match self.raster {
            Raster::Smooth(glyph) => (glyph.width, glyph.height),
            Raster::Rescue(glyph) => (glyph.width, super::font::GLYPH_HEIGHT as u8),
        }
    }

    /// Top-left offset relative to the pen at the baseline; y grows downward.
    pub fn bearing(&self) -> (i16, i16) {
        match self.raster {
            Raster::Smooth(glyph) => (glyph.x, -glyph.y - i16::from(glyph.height)),
            Raster::Rescue(_) => (0, -14),
        }
    }

    /// Coverage outside the glyph is transparent, including at clipped edges.
    pub fn coverage(&self, x: u8, y: u8) -> u8 {
        let (width, height) = self.extent();
        if x >= width || y >= height {
            return 0;
        }
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
        let names = include_str!("../../../../products/creche/names/catalog.mjs");
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
