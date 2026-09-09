//! Fabrication-only preparation of the fixed native graphical text repertoire.

use std::{collections::BTreeSet, env, fmt::Write as _, fs, path::PathBuf};

use sha2::{Digest, Sha256};
use fontdue::{Font, FontSettings};

const MAX_FONT_BYTES: usize = 1_048_576;
const MAX_CODEPOINTS: usize = 1_024;
const MAX_COVERAGE_BYTES: usize = 4 * 1_048_576;
const MAX_GLYPH_EDGE: usize = 64;
const SUBSET: &str = "../../products/patchbay/native/assets/unifont/unifont-17.0.04-patchbay.hex";
const NAMES: &str = "../../products/creche/names/catalog.mjs";
const FONTS: &[(&str, &str)] = &[
    (
        "DejaVuSans.ttf",
        "7da195a74c55bef988d0d48f9508bd5d849425c1770dba5d7bfc6ce9ed848954",
    ),
    (
        "DejaVuSansMono.ttf",
        "b4a6c3e4faab8773f4ff761d56451646409f29abedd68f05d38c2df667d3c582",
    ),
];
// Fixed profile index: label, body, heading, title, code. These are Presenter
// resources, never part of a portable Presentation or authored Form identity.
const PROFILES: &[(usize, u8)] = &[(0, 14), (0, 16), (0, 20), (0, 24), (1, 14)];

pub fn generate() {
    println!("cargo:rerun-if-changed={SUBSET}");
    println!("cargo:rerun-if-changed={NAMES}");
    let mut repertoire = BTreeSet::new();
    for line in fs::read_to_string(SUBSET)
        .expect("pinned fallback subset")
        .lines()
    {
        let codepoint = u32::from_str_radix(line.split_once(':').unwrap().0, 16).unwrap();
        repertoire.insert(char::from_u32(codepoint).expect("Unicode scalar"));
    }
    let names = fs::read_to_string(NAMES).expect("shared naming catalog");
    repertoire.extend(
        names
            .chars()
            .chain(names.chars().flat_map(char::to_uppercase))
            .filter(|character| !character.is_control()),
    );
    assert!(
        repertoire.len() <= MAX_CODEPOINTS,
        "graphical text repertoire exceeds admission"
    );

    let fonts: Vec<_> = FONTS
        .iter()
        .map(|(name, checksum)| {
            let path = format!("assets/fonts/dejavu/{name}");
            println!("cargo:rerun-if-changed={path}");
            let bytes = fs::read(path).expect("pinned graphical font");
            assert!(
                bytes.len() <= MAX_FONT_BYTES,
                "font source exceeds admission"
            );
            assert_eq!(
                format!("{:x}", Sha256::digest(&bytes)),
                *checksum,
                "font source changed"
            );
            Font::from_bytes(bytes, FontSettings::default()).expect("verified TrueType source")
        })
        .collect();
    let mut coverage = Vec::new();
    let mut records = String::from("pub(super) static GLYPHS: &[Glyph] = &[\n");
    let mut profiles = String::from("pub(super) static PROFILES: &[ProfileMetrics] = &[\n");
    for (profile, &(face, size)) in PROFILES.iter().enumerate() {
        let font = &fonts[face];
        let line = font
            .horizontal_line_metrics(f32::from(size))
            .expect("horizontal font metrics");
        writeln!(
            profiles,
            "ProfileMetrics {{ ascent: {}, line_height: {} }},",
            line.ascent.ceil() as i16,
            line.new_line_size.ceil() as u16
        )
        .unwrap();
        for &character in &repertoire {
            // Absent scalars remain explicit at lookup: the native runtime may
            // choose its pinned fallback, never the font parser's implicit .notdef.
            if font.lookup_glyph_index(character) == 0 {
                continue;
            }
            let (metrics, bitmap) = font.rasterize(character, f32::from(size));
            assert!(metrics.width <= MAX_GLYPH_EDGE && metrics.height <= MAX_GLYPH_EDGE);
            assert_eq!(bitmap.len(), metrics.width * metrics.height);
            assert!(
                coverage.len() + bitmap.len() <= MAX_COVERAGE_BYTES,
                "coverage exceeds admission"
            );
            let offset = coverage.len();
            coverage.extend_from_slice(&bitmap);
            writeln!(records, "Glyph {{ profile: {profile}, codepoint: {}, x: {}, y: {}, width: {}, height: {}, advance: {}, offset: {offset} }},",
                u32::from(character), metrics.xmin, metrics.ymin, metrics.width, metrics.height,
                (metrics.advance_width * 64.0).round() as u16).unwrap();
        }
    }
    records.push_str("];\n");
    profiles.push_str("];\n");
    records.push_str(&profiles);
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo OUT_DIR"));
    fs::write(output.join("native_typography.rs"), records).expect("fixed glyph records");
    fs::write(output.join("native_coverage.bin"), coverage).expect("fixed coverage table");
}
