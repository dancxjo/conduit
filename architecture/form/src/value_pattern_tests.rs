use crate::{
    parse_text_pattern, TextPatternDefinitionError, TextPatternExpression, TextPatternSourceError,
};
use alloc::{boxed::Box, vec};
use conduit_core::{MAX_PATTERN_MATCH_STEPS, MAX_PATTERN_STATES};

fn scalar(character: char) -> TextPatternExpression {
    TextPatternExpression::ScalarRange {
        first: character as u32,
        last: character as u32,
    }
}

#[test]
fn canonical_bounded_regex_source_compiles_before_play() {
    let pattern = parse_text_pattern("[A-Z]{2}[0-9]{4}")
        .unwrap()
        .compile(16)
        .unwrap();
    assert!(pattern.is_match("AB1234"));
    assert!(!pattern.is_match("Ab1234"));
    assert!(!pattern.is_match("AB123"));
}

#[test]
fn noncapturing_and_named_groups_compile_to_the_same_boolean_language() {
    let ordinary = parse_text_pattern("(AB){2}").unwrap().compile(8).unwrap();
    let noncapturing = parse_text_pattern("(?:AB){2}").unwrap().compile(8).unwrap();
    let named = parse_text_pattern("(?<code>AB){2}")
        .unwrap()
        .compile(8)
        .unwrap();
    assert_eq!(ordinary, noncapturing);
    assert_eq!(ordinary, named);
    assert!(named.is_match("ABAB"));
}

#[test]
fn leading_positive_and_negative_lookahead_compile_to_finite_languages() {
    let positive = parse_text_pattern("(?=AB)A.").unwrap().compile(8).unwrap();
    assert!(positive.is_match("AB"));
    assert!(!positive.is_match("AC"));

    let negative = parse_text_pattern("(?!AB)A.").unwrap().compile(8).unwrap();
    assert!(!negative.is_match("AB"));
    assert!(negative.is_match("AC"));

    let shorter_assertion = parse_text_pattern("(?=A)AB").unwrap().compile(8).unwrap();
    assert!(shorter_assertion.is_match("AB"));
    assert!(!shorter_assertion.is_match("AC"));
}

#[test]
fn source_refuses_malformed_constructs() {
    assert!(matches!(
        parse_text_pattern("[Z-A]"),
        Err(TextPatternSourceError::InvalidRange { .. })
    ));
    assert!(matches!(
        parse_text_pattern("a{2,}"),
        Err(TextPatternSourceError::InvalidRepeat { .. })
    ));
    assert!(matches!(
        parse_text_pattern("^a$"),
        Err(TextPatternSourceError::Unexpected { .. })
    ));
    assert!(matches!(
        parse_text_pattern(r"\d{2}"),
        Err(TextPatternSourceError::Unexpected { .. })
    ));
    assert!(matches!(
        parse_text_pattern("(?=AB)"),
        Err(TextPatternSourceError::Unexpected { .. })
    ));
    assert!(matches!(
        parse_text_pattern("A(?=B)B"),
        Err(TextPatternSourceError::Unexpected { .. })
    ));
}

#[test]
fn star_and_plus_are_finitely_bound_by_the_input_contract() {
    let star = parse_text_pattern("a*").unwrap().compile(8).unwrap();
    assert!(star.is_match(""));
    assert!(star.is_match("aaaaaaaa"));
    assert!(!star.is_match("aaaaaaaaa"));
    let plus = parse_text_pattern("[A-Z]+").unwrap().compile(8).unwrap();
    assert!(!plus.is_match(""));
    assert!(plus.is_match("ABC"));
    assert!(!plus.is_match("ABC1"));
}

#[test]
fn sequence_choice_and_finite_repeat_compile_to_full_match() {
    let expression = TextPatternExpression::Sequence(vec![
        scalar('a'),
        TextPatternExpression::Repeat {
            expression: Box::new(TextPatternExpression::Choice(vec![
                scalar('b'),
                scalar('c'),
            ])),
            minimum: 1,
            maximum: 2,
        },
    ]);
    let pattern = expression.compile(3).unwrap();
    assert!(pattern.is_match("ab"));
    assert!(pattern.is_match("acc"));
    assert!(pattern.is_match("abc"));
    assert!(!pattern.is_match("a"));
    assert!(!pattern.is_match("abbb"));
}

#[test]
fn empty_forms_have_exact_regular_language_meanings() {
    assert!(TextPatternExpression::Empty
        .compile(0)
        .unwrap()
        .is_match(""));
    assert!(TextPatternExpression::Sequence(vec![])
        .compile(0)
        .unwrap()
        .is_match(""));
    let never = TextPatternExpression::Choice(vec![]).compile(1).unwrap();
    assert!(!never.is_match(""));
    assert!(!never.is_match("a"));
}

#[test]
fn unicode_range_crossing_surrogates_is_split_canonically() {
    let pattern = TextPatternExpression::ScalarRange {
        first: 0xd7ff,
        last: 0xe000,
    }
    .compile(1)
    .unwrap();
    assert!(pattern.is_match("\u{d7ff}"));
    assert!(pattern.is_match("\u{e000}"));
    assert_eq!(pattern.states[0].transitions.len(), 2);
}

#[test]
fn overlapping_choices_are_determinized_into_disjoint_ranges() {
    let pattern = TextPatternExpression::Choice(vec![
        TextPatternExpression::ScalarRange {
            first: 'a' as u32,
            last: 'm' as u32,
        },
        TextPatternExpression::ScalarRange {
            first: 'h' as u32,
            last: 'z' as u32,
        },
    ])
    .compile(1)
    .unwrap();
    assert!(pattern.is_match("a"));
    assert!(pattern.is_match("j"));
    assert!(pattern.is_match("z"));
    assert!(!pattern.is_match("0"));
    for pair in pattern.states[0].transitions.windows(2) {
        assert!(pair[0].last_scalar < pair[1].first_scalar);
    }
}

#[test]
fn malformed_source_has_exact_errors() {
    assert_eq!(
        TextPatternExpression::ScalarRange {
            first: 0xd800,
            last: 0xd800
        }
        .compile(1),
        Err(TextPatternDefinitionError::InvalidScalarRange {
            first: 0xd800,
            last: 0xd800
        })
    );
    assert_eq!(
        TextPatternExpression::Repeat {
            expression: Box::new(scalar('x')),
            minimum: 2,
            maximum: 1
        }
        .compile(2),
        Err(TextPatternDefinitionError::InvalidRepeatRange {
            minimum: 2,
            maximum: 1
        })
    );
}

#[test]
fn admitted_match_work_is_refused_before_play() {
    assert_eq!(
        scalar('x').compile(MAX_PATTERN_MATCH_STEPS + 1),
        Err(TextPatternDefinitionError::MatchWorkExceeded)
    );
}

#[test]
fn compiled_state_ceiling_is_refused_exactly() {
    let expression =
        TextPatternExpression::Sequence((0..MAX_PATTERN_STATES).map(|_| scalar('x')).collect());
    assert_eq!(
        expression.compile(MAX_PATTERN_STATES as u32),
        Err(TextPatternDefinitionError::TooManyStates)
    );
}
