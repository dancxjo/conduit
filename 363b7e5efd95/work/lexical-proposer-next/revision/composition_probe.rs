extern crate alloc;
#[path = "lexical_canonical_sequence.rs"]
mod lexical_canonical_sequence;
#[path = "parser_canonical_composition.rs"]
mod parser_canonical_composition;
#[path = "parser_canonical_nominal.rs"]
mod parser_canonical_nominal;
#[path = "parser_canonical_schema.rs"]
mod parser_canonical_schema;
use conduit_core::validate_canonical_structured_value as view;
use conduit_language::*;
use conduit_plot::rust_binding::*;
use lexical_canonical_sequence::*;
use parser_canonical_composition::*;
fn family() -> PreparedNativeFamily {
    PreparedNativeFamily::prepare(
        &[
            LanguageLexicalTokenProposal::PREPARED_DESCRIPTOR,
            LanguageLexicalProposedTape::PREPARED_DESCRIPTOR,
        ],
        PreparedNativeFamilyLimits {
            maximum_types: 64,
            maximum_laws_per_type: 256,
            maximum_input_bytes: 262144,
            maximum_retained_bytes: 300_000_000,
            maximum_preparation_peak_bytes: 1_000_000_000,
            maximum_conversion_requested_bytes: 1_000_000_000,
        },
    )
    .unwrap()
}
fn limits() -> ParserCompositionLimits {
    ParserCompositionLimits {
        maximum_output_bytes: 262144,
        maximum_preparation_requested_bytes: 10_000_000,
        maximum_retained_requested_bytes: 10_000_000,
    }
}
#[test]
fn exact_variant_payload_and_bounded_candidate_sequence_parity() {
    let family = family();
    let mut unit = PreparedParserCanonicalComposer::prepare_field::<LanguageLexicalToken>(
        &family,
        &["category", "word"],
        limits(),
    )
    .unwrap();
    let unit = unit.leaf(&[]).unwrap();
    let mut category = PreparedParserCanonicalComposer::prepare_field::<LanguageLexicalToken>(
        &family,
        &["category"],
        limits(),
    )
    .unwrap();
    let actual = category.variant("word", view(unit).unwrap()).unwrap();
    assert_eq!(actual, LinguisticTokenCategory::Word.encode().unwrap());
    let candidate = LanguageLexicalCandidate::new(
        "<unknown>".into(),
        BoundedSequence::new(),
        LanguageLexicalPos::Noun,
    )
    .unwrap();
    let candidate_bytes = candidate.clone().encode().unwrap();
    let mut sequence = PreparedLexicalCanonicalSequence::prepare::<LanguageLexicalToken>(
        &family,
        &["candidates"],
        262144,
        262144,
        262144,
    )
    .unwrap();
    let actual = sequence
        .compose([view(&candidate_bytes).unwrap()].into_iter())
        .unwrap();
    let token = LanguageLexicalToken::new(
        BoundedSequence::try_from_iter([candidate]).unwrap(),
        LinguisticTokenCategory::Word,
        LanguageLexicalCompleteness::Complete,
        LinguisticTokenIdentity::new(
            0,
            LanguageTextId::new("t".into()).unwrap(),
            LanguageTextRevisionId::new("r".into()).unwrap(),
        )
        .unwrap(),
        None,
        TextSpan::new(
            LinguisticOffsetBasis::UnicodeScalar,
            1,
            0,
            LanguageTextId::new("t".into()).unwrap(),
            LanguageTextRevisionId::new("r".into()).unwrap(),
        )
        .unwrap(),
        "x".into(),
    )
    .unwrap();
    let token_bytes = token.encode().unwrap();
    let expected = view(&token_bytes)
        .unwrap()
        .record_field("candidates")
        .unwrap()
        .unwrap();
    let mut expected_bytes = expected.type_bytes().to_vec();
    expected_bytes.extend_from_slice(expected.value_node());
    assert_eq!(actual, expected_bytes);
    assert!(matches!(
        sequence.compose([view(unit).unwrap()].into_iter()),
        Err(SequenceRefusal::Type)
    ));
    assert!(matches!(
        sequence.compose([view(&candidate_bytes).unwrap(); 5].into_iter()),
        Err(SequenceRefusal::Length)
    ));
    assert!(
        PreparedLexicalCanonicalSequence::prepare::<LanguageLexicalToken>(
            &family,
            &["candidates"],
            262144,
            262143,
            262144
        )
        .is_err()
    );
    assert_eq!(sequence.retained_capacity_bytes(), 262144);
}
