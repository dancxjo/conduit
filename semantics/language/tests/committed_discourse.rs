//! Continue original retained parser commitments, without rerunning inference.
extern crate alloc;
use conduit_language::{lexical::prepare_lexical_tape, *};
use conduit_plot::rust_binding::NativeRustBinding;
#[path = "../src/committed_discourse.rs"]
mod owned;

#[test]
#[ignore = "requires original actual word-stream commitment receipt"]
fn actual_committed_vocative_retains_original_dependency_custody() {
    let path = std::env::var("CONDUIT_WORD_STREAM_COMMITTED_ROLES").unwrap();
    assert!(std::fs::metadata(&path).unwrap().len() <= 32 * 1024 * 1024);
    let receipt: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let bytes =
        |value: &serde_json::Value| -> Vec<u8> { serde_json::from_value(value.clone()).unwrap() };
    let tape = LanguageLexicalTape::decode(&bytes(&receipt["lexical_tape_bytes"])).unwrap();
    let mut lexical = None;
    let mut earlier = None;
    for (index, revision) in receipt["source_revision_history_bytes"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let source = LanguageTextRevision::decode(&bytes(revision)).unwrap();
        lexical = Some(prepare_lexical_tape(&source, tape.profile(), lexical.as_ref()).unwrap());
        if index == 0 {
            earlier = Some(prepare_lexical_tape(&source, tape.profile(), None).unwrap());
        }
    }
    let lexical = lexical.unwrap();
    assert_eq!(lexical.tape(), &tape);
    let provenance = || {
        LinguisticDerivationProvenance::deterministic_rule(
            "language/committed-vocative-discourse".into(),
            "actual-retained-commit/1".into(),
        )
        .unwrap()
    };
    let rows = receipt["commitments"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    let mut vocatives = 0;
    for row in rows {
        let committed = LanguageParserCommittedDependencyAdmission::decode(&bytes(
            &row["committed_dependency_admission_bytes"],
        ))
        .unwrap();
        let result = owned::prepare_committed_vocative_fact(
            "committed/addressee".into(),
            &lexical,
            &committed,
            provenance(),
        );
        if row["dependent"] == 2 {
            let prepared = result.unwrap();
            vocatives += 1;
            assert_eq!(prepared.committed(), &committed);
            assert_eq!(prepared.fact().fact().basis(), committed.admission().arc());
            assert_eq!(prepared.fact().fact().source(), tape.source());
            assert!(matches!(
                prepared.fact().fact().role(),
                LanguageDiscourseRole::Addressee
            ));
            assert!(matches!(
                owned::prepare_committed_vocative_fact(
                    "committed/addressee".into(),
                    earlier.as_ref().unwrap(),
                    &committed,
                    provenance(),
                ),
                Err(owned::CommittedDiscourseRefusal::LexicalBasis)
            ));
        } else {
            let Err(owned::CommittedDiscourseRefusal::Discourse(refusal)) = result else {
                panic!("a committed root must not become an addressee")
            };
            assert!(matches!(
                refusal,
                discourse::DiscourseRefusal::TokenBound | discourse::DiscourseRefusal::Native(_)
            ));
        }
    }
    assert_eq!(vocatives, 1);
}
