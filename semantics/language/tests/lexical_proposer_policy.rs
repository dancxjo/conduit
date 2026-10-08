use conduit_core::{StructuredFieldValue, StructuredInfoValue};
use conduit_language::*;
use conduit_plot::rust_binding::{
    BoundedSequence, NativeRustBinding, PreparedNativeFamily, PreparedNativeFamilyLimits,
    PreparedNativeRustBinding,
};
fn provenance() -> LinguisticDerivationProvenance {
    LinguisticDerivationProvenance::deterministic_rule(
        "reviewed-proposal-policy".into(),
        "1".into(),
    )
    .unwrap()
}
fn definition() -> LanguageLexicalProposerDefinition {
    LanguageLexicalProposerDefinition::new(
        "dictionary-full-resource".into(),
        "proposer".into(),
        LanguageId::new("language/en".into()).unwrap(),
        provenance(),
        LanguageLexicalUnknownPolicy::new(
            BoundedSequence::try_from_iter([LanguageLexicalPos::Noun, LanguageLexicalPos::Verb])
                .unwrap(),
            LanguageLexicalUnknownCommitPolicy::StableInputConsensus,
            "unknown-policy".into(),
            provenance(),
        )
        .unwrap(),
    )
    .unwrap()
}
fn token(lemma: &str) -> LanguageLexicalToken {
    LanguageLexicalToken::new(
        BoundedSequence::try_from_iter(
            [LanguageLexicalPos::Noun, LanguageLexicalPos::Verb]
                .into_iter()
                .map(|pos| {
                    LanguageLexicalCandidate::new(lemma.into(), BoundedSequence::new(), pos)
                        .unwrap()
                }),
        )
        .unwrap(),
        LinguisticTokenCategory::Word,
        LanguageLexicalCompleteness::Complete,
        LinguisticTokenIdentity::new(
            0,
            LanguageTextId::new("original".into()).unwrap(),
            LanguageTextRevisionId::new("original-r0".into()).unwrap(),
        )
        .unwrap(),
        None,
        TextSpan::new(
            LinguisticOffsetBasis::UnicodeScalar,
            7,
            0,
            LanguageTextId::new("original".into()).unwrap(),
            LanguageTextRevisionId::new("original-r0".into()).unwrap(),
        )
        .unwrap(),
        "unlisted".into(),
    )
    .unwrap()
}
fn origin(id: &str) -> LanguageLexicalProposalOrigin {
    LanguageLexicalProposalOrigin::unknown(id.into(), provenance()).unwrap()
}
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
#[test]
fn exact_unknown_proposal_preserves_origin_and_refuses_laundered_lemma_or_policy() {
    let mut family = family();
    eprintln!("proposer_two_root_family={:?}", family.storage_receipt());
    assert_eq!(family.storage_receipt().types, 30);
    let query = LanguageLexicalProposalQuery::new(
        definition(),
        origin("unknown-policy"),
        None,
        token("<unknown>"),
    )
    .unwrap();
    let proposal = LanguageLexicalTokenProposal::new(query).unwrap();
    let bytes = proposal.clone().encode().unwrap();
    assert_eq!(
        family
            .decode::<LanguageLexicalTokenProposal>(&bytes)
            .unwrap(),
        proposal
    );
    assert!(matches!(
        proposal.query().origin(),
        LanguageLexicalProposalOrigin::Unknown(_)
    ));
    for (lemma, id) in [
        ("invented-known-lemma", "unknown-policy"),
        ("<unknown>", "foreign-policy"),
    ] {
        assert!(
            LanguageLexicalProposalQuery::new(definition(), origin(id), None, token(lemma))
                .is_err()
        );
        let original = proposal.query().clone().into_structured().unwrap();
        let conduit_core::StructuredInfoValueShape::Record(fields) = original.shape() else {
            panic!()
        };
        let none = fields
            .iter()
            .find(|f| f.name() == "reviewed_entry")
            .unwrap()
            .value()
            .clone();
        let value = StructuredInfoValue::record(
            LanguageLexicalProposalQuery::semantic_type().unwrap(),
            vec![
                StructuredFieldValue::new("definition", definition().into_structured().unwrap())
                    .unwrap(),
                StructuredFieldValue::new("origin", origin(id).into_structured().unwrap()).unwrap(),
                StructuredFieldValue::new("reviewed_entry", none).unwrap(),
                StructuredFieldValue::new("token", token(lemma).into_structured().unwrap())
                    .unwrap(),
            ],
        )
        .unwrap();
        let bytes = value.canonical_bytes().unwrap();
        assert!(LanguageLexicalProposalQuery::decode(&bytes).is_err());
        assert!(family
            .decode::<LanguageLexicalProposalQuery>(&bytes)
            .is_err());
    }
    assert!(LanguageLexicalUnknownPolicy::new(
        BoundedSequence::try_from_iter([LanguageLexicalPos::Noun, LanguageLexicalPos::Noun])
            .unwrap(),
        LanguageLexicalUnknownCommitPolicy::StableInputConsensus,
        "policy".into(),
        provenance()
    )
    .is_err());
}

#[test]
fn exact_checked_proposal_source_matches_prepared_execution_and_native_output() {
    let hex = include_str!(concat!(env!("OUT_DIR"), "/lexical_token_proposal.hex"));
    let raw: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect();
    let decode =
        conduit_plot::PortableExpressionProgram::canonical_decode_storage_bound(&raw).unwrap();
    let program = conduit_plot::PortableExpressionProgram::from_canonical_bytes_with_storage_limit(
        &raw, decode,
    )
    .unwrap();
    assert_eq!(
        program.input_type.canonical_bytes().unwrap(),
        LanguageLexicalProposalQuery::PREPARED_DESCRIPTOR.type_bytes
    );
    assert_eq!(
        program.output_type.canonical_bytes().unwrap(),
        LanguageLexicalTokenProposal::PREPARED_DESCRIPTOR.type_bytes
    );
    let (mut prepared, receipt) =
        conduit_plot::PreparedPortableExpressionEvaluator::new_with_storage_limits(
            &program,
            100_000_000,
            200_000_000,
            100_000_000,
        )
        .unwrap();
    let query = LanguageLexicalProposalQuery::new(
        definition(),
        origin("unknown-policy"),
        None,
        token("<unknown>"),
    )
    .unwrap();
    let input = query.clone().encode().unwrap();
    let mut family = family();
    assert_eq!(
        family
            .decode::<LanguageLexicalProposalQuery>(&input)
            .unwrap(),
        query
    );
    let reference = program.evaluate(&input).unwrap();
    let output = prepared.evaluate(&input).unwrap();
    assert_eq!(reference, output);
    let proposal: LanguageLexicalTokenProposal = family.decode(output).unwrap();
    assert_eq!(proposal.query(), &query);
    eprintln!("proposal_source_decode_bound={decode} prepared={receipt:?}");
    let foreign = definition().encode().unwrap();
    assert!(program.evaluate(&foreign).is_err());
    assert!(prepared.evaluate(&foreign).is_err());
    let mut malformed = input;
    malformed.push(0);
    assert!(program.evaluate(&malformed).is_err());
    assert!(prepared.evaluate(&malformed).is_err());
}
