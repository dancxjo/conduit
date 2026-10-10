use super::*;

#[test]
fn source_bytes_refuse_before_constructing_an_expression() {
    assert!(parse_text_pattern(&"a".repeat(MAXIMUM_TEXT_PATTERN_SOURCE_BYTES)).is_ok());
    assert_eq!(
        parse_text_pattern(&"a".repeat(MAXIMUM_TEXT_PATTERN_SOURCE_BYTES + 1)),
        Err(TextPatternSourceError::SourceLimitExceeded)
    );
    let unicode = "ʃ".repeat(MAXIMUM_TEXT_PATTERN_SOURCE_BYTES / 2);
    assert!(parse_text_pattern(&unicode).is_ok());
    assert_eq!(
        parse_text_pattern(&(unicode + "ʃ")),
        Err(TextPatternSourceError::SourceLimitExceeded)
    );
}

#[test]
fn groups_refuse_at_the_shared_depth_ceiling() {
    let nested = |depth: usize| "(".repeat(depth) + "a" + &")".repeat(depth);
    let admitted = parse_text_pattern(&nested(MAXIMUM_TEXT_PATTERN_SOURCE_DEPTH)).unwrap();
    assert!(admitted.compile(1).unwrap().is_match("a"));
    assert_eq!(
        parse_text_pattern(&nested(MAXIMUM_TEXT_PATTERN_SOURCE_DEPTH + 1)),
        Err(TextPatternSourceError::DepthLimitExceeded {
            offset: MAXIMUM_TEXT_PATTERN_SOURCE_DEPTH
        })
    );
}

#[test]
fn sequential_assertions_share_the_group_recursion_budget() {
    let source = "(?=a)".repeat(MAXIMUM_TEXT_PATTERN_SOURCE_DEPTH) + "a";
    assert!(parse_text_pattern(&source).is_ok());
    let excess = "(?=a)".repeat(MAXIMUM_TEXT_PATTERN_SOURCE_DEPTH + 1) + "a";
    assert_eq!(
        parse_text_pattern(&excess),
        Err(TextPatternSourceError::DepthLimitExceeded {
            offset: MAXIMUM_TEXT_PATTERN_SOURCE_DEPTH * "(?=a)".len()
        })
    );
    let group = "(".repeat(MAXIMUM_TEXT_PATTERN_SOURCE_DEPTH)
        + "a"
        + &")".repeat(MAXIMUM_TEXT_PATTERN_SOURCE_DEPTH);
    assert!(matches!(
        parse_text_pattern(&(String::from("(?=a)") + &group)),
        Err(TextPatternSourceError::DepthLimitExceeded { .. })
    ));
}

#[test]
fn nested_assertion_diagnostics_keep_original_payload_offsets() {
    assert_eq!(
        parse_text_pattern("(?=^)b"),
        Err(TextPatternSourceError::Unexpected {
            offset: 3,
            character: Some('^')
        })
    );
    assert_eq!(
        parse_text_pattern("(?=a)^"),
        Err(TextPatternSourceError::Unexpected {
            offset: 5,
            character: Some('^')
        })
    );
    assert_eq!(
        parse_text_pattern("(?=ʃ)^"),
        Err(TextPatternSourceError::Unexpected {
            offset: 6,
            character: Some('^')
        })
    );
}
