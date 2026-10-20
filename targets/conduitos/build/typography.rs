//! Make-only preparation of the fixed native graphical text repertoire.

use std::{collections::BTreeSet, env, fmt::Write as _, fs, path::PathBuf};

use fontdue::{Font, FontSettings};
use sha2::{Digest, Sha256};

const MAX_FONT_BYTES: usize = 1_048_576;
const MAX_CODEPOINTS: usize = 1_024;
const MAX_COVERAGE_BYTES: usize = 4 * 1_048_576;
const MAX_GLYPH_EDGE: usize = 64;
const SUBSET: &str = "../../mechanisms/implementations/bounded-unifont/unifont-17.0.04-bounded.hex";
const NAMES: &str = "../../plots/birth/names/catalog.mjs";
const FONTS: &[(&str, &str)] = &[
    (
        "DejaVuSans.ttf",
        "7da195a74c55bef988d0d48f9508bd5d849425c1770dba5d7bfc6ce9ed848954",
    ),
    (
        "DejaVuSerif.ttf",
        "42d1edeb7952f31b1f96d767ed7030b08a39e0c372b0071641518864e2bffb51",
    ),
    (
        "DejaVuSansMono.ttf",
        "b4a6c3e4faab8773f4ff761d56451646409f29abedd68f05d38c2df667d3c582",
    ),
];
// Fixed profile index: label, body, heading, title, code. These are Presenter
// resources, never part of a portable Face or authored plot identity.
const PROFILES: &[(usize, u8)] = &[(0, 14), (0, 16), (0, 20), (1, 24), (2, 14)];

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
    let mut ledger = String::from(
        "profile\tfont\tsize\tglyphs\tfallbacks\tcoverage_bytes\tascent\tline_height\tleft_ink\tright_overhang\n",
    );
    for (profile, &(front, size)) in PROFILES.iter().enumerate() {
        let font = &fonts[front];
        let line = font
            .horizontal_line_metrics(f32::from(size))
            .expect("horizontal font metrics");
        // Rescue outlines are baseline-14 with a sixteen-pixel height.
        // Include selected and Sans-fallback ink, not only font-wide metadata.
        let mut ascent = (line.ascent.ceil() as i16).max(14);
        let mut descent = (line.new_line_size.ceil() as i16 - ascent).max(2);
        let (mut glyph_count, mut fallbacks) = (0usize, 0usize);
        let coverage_start = coverage.len();
        let (mut left_ink, mut right_overhang) = (0i16, 0i16);
        for &character in &repertoire {
            // The pinned UI front supplies scalars absent from the selected front.
            // Keep the code profile's cell advance even for fallback outlines.
            let raster_font = if font.lookup_glyph_index(character) == 0 {
                &fonts[0]
            } else {
                font
            };
            // Absent scalars remain explicit at lookup: the native runtime may
            // choose its pinned fallback, never the font parser's implicit .notdef.
            if raster_font.lookup_glyph_index(character) == 0 {
                continue;
            }
            let (metrics, bitmap) = raster_font.rasterize(character, f32::from(size));
            let advance = if front == 2 && metrics.advance_width != 0.0 {
                font.metrics('M', f32::from(size)).advance_width
            } else {
                metrics.advance_width
            };
            assert!(metrics.width <= MAX_GLYPH_EDGE && metrics.height <= MAX_GLYPH_EDGE);
            assert_eq!(bitmap.len(), metrics.width * metrics.height);
            assert!(
                coverage.len() + bitmap.len() <= MAX_COVERAGE_BYTES,
                "coverage exceeds admission"
            );
            ascent = ascent.max(metrics.ymin as i16 + metrics.height as i16);
            descent = descent.max(-(metrics.ymin as i16));
            left_ink = left_ink.max(-(metrics.xmin as i16));
            right_overhang = right_overhang
                .max(metrics.xmin as i16 + metrics.width as i16 - advance.floor() as i16);
            glyph_count += 1;
            fallbacks += usize::from(!core::ptr::eq(raster_font, font));
            let offset = coverage.len();
            coverage.extend_from_slice(&bitmap);
            writeln!(records, "Glyph {{ profile: {profile}, codepoint: {}, x: {}, y: {}, width: {}, height: {}, advance: {}, offset: {offset} }},",
                u32::from(character), metrics.xmin, metrics.ymin, metrics.width, metrics.height,
                (advance * 64.0).round() as u16).unwrap();
        }
        // One pixel covers independent floor(bearing)/ceil(extent) rounding
        // at all admitted 100..=200 percent scales. Keep advertised font lead.
        let line_height = (line.new_line_size.ceil() as u16).max((ascent + descent + 1) as u16);
        writeln!(
            profiles,
            "ProfileMetrics {{ ascent: {ascent}, line_height: {line_height} }},"
        )
        .unwrap();
        writeln!(ledger, "{profile}\t{}\t{size}\t{glyph_count}\t{fallbacks}\t{}\t{ascent}\t{line_height}\t{left_ink}\t{right_overhang}", FONTS[front].0, coverage.len() - coverage_start).unwrap();
    }
    records.push_str("];\n");
    profiles.push_str("];\n");
    records.push_str(&profiles);
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo OUT_DIR"));
    fs::write(output.join("native_typography_resources.tsv"), ledger)
        .expect("fixed resource ledger");
    fs::write(output.join("native_typography.rs"), records).expect("fixed glyph records");
    fs::write(output.join("native_coverage.bin"), coverage).expect("fixed coverage table");
}
