#![cfg(feature = "parser-model")]
//! Model-free instrumentation of the SAME input already used by the frozen V3
//! causal negative. This is not a new learned protocol or a calibration result.
#[path = "common/parser_kernel.rs"]
mod parser_kernel;
#[path = "common/parser_selection_io.rs"]
mod parser_selection_io;
#[path = "common/parser_selection_values.rs"]
mod parser_selection_values;
use conduit_language::{lexical::*, *};
use parser_selection_io::SourceGraph;
use parser_selection_values::{profile, revision_with_text};

#[test]
fn frozen_causal_word_boundaries_admit_complete_travis_before_terminal_period() {
    let protocol = [
        ("Hello ", 5),
        ("Hello, ", 6),
        ("Hello, Travis ", 13),
        ("Hello, Travis.", 14),
    ];
    let mut previous: Option<PreparedLexicalTape> = None;
    let mut availability = SourceGraph::language("language-parser-availability");
    for (index, (text, stable)) in protocol.into_iter().enumerate() {
        let old = previous.as_ref();
        let source = revision_with_text(
            text,
            "word-arrival-same-input-proof",
            &format!("word-arrival/{index}"),
            index as u64,
            old.map(|p| p.tape().source()),
            Some(stable),
            LanguageTextFinality::Partial,
        );
        validate_text_revision(old.map(|p| p.tape().source()), &source, 0, 4096).unwrap();
        let lexical = prepare_lexical_tape(&source, &profile(), old).unwrap();
        assert_eq!(lexical.tape().tokens().len(), index + 1);
        for (ordinal, token) in lexical.tape().tokens().iter().enumerate() {
            let (start, end, surface) = [
                (0, 5, "Hello"),
                (5, 6, ","),
                (7, 13, "Travis"),
                (13, 14, "."),
            ][ordinal];
            assert_eq!(
                (
                    *token.span().start(),
                    *token.span().end(),
                    token.surface().as_str()
                ),
                (start, end, surface)
            );
            assert_eq!(
                token.identity().text_revision(),
                source.material().revision()
            );
            assert!(matches!(
                token.completeness(),
                LanguageLexicalCompleteness::Complete
            ));
            assert!(!token.candidates().is_empty());
            if let Some(prior) = old.and_then(|p| p.tape().tokens().iter().nth(ordinal)) {
                assert_eq!(token.prior_occurrence().as_ref(), Some(prior.identity()));
                assert_eq!(token.candidates(), prior.candidates());
            }
        }
        let input = LanguageParserAvailableLexical::new(lexical.tape().clone(), (index + 1) as u64)
            .unwrap();
        let actual: LanguageParserRawAvailability = availability.run_native(input.clone());
        assert_eq!(actual.lexical(), &input);
        assert!(!actual.waiting());
        assert!(!actual.final_input());
        previous = Some(lexical);
    }
    assert_eq!(availability.invocations(), 4);
    availability.close();
}

#[test]
fn actual_character_prefix_is_distinct_and_keeps_trailing_word_unavailable() {
    let source = revision_with_text(
        "Hello, Travis",
        "character-prefix-control",
        "character-prefix/0",
        0,
        None,
        Some(13),
        LanguageTextFinality::Partial,
    );
    let lexical = prepare_lexical_tape(&source, &profile(), None).unwrap();
    let trailing = &lexical.tape().tokens()[2];
    assert_eq!((*trailing.span().start(), *trailing.span().end()), (7, 13));
    assert!(matches!(
        trailing.completeness(),
        LanguageLexicalCompleteness::TrailingPartial
    ));
    assert!(trailing.candidates().is_empty());
    assert!(LanguageParserAvailableLexical::new(lexical.tape().clone(), 3).is_err());
    assert!(LanguageParserAvailableLexical::new(lexical.tape().clone(), 2).is_ok());
}
