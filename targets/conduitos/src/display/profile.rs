//! The single compiled graphical profile. No discovery, cache, or runtime allocation.
use conduit_presentation::GraphicsTextRole;

pub const PROFILE_ID: &str = "conduitos/graphical/dejavu-v1";
pub use super::tokens::*;

static BODY: &[u8] = include_bytes!("../../assets/graphical/body.atlas");
static HEADING: &[u8] = include_bytes!("../../assets/graphical/heading.atlas");
static TITLE: &[u8] = include_bytes!("../../assets/graphical/title.atlas");
static CODE: &[u8] = include_bytes!("../../assets/graphical/code.atlas");

#[allow(dead_code)]
#[derive(Clone, Copy)]
pub(super) struct Glyph {
    pub advance: u32,
    pub bearing: i32,
    pub width: u32,
    pub height: u32,
    pub coverage: &'static [u8],
}

pub const fn line_height(role: GraphicsTextRole) -> u32 {
    match role {
        GraphicsTextRole::Title => 34,
        GraphicsTextRole::Heading | GraphicsTextRole::Action => 24,
        _ => 18,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GlyphCoverage {
    Primary,
    UnicodeFallback,
    Replacement,
}

pub fn coverage(character: char, role: GraphicsTextRole) -> GlyphCoverage {
    if lookup(character, role).is_some() {
        GlyphCoverage::Primary
    } else if !super::font::glyph(character).1 {
        GlyphCoverage::UnicodeFallback
    } else {
        GlyphCoverage::Replacement
    }
}

pub(super) fn glyph(character: char, role: GraphicsTextRole) -> Option<Glyph> {
    lookup(character, role).or_else(|| {
        if super::font::glyph(character).1 {
            lookup('�', role)
        } else {
            None
        }
    })
}

fn lookup(character: char, role: GraphicsTextRole) -> Option<Glyph> {
    let atlas = match role {
        GraphicsTextRole::Title => TITLE,
        GraphicsTextRole::Heading | GraphicsTextRole::Action => HEADING,
        GraphicsTextRole::Code => CODE,
        _ => BODY,
    };
    let count = word(atlas, 0) as usize;
    let (mut low, mut high) = (0, count);
    while low < high {
        let mid = low + (high - low) / 2;
        let record = 4 + mid * 12;
        match word(atlas, record).cmp(&(character as u32)) {
            core::cmp::Ordering::Less => low = mid + 1,
            core::cmp::Ordering::Greater => high = mid,
            core::cmp::Ordering::Equal => {
                let width = u32::from(atlas[record + 6]);
                let height = u32::from(atlas[record + 7]);
                let offset = 4 + count * 12 + word(atlas, record + 8) as usize;
                return Some(Glyph {
                    advance: u32::from(atlas[record + 4]),
                    bearing: i32::from(atlas[record + 5] as i8),
                    width,
                    height,
                    coverage: &atlas[offset..offset + (width * height) as usize],
                });
            }
        }
    }
    None
}

fn word(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("validated atlas"),
    )
}

pub fn advance(character: char, role: GraphicsTextRole) -> u32 {
    glyph(character, role).map_or_else(
        || u32::from(super::font::glyph(character).0.width),
        |glyph| glyph.advance,
    )
}

/// Called after the ordinary first frame succeeds, never by the primitive renderer.
pub fn emit_boot_receipt() {
    crate::arch::early_write(alloc::format!("CONDUIT_GRAPHICAL_PROFILE {{\"profile\":\"{}\",\"primary\":\"DejaVu Sans\",\"code\":\"DejaVu Sans Mono\",\"fallback\":\"pinned Unifont\",\"primitive\":false}}\n", PROFILE_ID).as_bytes());
}
