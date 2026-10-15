//! Resource and original-source boundaries of the supported revision profile.
#[path = "common/parser_revision_fixture.rs"]
mod fixture;
use conduit_language::{lexical::*, *};
use conduit_plot::rust_binding::BoundedSequence;

fn analysis() -> LanguageAnalysisRevisionId {
    LanguageAnalysisRevisionId::new("analysis/2".into()).unwrap()
}
#[test]
fn a_committed_dependent_also_protects_its_uncommitted_head_occurrence() {
    let old = fixture::revision("answer record ", None, 13);
    let tape = prepare_lexical_tape(&old, &fixture::profile(), None).unwrap();
    let basis = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("analysis/1".into()).unwrap(),
        old.material().revision().clone(),
        old.material().identity().clone(),
    )
    .unwrap();
    let root = LanguageParserRelation::new(
        LanguageUniversalDependencyRelation::Root,
        LanguageParserSubtype::new(String::new()).unwrap(),
    )
    .unwrap();
    let previous = LanguageParserState::new(
        basis,
        1,
        2,
        [1, 4, 5, 5, 4],
        fixture::dep(),
        root,
        fixture::dep(),
        fixture::dep(),
        [4, 1, 4, 4, 4],
        2,
        2,
    )
    .unwrap();
    let preserved = fixture::revision("answer record answer ", Some(&old), 13);
    assert!(PreparedParserRevisionRebase::prepare(
        &previous,
        &tape,
        &preserved,
        analysis(),
        fixture::dep()
    )
    .is_ok());
    let changed_head = fixture::revision("answer answer ", Some(&old), 13);
    assert!(matches!(
        PreparedParserRevisionRebase::prepare(
            &previous,
            &tape,
            &changed_head,
            analysis(),
            fixture::dep()
        ),
        Err(ParserRevisionRefusal::Revision(_))
    ));
    assert_eq!(tape.tape().source(), &old);
    assert_eq!(previous.heads()[0], 1);
}
#[test]
fn five_complete_tokens_refuse_before_rebase_publication() {
    let old = fixture::revision("answer record ", None, 7);
    let tape = prepare_lexical_tape(&old, &fixture::profile(), None).unwrap();
    let next = fixture::revision("answer record answer record answer ", Some(&old), 7);
    assert!(matches!(
        PreparedParserRevisionRebase::prepare(
            &fixture::state(1),
            &tape,
            &next,
            analysis(),
            fixture::dep()
        ),
        Err(ParserRevisionRefusal::Native(_))
    ));
    assert_eq!(tape.tape().source(), &old);
}
#[test]
fn large_but_legal_lexical_inventory_cannot_exceed_the_request_envelope() {
    let base = fixture::profile();
    let mut entries = base.entries().iter().cloned().collect::<Vec<_>>();
    for index in entries.len()..64 {
        let candidates = (0..4).map(|choice| {
            LanguageLexicalCandidate::new(
                format!("candidate_{index:02}_{choice}_{}", "x".repeat(110)),
                BoundedSequence::new(),
                LanguageLexicalPos::Noun,
            )
            .unwrap()
        });
        entries.push(
            LanguageLexicalEntry::new(
                BoundedSequence::try_from_iter(candidates).unwrap(),
                format!("unused_{index:02}"),
            )
            .unwrap(),
        );
    }
    let large = LanguageLexicalProfile::new(
        BoundedSequence::try_from_iter(entries).unwrap(),
        "fixture/large-revision".into(),
        base.language().clone(),
        base.provenance().clone(),
    )
    .unwrap();
    let old = fixture::revision("answer record ", None, 7);
    let tape = prepare_lexical_tape(&old, &large, None).unwrap();
    let next = fixture::revision("answer answer ", Some(&old), 7);
    assert!(matches!(
        PreparedParserRevisionRebase::prepare(
            &fixture::state(1),
            &tape,
            &next,
            analysis(),
            fixture::dep()
        ),
        Err(ParserRevisionRefusal::RequestBytes)
    ));
    assert_eq!(tape.tape().source(), &old);
}
