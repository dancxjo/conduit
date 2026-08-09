use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

const SUBSET_PATH: &str = "assets/unifont/unifont-17.0.04-patchbay.hex";

fn main() {
    println!("cargo:rerun-if-changed={SUBSET_PATH}");
    let source = fs::read_to_string(SUBSET_PATH).expect("read pinned Unifont subset");
    let mut generated = String::from("static GLYPHS: &[GlyphRecord] = &[\n");
    let mut previous = None;
    let mut required = [false; 8];
    let mut glyph_count = 0usize;

    for (line_index, line) in source.lines().enumerate() {
        let (codepoint, bitmap) = line
            .split_once(':')
            .unwrap_or_else(|| panic!("{SUBSET_PATH}:{}: missing ':'", line_index + 1));
        assert!(
            (4..=6).contains(&codepoint.len())
                && codepoint.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "{SUBSET_PATH}:{}: invalid codepoint",
            line_index + 1
        );
        let codepoint = u32::from_str_radix(codepoint, 16).expect("validated codepoint");
        assert!(
            char::from_u32(codepoint).is_some(),
            "{SUBSET_PATH}:{}: non-scalar codepoint",
            line_index + 1
        );
        assert!(
            previous.is_none_or(|value| codepoint > value),
            "{SUBSET_PATH}:{}: glyphs must be sorted and unique",
            line_index + 1
        );
        previous = Some(codepoint);
        glyph_count += 1;
        assert!(
            bitmap.len() == 32 || bitmap.len() == 64,
            "{SUBSET_PATH}:{}: glyph must be 8x16 or 16x16",
            line_index + 1
        );
        assert!(
            bitmap.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "{SUBSET_PATH}:{}: invalid bitmap hex",
            line_index + 1
        );

        let mut bytes = [0u8; 32];
        for (index, pair) in bitmap.as_bytes().chunks_exact(2).enumerate() {
            bytes[index] = (nibble(pair[0]) << 4) | nibble(pair[1]);
        }
        let width = if bitmap.len() == 32 { 8 } else { 16 };
        write!(
            generated,
            "GlyphRecord {{ codepoint: 0x{codepoint:04X}, width: {width}, bitmap: ["
        )
        .unwrap();
        for byte in bytes {
            write!(generated, "0x{byte:02X},").unwrap();
        }
        generated.push_str("] },\n");

        for (present, wanted) in required.iter_mut().zip([
            0x0041, 0x00E9, 0x03A9, 0x0416, 0x2192, 0x2500, 0x4E2D, 0xFFFD,
        ]) {
            *present |= codepoint == wanted;
        }
    }
    assert!(
        glyph_count <= 1_024,
        "subset exceeds the admitted 1,024-glyph bound"
    );
    assert!(
        required.into_iter().all(|present| present),
        "subset is missing an acceptance-script glyph"
    );
    generated.push_str("];\n");

    let output = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    fs::write(output.join("unifont_subset.rs"), generated).expect("write generated glyph table");
}

fn nibble(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        b'A'..=b'F' => byte - b'A' + 10,
        _ => unreachable!("validated hex nibble"),
    }
}
