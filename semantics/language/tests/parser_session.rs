use conduit_language::{lexical::prepare_lexical_tape, *};
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
#[path = "common/parser_fixture.rs"]
mod fixture;
#[path = "common/parser_joint_v2_fixture.rs"]
mod joint;
#[path = "common/parser_kernel.rs"]
mod parser_kernel;
#[path = "common/scorer_model.rs"]
mod scorer_model;

fn source() -> String {
    [
        fixture::parser_source(),
        include_str!("../text_revision.conduit").into(),
        include_str!("../lexical.conduit").into(),
        include_str!("../parser_joint.conduit").into(),
        include_str!("../parser_available.conduit").into(),
        include_str!("../parser_scorer_v2.conduit").into(),
        include_str!("../parser_session.conduit").into(),
        include_str!("../parser_joint_decode.conduit").into(),
        include_str!("../parser_session_policy.conduit").into(),
        include_str!("../parser_session_facts.conduit").into(),
        include_str!("../parser_session_commit.conduit").into(),
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

#[test]
fn source_score_band_preserves_boundaries_and_refuses_forged_maximum() {
    let lexical = lexical();
    let initial = state();
    let candidate = |score, active| {
        LanguageParserJointHypothesis::new(
            [0; 4],
            LanguageParserHypothesis::new(active, 0, score, initial.clone()).unwrap(),
        )
        .unwrap()
    };
    let beam = LanguageParserJointBeam::new(
        initial.basis().clone(),
        candidate(13216, true),
        candidate(12238, true),
        candidate(12216, true),
        candidate(12215, true),
        0,
        0,
        lexical,
    )
    .unwrap();
    let runtime = LanguageParserJointRuntimeBeam::new(beam.clone(), [0; 4]).unwrap();
    let query = LanguageParserJointScoreBandQuery::new(runtime).unwrap();
    let mut execution =
        parser_kernel::Execution::prepare(source(), "language-parser-joint-score-band-1000");
    execution.kernel.start().unwrap();
    let result = execution.transact(0, &query.into_structured().unwrap());
    let native = LanguageParserJointRuntimeRawBeam::from_structured(joint::retype(
        &LanguageParserJointRuntimeRawBeam::semantic_type().unwrap(),
        &result,
    ))
    .unwrap();
    for (slot, old, expected) in [
        (native.candidate0(), beam.candidate0(), true),
        (native.candidate1(), beam.candidate1(), true),
        (native.candidate2(), beam.candidate2(), true),
        (native.candidate3(), beam.candidate3(), false),
    ] {
        assert_eq!(*slot.hypothesis().parser().active(), expected);
        assert_eq!(slot.hypothesis().parser().state(), old.parser().state());
        assert_eq!(slot.hypothesis().parser().score(), old.parser().score());
        assert_eq!(slot.hypothesis().choices(), old.choices());
    }
    let forged = LanguageParserJointBeam::new(
        initial.basis().clone(),
        candidate(0, true),
        candidate(1, true),
        candidate(0, false),
        candidate(0, false),
        0,
        1,
        beam.lexical().clone(),
    )
    .unwrap();
    assert!(LanguageParserJointScoreBandQuery::new(
        LanguageParserJointRuntimeBeam::new(forged, [0; 4]).unwrap()
    )
    .is_err());
}

#[test]
#[ignore = "ordinary Source policy replay of preserved actual learned stream receipts"]
fn actual_stream_score_band_admits_independent_non_root_facts() {
    let rows: Vec<serde_json::Value> = serde_json::from_slice(include_bytes!(
        "../training/ewt_joint_v2/native_stream_planned_graph_receipts.json"
    ))
    .unwrap();
    let mut pruning =
        parser_kernel::Execution::prepare(source(), "language-parser-joint-score-band-1000");
    let mut facts =
        parser_kernel::Execution::prepare(source(), "language-parser-joint-stable-fact");
    pruning.kernel.start().unwrap();
    facts.kernel.start().unwrap();
    let mut commit = parser_kernel::Execution::prepare(source(), "language-parser-joint-commit");
    commit.kernel.start().unwrap();
    let mut fact_sequence = 0;
    let mut commit_sequence = 0;
    let mut receipts = Vec::new();
    for (sequence, row) in rows.iter().enumerate() {
        let bytes: Vec<u8> = serde_json::from_value(row["session"]["beam_bytes"].clone()).unwrap();
        let runtime = LanguageParserJointRuntimeBeam::from_structured(
            conduit_core::StructuredInfoValue::from_canonical_bytes(&bytes).unwrap(),
        )
        .unwrap();
        let query = LanguageParserJointScoreBandQuery::new(runtime.clone()).unwrap();
        let result = pruning.transact(sequence as u64, &query.into_structured().unwrap());
        let slots = LanguageParserJointRuntimeRawBeam::from_structured(joint::retype(
            &LanguageParserJointRuntimeRawBeam::semantic_type().unwrap(),
            &result,
        ))
        .unwrap();
        let admitted = joint::admit(
            &slots,
            runtime.beam().lexical(),
            runtime.beam().basis(),
            sequence as u64,
        );
        let expected = [vec![false], vec![true, false], vec![true, true]];
        let mut observed = Vec::new();
        let mut fact_receipts = Vec::new();
        for dependent in 0..*admitted.beam().lexical().token_count() {
            let query =
                LanguageParserJointConsensusQuery::new(admitted.beam().clone(), dependent).unwrap();
            let result = facts.transact(fact_sequence, &query.clone().into_structured().unwrap());
            fact_sequence += 1;
            let proposal = LanguageParserJointStableFactProposal::from_structured(result).unwrap();
            assert_eq!(proposal.query(), &query);
            let native = LanguageParserJointStableFact::from_structured(joint::retype(
                &LanguageParserJointStableFact::semantic_type().unwrap(),
                &proposal.into_structured().unwrap(),
            ));
            eprintln!(
                "NATIVE_STABLE_FACT id={} dependent={} admission={:?}",
                row["id"],
                dependent,
                native.as_ref().map(|_| ())
            );
            let accepted = native.is_ok();
            if let Ok(fact) = &native {
                assert_eq!(fact.query(), &query);
            }
            observed.push(accepted);
            let fact_bytes = native.as_ref().ok().map(|fact| {
                fact.clone()
                    .into_structured()
                    .unwrap()
                    .canonical_bytes()
                    .unwrap()
            });
            let refusal = native.as_ref().err().map(|error| format!("{error:?}"));
            fact_receipts.push(serde_json::json!({"dependent":dependent,"accepted":accepted,"native_fact_bytes":fact_bytes,"refusal":refusal}));
            if accepted && dependent == 0 {
                let fact = native.unwrap();
                let query = LanguageParserJointCommitQuery::new(fact).unwrap();
                let result = commit.transact(commit_sequence, &query.into_structured().unwrap());
                commit_sequence += 1;
                let candidate = LanguageParserJointHypothesis::from_structured(joint::retype(
                    &LanguageParserJointHypothesis::semantic_type().unwrap(),
                    fixture::field(&result, "candidate0"),
                ))
                .unwrap();
                let old = admitted.beam().candidate0();
                assert_eq!(*candidate.parser().state().committed(), 1);
                assert_eq!(
                    candidate.parser().state().heads(),
                    old.parser().state().heads()
                );
                assert_eq!(
                    candidate.parser().state().basis(),
                    old.parser().state().basis()
                );
                assert_eq!(candidate.choices(), old.choices());
                println!("SOURCE_COMMITTED_PREFIX id={} committed=1", row["id"]);
            } else if accepted {
                assert!(LanguageParserJointCommitQuery::new(native.unwrap()).is_err());
            }
        }
        assert_eq!(observed, expected[sequence]);
        receipts.push(serde_json::json!({"id":row["id"],"source_revision":admitted.beam().basis().source_revision().get(),"analysis_revision":admitted.beam().basis().analysis_revision().get(),"policy":"uncalibrated-score-band-1000","proof":"ordinary Source raw proposal followed by generic Native Source-law admission","scope":"post-decode replay of retained actual learned beams; not live commitment","facts":fact_receipts}));
        println!(
            "SOURCE_STABLE_FACTS id={} admitted={observed:?} policy=uncalibrated-score-band-1000",
            row["id"]
        );
    }
    if let Ok(path) = std::env::var("CONDUIT_PARSER_POLICY_RECEIPTS_OUTPUT") {
        std::fs::write(path, serde_json::to_vec_pretty(&receipts).unwrap()).unwrap();
    }
}
