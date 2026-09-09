//! Build-time structural admission of immutable graphical resources.
pub fn validate() {
    if std::env::var_os("CARGO_FEATURE_NATIVE_COMPOSITOR").is_none() {
        return;
    }
    for name in ["body", "heading", "title", "code"] {
        let path = format!("assets/graphical/{name}.atlas");
        println!("cargo:rerun-if-changed={path}");
        let bytes = std::fs::read(&path).expect("required graphical profile atlas is missing");
        let word = |offset: usize| {
            u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize
        };
        let count = word(0);
        assert!((1..=1024).contains(&count), "finite atlas glyph bound");
        let start = 4 + count * 12;
        let (mut previous, mut expected) = (None, 0);
        for index in 0..count {
            let record = 4 + index * 12;
            let codepoint = word(record);
            assert!(
                previous.is_none_or(|value| codepoint > value),
                "sorted unique atlas glyphs"
            );
            assert!(char::from_u32(codepoint as u32).is_some());
            previous = Some(codepoint);
            let width = usize::from(bytes[record + 6]);
            let height = usize::from(bytes[record + 7]);
            assert!((1..=64).contains(&width) && (1..=40).contains(&height));
            assert!(bytes[record + 4] <= 64);
            assert_eq!(word(record + 8), expected, "canonical coverage offset");
            expected += width * height;
        }
        assert_eq!(
            start + expected,
            bytes.len(),
            "exact finite coverage storage"
        );
        assert!(bytes.len() < 600_000, "per-face storage bound");
    }
}
