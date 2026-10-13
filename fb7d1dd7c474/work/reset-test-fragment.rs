
#[test]
fn revision_reset_matches_full_lineage_and_preserves_committed_state() {
    let previous = revision("answer record", 0, None, None, LanguageTextFinality::Final);
    let next = revision("record answer", 1, Some(&previous), None, LanguageTextFinality::Final);
    let profile = profile();
    let old_tape = prepare_lexical_tape(&previous, &profile, None).unwrap();
    let new_tape = prepare_lexical_tape(&next, &profile, Some(&old_tape)).unwrap();
    let old = LanguageParserAvailableLexical::new(old_tape.tape().clone(), 2).unwrap();
    let new = LanguageParserAvailableLexical::new(new_tape.tape().clone(), 2).unwrap();
    let basis = LanguageParserBasis::new(LanguageAnalysisRevisionId::new("analysis/1".into()).unwrap(), previous.material().revision().clone(), previous.material().identity().clone()).unwrap();
    let new_basis = LanguageParserBasis::new(LanguageAnalysisRevisionId::new("analysis/2".into()).unwrap(), next.material().revision().clone(), next.material().identity().clone()).unwrap();
    let dep = LanguageParserRelation::new(LanguageUniversalDependencyRelation::Dep, LanguageParserSubtype::new("".into()).unwrap()).unwrap();
    let root = LanguageParserRelation::new(LanguageUniversalDependencyRelation::Root, LanguageParserSubtype::new("".into()).unwrap()).unwrap();
    let lineage = prepare_text_revision_lineage(previous.clone(), next).unwrap();
    let src = format!("{}\n{}\n{}", source(), include_str!("../revision_lineage.conduit"), include_str!("../parser_revision.conduit"));
    let mut execution = parser_kernel::Execution::prepare(src, "language-parser-revision-reset");
    execution.kernel.start().unwrap();
    for (invocation, committed) in [(0,0), (1,1)] {
        let state = LanguageParserState::new(basis.clone(), committed, 2, [4,5,5,5,4], root.clone(), dep.clone(), dep.clone(), dep.clone(), [4,0,4,4,4], 2, 1).unwrap();
        let context = LanguageParserRevisionContext::new(new_basis.clone(), lineage.clone(), new.clone(), LanguageParserAvailableState::new(old.clone(), state.clone()).unwrap()).unwrap();
        let result = execution.transact(invocation, &context.into_structured().unwrap());
        let accepted = fixture::accepted(&result);
        assert_eq!(accepted, committed == 0);
        let value = fixture::field(&result, "state");
        let admitted = LanguageParserState::from_structured(retype_record(&LanguageParserState::semantic_type().unwrap(), value)).unwrap();
        if committed == 0 {
            assert_eq!(admitted.basis(), &new_basis);
            assert_eq!(*admitted.unread(), 0);
            assert_eq!(*admitted.depth(), 1);
            assert_eq!(admitted.heads(), &[5,5,5,5,4]);
        } else {
            assert_eq!(admitted, state);
            assert_eq!(fixture::tag(fixture::field(&result, "refusal")), "committed_prefix");
        }
    }
    let foreign_basis = LanguageParserBasis::new(LanguageAnalysisRevisionId::new("analysis/3".into()).unwrap(), previous.material().revision().clone(), previous.material().identity().clone()).unwrap();
    let state = LanguageParserState::new(basis, 0, 1, [5,5,5,5,4], dep.clone(), dep.clone(), dep.clone(), dep, [4,4,4,4,4], 2, 0).unwrap();
    assert!(LanguageParserRevisionContext::new(foreign_basis, lineage, new, LanguageParserAvailableState::new(old, state).unwrap()).is_err());
}
