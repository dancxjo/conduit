#![cfg(feature = "semantic-bindings")]
use conduit_language::{pronunciation_selection::*, *};
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_speech::{phonemic_pronunciation::*, phonemic_pronunciation_intent::*, semantic::*};
#[path = "../../language/tests/common/pronunciation_fixture.rs"]
mod language_fixture;
#[path = "common/pronunciation_fixture.rs"]
mod speech_fixture;
fn profile() -> SpeechPhonemicPronunciationProfile {
    let unit = |id: &str, stress| {
        SpeechPhonemicPronunciationPhoneme::new(PhonemeId::new(id.into()).unwrap(), stress).unwrap()
    };
    let row = |pos, vowel, stress| {
        SpeechPhonemicPronunciationRow::new(
            language_fixture::candidate(pos),
            BoundedSequence::try_from_iter([
                unit("phoneme/ɹ", SpeechStress::Unstressed),
                unit(vowel, stress),
            ])
            .unwrap(),
        )
        .unwrap()
    };
    SpeechPhonemicPronunciationProfile::new(
        "reviewed/phonemic-record".into(),
        LanguageId::new("language/en".into()).unwrap(),
        speech_fixture::profile().provenance().clone(),
        BoundedSequence::try_from_iter([
            row(LanguageLexicalPos::Noun, "phoneme/ɛ", SpeechStress::Primary),
            row(
                LanguageLexicalPos::Verb,
                "phoneme/ɪ",
                SpeechStress::Unstressed,
            ),
        ])
        .unwrap(),
    )
    .unwrap()
}
#[test]
fn original_lexical_selection_and_source_receipts_survive_phonemic_intent() {
    for (text, relation, index) in [
        ("a record", LanguageUniversalDependencyRelation::Det, 0),
        ("we record", LanguageUniversalDependencyRelation::Nsubj, 1),
    ] {
        let lexical = language_fixture::lexical(text);
        let arc = language_fixture::arc(&lexical, relation);
        let selection = prepare_pronunciation_selection(
            &lexical,
            1,
            &language_fixture::analysis(),
            &arc,
            &language_fixture::profile(),
        )
        .unwrap();
        let profile = profile();
        let prepared = prepare_phonemic_pronunciation(&selection, &profile).unwrap();
        assert!(core::ptr::eq(prepared.selection(), &selection));
        assert!(core::ptr::eq(prepared.profile(), &profile));
        assert!(core::ptr::eq(prepared.row(), &profile.rows()[index]));
        assert_eq!(*prepared.index().index(), index as u64);
        assert_eq!(prepared.request().candidate(), selection.candidate());
        assert_eq!(
            prepared.request_canonical(),
            prepared.request().clone().encode().unwrap()
        );
        assert_eq!(prepared.executions().len(), 1);
        let execution = &prepared.executions()[0];
        assert_eq!(
            execution.input_canonical(),
            prepared.query().clone().encode().unwrap()
        );
        let replay = conduit_plot::PortableExpressionProgram::from_canonical_hex(
            execution.source_program_hex(),
        )
        .unwrap()
        .evaluate(execution.input_canonical())
        .unwrap();
        assert_eq!(replay, execution.output_canonical());
        assert_eq!(
            SpeechPhonemicPronunciationRowIndex::decode(&replay).unwrap(),
            *prepared.index()
        );
        assert_eq!(prepared.result().phonemes(), prepared.row().phonemes());
        assert_eq!(
            prepared.admitted_canonical(),
            prepared.result().clone().encode().unwrap()
        );
        let legacy_profile = speech_fixture::profile();
        let legacy = conduit_speech::lexical_pronunciation::prepare_pronunciation(
            &selection,
            &legacy_profile,
        )
        .unwrap();
        let old = speech_fixture::fixture(&selection, &legacy);
        let SpeechUtteranceIntentEvent::Segment(first) = &old.intent.events()[0] else {
            panic!("segment")
        };
        let intent = prepare_phonemic_pronunciation_intent(
            &prepared,
            first.occurrence(),
            first.prosody(),
            first.provenance(),
        )
        .unwrap();
        assert!(core::ptr::eq(intent.pronunciation(), &prepared));
        for (ordinal, event) in intent.intent().events().iter().enumerate() {
            let SpeechUtteranceIntentEvent::Segment(segment) = event else {
                panic!("segment")
            };
            assert_eq!(segment.phone(), &PhoneSpecification::unspecified());
            assert_eq!(
                segment.phoneme(),
                &PhonemeSpecification::known(prepared.row().phonemes()[ordinal].phoneme().clone())
                    .unwrap()
            );
            assert_eq!(segment.sources(), first.sources());
            assert_eq!(segment.prosody(), first.prosody());
            assert_eq!(*segment.occurrence().ordinal(), ordinal as u32);
        }
        assert!(matches!(first.phone(), PhoneSpecification::Known(_)));
        assert_eq!(first.phoneme(), &PhonemeSpecification::unspecified());
        let missing = SpeechPhonemicPronunciationProfile::new(
            profile.identity().clone(),
            profile.language().clone(),
            profile.provenance().clone(),
            BoundedSequence::try_from_iter([profile.rows()[1 - index].clone()]).unwrap(),
        )
        .unwrap();
        let Err(PhonemicPronunciationRefusal::Unresolved(rejected)) =
            prepare_phonemic_pronunciation(&selection, &missing)
        else {
            panic!("missing exact candidate")
        };
        assert_eq!(*rejected.index().index(), 4);
        assert_eq!(rejected.request().profile(), &missing);
        assert_eq!(
            rejected.executions()[0].input_canonical(),
            rejected.query().clone().encode().unwrap()
        );
        let duplicate = SpeechPhonemicPronunciationProfile::new(
            profile.identity().clone(),
            profile.language().clone(),
            profile.provenance().clone(),
            BoundedSequence::try_from_iter([
                profile.rows()[index].clone(),
                profile.rows()[index].clone(),
            ])
            .unwrap(),
        )
        .unwrap();
        assert!(matches!(
            prepare_phonemic_pronunciation(&selection, &duplicate),
            Err(PhonemicPronunciationRefusal::Unresolved(_))
        ));
        let foreign = SpeechPhonemicPronunciationProfile::new(
            profile.identity().clone(),
            LanguageId::new("language/fr".into()).unwrap(),
            profile.provenance().clone(),
            profile.rows().clone(),
        )
        .unwrap();
        assert!(matches!(
            prepare_phonemic_pronunciation(&selection, &foreign),
            Err(PhonemicPronunciationRefusal::Native(_))
        ));
    }
}
