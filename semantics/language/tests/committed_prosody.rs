//! Continue original retained parser commitments, without rerunning inference.
extern crate alloc;
use conduit_language::committed_discourse as owned;
use conduit_language::committed_prosody;
use conduit_language::{lexical::prepare_lexical_tape, *};
use conduit_plot::rust_binding::NativeRustBinding;

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
            let profile = LanguageProsodyProfile::new(
                LanguageProsodyChoice::new(
                    LanguageProsodyBoundary::None,
                    LanguageProsodyPitch::Level,
                    LanguageProsodyProminence::Neutral,
                )
                .unwrap(),
                "reviewed/committed-vocative-prosody-v1".into(),
                tape.source().material().language().clone(),
                provenance(),
                LanguageProsodyChoice::new(
                    LanguageProsodyBoundary::MajorPhrase,
                    LanguageProsodyPitch::Rising,
                    LanguageProsodyProminence::Prominent,
                )
                .unwrap(),
            )
            .unwrap();
            let prosody = committed_prosody::prepare_rich_prosody_from_committed_vocative(
                &lexical, 2, &prepared, &profile,
            )
            .unwrap();
            assert!(core::ptr::eq(prosody.discourse(), &prepared));
            assert!(core::ptr::eq(prosody.discourse().committed(), &committed));
            assert_eq!(
                prosody.prepared().requested().discourse(),
                prepared.fact().fact()
            );
            assert_eq!(
                prosody.prepared().accepted().choice().pitch(),
                &LanguageProsodyPitch::Rising
            );
            assert!(matches!(
                committed_prosody::prepare_rich_prosody_from_committed_vocative(
                    earlier.as_ref().unwrap(),
                    2,
                    &prepared,
                    &profile
                ),
                Err(committed_prosody::CommittedVocativeProsodyRefusal::LexicalBasis)
            ));
            let Err(committed_prosody::CommittedVocativeProsodyRefusal::Prosody(reason)) =
                committed_prosody::prepare_rich_prosody_from_committed_vocative(
                    &lexical, 0, &prepared, &profile,
                )
            else {
                panic!("wrong word must refuse")
            };
            assert!(matches!(
                reason,
                conduit_language::prosody::ProsodyRefusal::Native(_)
            ));
            if let Some(path) = std::env::var_os("CONDUIT_COMMITTED_PROSODY_OUTPUT") {
                std::fs::write(path,serde_json::to_vec(&serde_json::json!({"schema":"language/rich-prosody-from-committed-vocative@1","scope":"Original retained dependency commitment through original discourse Source and original rich-prosody Source; no prosody commitment, PCM, neural or playback authority","original_dependency_admission_bytes":committed.clone().encode().unwrap(),"original_discourse_fact_bytes":prepared.fact().fact().clone().encode().unwrap(),"profile_bytes":profile.clone().encode().unwrap(),"rich_request_bytes":prosody.prepared().requested().clone().encode().unwrap(),"rich_accepted_bytes":prosody.prepared().accepted().clone().encode().unwrap(),"original_owner_pointer_checks":true})).unwrap()).unwrap();
            }
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
