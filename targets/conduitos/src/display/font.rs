//! Fixed GNU Unifont glyph storage for the ConduitOS framebuffer presenter.

pub(super) const GLYPH_HEIGHT: u32 = 16;
const REPLACEMENT_CODEPOINT: u32 = 0xfffd;

pub(super) struct GlyphRecord {
    codepoint: u32,
    pub(super) width: u8,
    pub(super) bitmap: [u8; 32],
}

include!(concat!(env!("OUT_DIR"), "/conduitos_unifont_subset.rs"));

pub(super) fn glyph(character: char) -> (&'static GlyphRecord, bool) {
    match GLYPHS.binary_search_by_key(&(character as u32), |glyph| glyph.codepoint) {
        Ok(index) => (&GLYPHS[index], false),
        Err(_) => {
            let index = GLYPHS
                .binary_search_by_key(&REPLACEMENT_CODEPOINT, |glyph| glyph.codepoint)
                .expect("build script requires the replacement glyph");
            (&GLYPHS[index], true)
        }
    }
}

#[cfg(test)]
pub(super) fn glyph_count() -> usize {
    GLYPHS.len()
}
