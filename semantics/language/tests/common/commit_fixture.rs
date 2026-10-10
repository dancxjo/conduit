//! Deterministic symbolic input: canonical lexical custody and actual parser transitions.
use super::fixture::*;
use conduit_core::*;
use conduit_language::{lexical::prepare_lexical_tape, *};
use conduit_plot::{rust_binding::*, CheckedSyntaxDocument};

pub fn query(checked: &CheckedSyntaxDocument) -> StructuredInfoValue {
    let ty = |name: &str| {
        &checked
            .native_types
            .iter()
            .find(|t| t.name == name)
            .unwrap()
            .value_type
    };
    let admit = |name: &str, value: &StructuredInfoValue| {
        let owner = checked
            .native_types
            .iter()
            .find(|t| t.name == name)
            .unwrap();
        assert_eq!(value.value_type(), &owner.value_type);
        validate_native_contracts(value, &owner.value_contracts).unwrap();
        validate_native_invariants(value, &owner.invariants).unwrap();
    };
    let provenance = LinguisticDerivationProvenance::deterministic_rule(
        "fixture/symbolic-commit".into(),
        "fixture/1".into(),
    )
    .unwrap();
    let language = LanguageId::new("language/en".into()).unwrap();
    let source = LanguageTextRevision::new(
        LanguageTextFinality::Final,
        LanguageText::new(
            LanguageTextId::new("utterance".into()).unwrap(),
            language.clone(),
            LanguageTextRevisionId::new("source/1".into()).unwrap(),
            "answer".into(),
        )
        .unwrap(),
        None,
        provenance.clone(),
        0,
        Some(6),
    )
    .unwrap();
    let candidate = LanguageLexicalCandidate::new(
        "answer".into(),
        BoundedSequence::new(),
        LanguageLexicalPos::Noun,
    )
    .unwrap();
    let entry = LanguageLexicalEntry::new(
        BoundedSequence::try_from_iter([candidate]).unwrap(),
        "answer".into(),
    )
    .unwrap();
    let profile = LanguageLexicalProfile::new(
        BoundedSequence::try_from_iter([entry]).unwrap(),
        "fixture/answer".into(),
        language,
        provenance,
    )
    .unwrap();
    let prepared = prepare_lexical_tape(&source, &profile, None).unwrap();
    let lexical = record(
        ty("LanguageParserJointLexical"),
        vec![
            ("tape", prepared.tape().clone().into_structured().unwrap()),
            (
                "token_count",
                number(
                    field_type(ty("LanguageParserJointLexical"), "token_count"),
                    1,
                ),
            ),
        ],
    );
    admit("LanguageParserJointLexical", &lexical);
    let mut f = Fixture::new();
    let initial = f.initial(1);
    let transitioned = f.step(&initial, "right_arc", "root", "analysis/1");
    assert!(accepted(&transitioned));
    let original_state = field(&transitioned, "state");
    // Construct the exact checked state from the original operation's fields,
    // then run its complete Native admission; no nominal identity is rewritten.
    let state_type = ty("LanguageParserState");
    let StructuredInfoTypeShape::Record { fields, .. } = state_type.shape() else {
        panic!("state")
    };
    let state = record(
        state_type,
        fields
            .iter()
            .map(|member| (member.name(), field(original_state, member.name()).clone()))
            .collect(),
    );
    LanguageParserState::from_structured(state.clone()).unwrap();
    assert_eq!(count(field(&state, "committed")), 0);
    let hypothesis_type = ty("LanguageParserHypothesis");
    let candidate = |active: bool, identity: u64| {
        let parser = record(
            hypothesis_type,
            vec![
                ("state", state.clone()),
                (
                    "active",
                    StructuredInfoValue::leaf(
                        field_type(hypothesis_type, "active").clone(),
                        vec![u8::from(active)],
                    )
                    .unwrap(),
                ),
                (
                    "identity",
                    number(field_type(hypothesis_type, "identity"), identity),
                ),
                (
                    "score",
                    StructuredInfoValue::leaf(
                        field_type(hypothesis_type, "score").clone(),
                        0_i64.to_le_bytes().to_vec(),
                    )
                    .unwrap(),
                ),
            ],
        );
        LanguageParserHypothesis::from_structured(parser.clone()).unwrap();
        let joint_type = ty("LanguageParserJointHypothesis");
        let choices_type = field_type(joint_type, "choices");
        let StructuredInfoTypeShape::Collection { element, .. } = choices_type.shape() else {
            panic!("choices")
        };
        let choices = StructuredInfoValue::collection(
            choices_type.clone(),
            (0..4).map(|_| number(element, 0)).collect(),
        )
        .unwrap();
        let joint = record(joint_type, vec![("parser", parser), ("choices", choices)]);
        admit("LanguageParserJointHypothesis", &joint);
        joint
    };
    let beam_type = ty("LanguageParserJointBeam");
    let beam = record(
        beam_type,
        vec![
            ("lexical", lexical),
            ("basis", field(&state, "basis").clone()),
            ("epoch", number(field_type(beam_type, "epoch"), 0)),
            ("invocation", number(field_type(beam_type, "invocation"), 1)),
            ("candidate0", candidate(true, 0)),
            ("candidate1", candidate(true, 1)),
            ("candidate2", candidate(false, 2)),
            ("candidate3", candidate(false, 3)),
        ],
    );
    admit("LanguageParserJointBeam", &beam);
    let consensus_type = ty("LanguageParserJointConsensusQuery");
    let consensus = record(
        consensus_type,
        vec![
            ("beam", beam),
            (
                "dependent",
                number(field_type(consensus_type, "dependent"), 0),
            ),
        ],
    );
    admit("LanguageParserJointConsensusQuery", &consensus);
    let fact = record(
        ty("LanguageParserJointStableFact"),
        vec![("query", consensus)],
    );
    admit("LanguageParserJointStableFact", &fact);
    let query = record(ty("LanguageParserJointCommitQuery"), vec![("fact", fact)]);
    admit("LanguageParserJointCommitQuery", &query);
    query
}
