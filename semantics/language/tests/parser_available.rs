use conduit_language::{lexical::*, *};
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
#[path = "common/parser_fixture.rs"]
mod fixture;
#[path = "common/parser_kernel.rs"]
mod parser_kernel;
fn provenance() -> LinguisticDerivationProvenance {
    LinguisticDerivationProvenance::deterministic_rule("fixture/lexical".into(), "profile/1".into())
        .unwrap()
}
fn revision(
    text: &str,
    sequence: u64,
    previous: Option<&LanguageTextRevision>,
    stable: Option<u32>,
    finality: LanguageTextFinality,
) -> LanguageTextRevision {
    LanguageTextRevision::new(
        finality,
        LanguageText::new(
            LanguageTextId::new("text".into()).unwrap(),
            LanguageId::new("language/en".into()).unwrap(),
            LanguageTextRevisionId::new(format!("r/{sequence}")).unwrap(),
            text.into(),
        )
        .unwrap(),
        previous.map(|old| {
            LanguageTextPriorRevision::new(old.material().revision().clone(), *old.sequence())
                .unwrap()
        }),
        provenance(),
        sequence,
        stable,
    )
    .unwrap()
}
fn profile() -> LanguageLexicalProfile {
    let candidate = |lemma: &str, pos| {
        LanguageLexicalCandidate::new(lemma.into(), BoundedSequence::new(), pos).unwrap()
    };
    let entries = BoundedSequence::try_from_iter([
        LanguageLexicalEntry::new(
            BoundedSequence::try_from_iter([candidate("answer", LanguageLexicalPos::Noun)])
                .unwrap(),
            "answer".into(),
        )
        .unwrap(),
        LanguageLexicalEntry::new(
            BoundedSequence::try_from_iter([
                candidate("record", LanguageLexicalPos::Noun),
                candidate("record", LanguageLexicalPos::Verb),
            ])
            .unwrap(),
            "record".into(),
        )
        .unwrap(),
    ])
    .unwrap();
    LanguageLexicalProfile::new(
        entries,
        "lexical/fixture".into(),
        LanguageId::new("language/en".into()).unwrap(),
        provenance(),
    )
    .unwrap()
}

fn source() -> String {
    [
        fixture::parser_source(),
        include_str!("../text_revision.conduit").into(),
        include_str!("../lexical.conduit").into(),
        include_str!("../parser_available.conduit").into(),
    ]
    .join("\n")
}
fn admit_availability(value: &conduit_core::StructuredInfoValue) -> LanguageParserAvailability {
    let ty = LanguageParserAvailability::semantic_type().unwrap();
    let admitted = fixture::record(
        &ty,
        vec![
            ("lexical", fixture::field(value, "lexical").clone()),
            ("waiting", fixture::field(value, "waiting").clone()),
            ("final_input", fixture::field(value, "final_input").clone()),
        ],
    );
    LanguageParserAvailability::from_structured(admitted).unwrap()
}
#[test]
fn available_prefix_refuses_partial_tokens_and_source_drives_finality() {
    let material = revision("answer re", 0, None, None, LanguageTextFinality::Partial);
    let prepared = prepare_lexical_tape(&material, &profile(), None).unwrap();
    let zero = LanguageParserAvailableLexical::new(prepared.tape().clone(), 0).unwrap();
    let prefix = LanguageParserAvailableLexical::new(prepared.tape().clone(), 1).unwrap();
    assert!(LanguageParserAvailability::new(true, prefix.clone(), false).is_err());
    assert!(LanguageParserAvailability::new(false, prefix.clone(), true).is_err());
    assert!(LanguageParserAvailability::new(false, zero.clone(), false).is_err());
    assert!(LanguageParserAvailableLexical::new(prepared.tape().clone(), 2).is_err());
    assert!(LanguageParserAvailableLexical::new(prepared.tape().clone(), 5).is_err());
    let blueprint = parser_kernel::Blueprint::prepare(source(), "language-parser-availability");
    let mut execution = blueprint.realize(0);
    execution.kernel.start().unwrap();
    let first = execution.transact(0, &zero.into_structured().unwrap());
    let first = admit_availability(&first);
    assert!(*first.waiting());
    assert!(!*first.final_input());
    let next = execution.transact(1, &prefix.into_structured().unwrap());
    let next = admit_availability(&next);
    assert!(!*next.waiting());
    assert!(!*next.final_input());
    let final_material = revision("answer", 0, None, None, LanguageTextFinality::Final);
    let final_tape = prepare_lexical_tape(&final_material, &profile(), None).unwrap();
    let final_prefix = LanguageParserAvailableLexical::new(final_tape.tape().clone(), 1).unwrap();
    let final_result = execution.transact(2, &final_prefix.into_structured().unwrap());
    let final_result = admit_availability(&final_result);
    assert!(!*final_result.waiting());
    assert!(*final_result.final_input());
}

fn retype_record(
    ty: &conduit_core::StructuredInfoType,
    value: &conduit_core::StructuredInfoValue,
) -> conduit_core::StructuredInfoValue {
    let conduit_core::StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("expected carrier")
    };
    fixture::record(
        ty,
        fields
            .iter()
            .map(|field| (field.name(), fixture::field(value, field.name()).clone()))
            .collect(),
    )
}
#[test]
fn nonfinal_exhaustion_suppresses_all_proposals_and_growth_preserves_state() {
    let material = revision("answer record", 0, None, None, LanguageTextFinality::Final);
    let tape = prepare_lexical_tape(&material, &profile(), None).unwrap();
    let available1 = LanguageParserAvailableLexical::new(tape.tape().clone(), 1).unwrap();
    let available2 = LanguageParserAvailableLexical::new(tape.tape().clone(), 2).unwrap();
    let basis = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("analysis/1".into()).unwrap(),
        material.material().revision().clone(),
        material.material().identity().clone(),
    )
    .unwrap();
    let dep = LanguageParserRelation::new(
        LanguageUniversalDependencyRelation::Dep,
        LanguageParserSubtype::new("".into()).unwrap(),
    )
    .unwrap();
    let root = LanguageParserRelation::new(
        LanguageUniversalDependencyRelation::Root,
        LanguageParserSubtype::new("".into()).unwrap(),
    )
    .unwrap();
    let state = LanguageParserState::new(
        basis.clone(),
        0,
        2,
        [4, 5, 5, 5, 4],
        root,
        dep.clone(),
        dep.clone(),
        dep,
        [4, 0, 4, 4, 4],
        1,
        1,
    )
    .unwrap();
    let input = LanguageParserAvailableState::new(available1, state.clone()).unwrap();
    let mut wait = parser_kernel::Execution::prepare(source(), "language-parser-wait-state");
    wait.kernel.start().unwrap();
    let output = wait.transact(0, &input.into_structured().unwrap());
    let status = LanguageParserWaitState::from_structured(retype_record(
        &LanguageParserWaitState::semantic_type().unwrap(),
        &output,
    ))
    .unwrap();
    assert!(*status.waiting());
    assert!(!*status.final_input());
    assert!(LanguageParserWaitState::new(false, status.input().clone(), false).is_err());
    let numeric = LanguageParserNumericState::from_structured(retype_record(
        &LanguageParserNumericState::semantic_type().unwrap(),
        &state.clone().into_structured().unwrap(),
    ))
    .unwrap();
    // Adversarial raw proposals: waiting must suppress every possible class.
    let mask = LanguageParserLegalMask::new([true; 76], basis, numeric).unwrap();
    let query = LanguageParserAvailableMask::new(mask, status).unwrap();
    let mut gate = parser_kernel::Execution::prepare(source(), "language-parser-wait-mask");
    gate.kernel.start().unwrap();
    let gated = LanguageParserLegalMask::from_structured(
        gate.transact(0, &query.into_structured().unwrap()),
    )
    .unwrap();
    assert!(gated.allowed().iter().all(|allowed| !allowed));
    let growth = LanguageParserAvailableGrowth::new(available2.clone(), state.clone()).unwrap();
    let mut grow = parser_kernel::Execution::prepare(source(), "language-parser-available-growth");
    grow.kernel.start().unwrap();
    let raw = grow.transact(0, &growth.into_structured().unwrap());
    let admitted = LanguageParserBufferGrowth::from_structured(retype_record(
        &LanguageParserBufferGrowth::semantic_type().unwrap(),
        &raw,
    ))
    .unwrap();
    let mut mutation = parser_kernel::Execution::prepare(source(), "language-parser-grow-buffer");
    mutation.kernel.start().unwrap();
    let grown = mutation.transact(0, &admitted.into_structured().unwrap());
    let grown = LanguageParserState::from_structured(retype_record(
        &LanguageParserState::semantic_type().unwrap(),
        &grown,
    ))
    .unwrap();
    assert_eq!(*grown.token_count(), 2);
    assert_eq!(grown.heads(), state.heads());
    assert_eq!(grown.stack(), state.stack());
    assert_eq!(grown.basis(), state.basis());
    assert_eq!(grown.unread(), state.unread());
    let input = LanguageParserAvailableState::new(available2, grown).unwrap();
    let output = wait.transact(1, &input.into_structured().unwrap());
    let status = LanguageParserWaitState::from_structured(retype_record(
        &LanguageParserWaitState::semantic_type().unwrap(),
        &output,
    ))
    .unwrap();
    assert!(!*status.waiting());
    assert!(*status.final_input());
}

#[test]
fn v2_lookahead_is_available_candidate_membership_without_future_pos_choice() {
    let material = revision("answer record", 0, None, None, LanguageTextFinality::Final);
    let tape = prepare_lexical_tape(&material, &profile(), None).unwrap();
    let basis = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("analysis/1".into()).unwrap(),
        material.material().revision().clone(),
        material.material().identity().clone(),
    )
    .unwrap();
    let dep = LanguageParserRelation::new(
        LanguageUniversalDependencyRelation::Dep,
        LanguageParserSubtype::new("".into()).unwrap(),
    )
    .unwrap();
    let src = format!(
        "{}\n{}",
        source(),
        include_str!("../parser_scorer_v2.conduit")
    );
    let mut execution = parser_kernel::Execution::prepare(src, "language-parser-v2-model-features");
    execution.kernel.start().unwrap();
    for (invocation, count, future_choice) in [(0, 2, 0), (1, 2, 1), (2, 1, 3)] {
        let lexical = LanguageParserAvailableLexical::new(tape.tape().clone(), count).unwrap();
        let state = LanguageParserState::new(
            basis.clone(),
            0,
            1,
            [5, 5, 5, 5, 4],
            dep.clone(),
            dep.clone(),
            dep.clone(),
            dep.clone(),
            [4, 4, 4, 4, 4],
            count,
            0,
        )
        .unwrap();
        let query = LanguageParserV2ChoiceQuery::new(
            [0, future_choice, 0, 0],
            0,
            invocation,
            lexical,
            state,
        )
        .unwrap();
        let result = LanguageParserV2ModelFeatures::from_structured(
            execution.transact(invocation, &query.into_structured().unwrap()),
        )
        .unwrap();
        assert_eq!(&result.indices()[..7], &[17, 25, 349, 361, 366, 371, 373]);
        assert_eq!(result.indices()[7], 374 + count);
        for code in 0..17 {
            let present = count == 2 && (code == 7 || code == 15);
            assert_eq!(
                result.indices()[8 + code],
                379 + 2 * code as u64 + u64::from(present)
            );
        }
    }
}
