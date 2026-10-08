use conduit_plot::{source_span, QuotedTextSourceMap, SpannedText};
fn token(source: &str, literal: &str) -> SpannedText {
    let start = source.find(literal).unwrap();
    SpannedText {
        text: literal.into(),
        span: source_span(source, start..start + literal.len()).unwrap(),
    }
}
#[test]
fn decoded_unicode_and_escapes_map_to_exact_authored_ranges() {
    let literal = r#""t͡ʃ\t\"\\\nã""#;
    let source = format!("# 雪\nvalue = {literal}\n");
    let map = QuotedTextSourceMap::new(&source, &token(&source, literal), 64).unwrap();
    assert_eq!(map.decoded(), "t͡ʃ\t\"\\\nã");
    for (range, raw) in [
        (0..5, "t͡ʃ"),
        (5..6, "\\t"),
        (6..7, "\\\""),
        (7..8, "\\\\"),
        (8..9, "\\n"),
        (9..11, "ã"),
    ] {
        let span = map.source_span(range).unwrap();
        assert_eq!(&source[span.start..span.end], raw);
        assert_eq!(span.line, 2);
        assert_eq!(span.end_line, 2);
        let prefix = source[..span.start].rsplit('\n').next().unwrap();
        assert_eq!(span.column, prefix.chars().count() + 1);
        assert_eq!(span.end_column, span.column + raw.chars().count());
    }
    assert!(
        map.source_span(0..2).is_none(),
        "cannot split a combining scalar's UTF-8 bytes"
    );
    assert!(map.source_span(12..12).is_none());
    let end = map.source_span(11..11).unwrap();
    assert_eq!(end.start, end.end);
    assert_eq!(&source[end.start..end.start + 1], "\"");
}
#[test]
fn bounded_mapping_refuses_foreign_tokens_bad_escapes_and_oversize() {
    for literal in [r#""\u1234""#, r#""a"b""#, "\"unfinished", r#""trailing\"#] {
        assert!(
            QuotedTextSourceMap::new(literal, &token(literal, literal), 64).is_none(),
            "{literal}"
        );
    }
    let source = "\"ã\"";
    assert!(QuotedTextSourceMap::new(source, &token(source, source), 1).is_none());
    let mut foreign = token(source, source);
    foreign.text = "\"a\"".into();
    assert!(QuotedTextSourceMap::new(source, &foreign, 64).is_none());
    let empty = "\"\"";
    let map = QuotedTextSourceMap::new(empty, &token(empty, empty), 0).unwrap();
    assert_eq!(map.source_span(0..0).unwrap().start, 1);
}
