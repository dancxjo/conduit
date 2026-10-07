use conduit_language::{lexical::prepare_lexical_tape, *};
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
#[path = "common/parser_fixture.rs"]
mod fixture;
#[path = "common/parser_kernel.rs"]
mod parser_kernel;

fn source() -> String {
    [
        fixture::parser_source(),
        include_str!("../text_revision.conduit").into(),
        include_str!("../lexical.conduit").into(),
        include_str!("../parser_joint.conduit").into(),
        include_str!("../parser_available.conduit").into(),
        include_str!("../parser_scorer_v2.conduit").into(),
        include_str!("../parser_session.conduit").into(),
    ]
    .join("\n")
}
fn lexical() -> LanguageParserJointLexical {
    lexical_with_pos(&[LanguageLexicalPos::Noun, LanguageLexicalPos::Verb])
}
fn lexical_with_pos(pos: &[LanguageLexicalPos]) -> LanguageParserJointLexical {
    let provenance = LinguisticDerivationProvenance::deterministic_rule(
        "fixture/joint".into(),
        "profile/1".into(),
    )
    .unwrap();
    let language = LanguageId::new("language/en".into()).unwrap();
    let source = LanguageTextRevision::new(
        LanguageTextFinality::Final,
        LanguageText::new(
            LanguageTextId::new("utterance".into()).unwrap(),
            language.clone(),
            LanguageTextRevisionId::new("source/1".into()).unwrap(),
            "record".into(),
        )
        .unwrap(),
        None,
        provenance.clone(),
        0,
        None,
    )
    .unwrap();
    let entries = BoundedSequence::try_from_iter([LanguageLexicalEntry::new(
        BoundedSequence::try_from_iter(pos.iter().cloned().map(|pos| {
            LanguageLexicalCandidate::new("record".into(), BoundedSequence::new(), pos).unwrap()
        }))
        .unwrap(),
        "record".into(),
    )
    .unwrap()])
    .unwrap();
    let profile =
        LanguageLexicalProfile::new(entries, "fixture/joint".into(), language, provenance).unwrap();
    let tape = prepare_lexical_tape(&source, &profile, None).unwrap();
    LanguageParserJointLexical::new(tape.tape().clone(), 1).unwrap()
}
fn state() -> LanguageParserState {
    let basis = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("analysis/1".into()).unwrap(),
        LanguageTextRevisionId::new("source/1".into()).unwrap(),
        LanguageTextId::new("utterance".into()).unwrap(),
    )
    .unwrap();
    let relation = LanguageParserRelation::new(
        LanguageUniversalDependencyRelation::Dep,
        LanguageParserSubtype::new("".into()).unwrap(),
    )
    .unwrap();
    LanguageParserState::new(
        basis,
        0,
        1,
        [5, 5, 5, 5, 4],
        relation.clone(),
        relation.clone(),
        relation.clone(),
        relation,
        [4, 4, 4, 4, 4],
        1,
        0,
    )
    .unwrap()
}

fn hypothesis(
    state: &LanguageParserState,
    active: bool,
    choice: u64,
) -> LanguageParserJointHypothesis {
    LanguageParserJointHypothesis::new(
        [choice, 0, 0, 0],
        LanguageParserHypothesis::new(active, 0, 0, state.clone()).unwrap(),
    )
    .unwrap()
}
#[test]
fn source_consensus_retains_lexical_disagreement_and_unassigned_abstention() {
    let lexical = lexical();
    let initial = state();
    let root = LanguageParserRelation::new(
        LanguageUniversalDependencyRelation::Root,
        LanguageParserSubtype::new("".into()).unwrap(),
    )
    .unwrap();
    let complete = LanguageParserState::new(
        initial.basis().clone(),
        0,
        1,
        [4, 5, 5, 5, 4],
        root,
        initial.relation1().clone(),
        initial.relation2().clone(),
        initial.relation3().clone(),
        [4, 4, 4, 4, 4],
        1,
        1,
    )
    .unwrap();
    let mut execution =
        parser_kernel::Execution::prepare(source(), "language-parser-joint-consensus");
    execution.kernel.start().unwrap();
    for (invocation, assigned, alternative, expected) in [
        (0, false, 0, false),
        (1, true, 0, true),
        (2, true, 1, false),
        (3, true, 0, true),
    ] {
        let current = if assigned { &complete } else { &initial };
        let a = hypothesis(current, true, 0);
        let b = hypothesis(current, true, alternative);
        let inactive = hypothesis(current, false, 0);
        let beam = LanguageParserJointBeam::new(
            current.basis().clone(),
            a,
            b,
            inactive.clone(),
            inactive,
            0,
            invocation,
            lexical.clone(),
        )
        .unwrap();
        let query = LanguageParserJointConsensusQuery::new(beam, 0).unwrap();
        let result = execution.transact(invocation, &query.into_structured().unwrap());
        let observation = LanguageParserJointConsensusObservation::from_structured(result).unwrap();
        assert_eq!(*observation.agreed(), expected);
    }
}
#[test]
#[ignore = "inspect retained native evidence without decoding new graphs"]
fn inspect_retained_native_candidates() {
    let rows: Vec<serde_json::Value> = serde_json::from_slice(
        &std::fs::read(std::env::var("CONDUIT_PARSER_STREAM_INSPECT").unwrap()).unwrap(),
    )
    .unwrap();
    for row in rows {
        let bytes: Vec<u8> = serde_json::from_value(row["session"]["beam_bytes"].clone()).unwrap();
        let value = conduit_core::StructuredInfoValue::from_canonical_bytes(&bytes).unwrap();
        let runtime = LanguageParserJointRuntimeBeam::from_structured(value).unwrap();
        let beam = runtime.beam();
        println!("{}", row["id"]);
        for candidate in [
            beam.candidate0(),
            beam.candidate1(),
            beam.candidate2(),
            beam.candidate3(),
        ] {
            println!(
                "active={} score={} choices={:?} heads={:?} depth={} unread={}",
                candidate.parser().active(),
                candidate.parser().score(),
                candidate.choices(),
                candidate.parser().state().heads(),
                candidate.parser().state().depth(),
                candidate.parser().state().unread()
            );
        }
    }
}
