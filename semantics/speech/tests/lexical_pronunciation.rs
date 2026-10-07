#![cfg(feature = "semantic-bindings")]
use conduit_language::{pronunciation_selection::*, *};
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{lexical_pronunciation::*, semantic::*};
#[path = "../../language/tests/common/pronunciation_fixture.rs"]
mod language_fixture;
#[path = "common/pronunciation_fixture.rs"]
mod speech_fixture;
#[test]
fn exact_supplied_syntax_choice_changes_phonetics_and_native_intent_waveform() {
    let mut recordings = Vec::new();
    for (text, relation, pos, ipa) in [
        (
            "a record",
            LanguageUniversalDependencyRelation::Det,
            LanguageLexicalPos::Noun,
            "ɹɛkəɹd",
        ),
        (
            "we record",
            LanguageUniversalDependencyRelation::Nsubj,
            LanguageLexicalPos::Verb,
            "ɹɪkɔɹd",
        ),
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
        let pronunciation = prepare_pronunciation(&selection, &speech_fixture::profile()).unwrap();
        assert_eq!(pronunciation.selection().request().basis(), &arc);
        assert_eq!(pronunciation.request().candidate().pos(), &pos);
        assert_eq!(
            pronunciation
                .selection()
                .request()
                .token()
                .candidates()
                .len(),
            2
        );
        let fixture = speech_fixture::fixture(&selection, &pronunciation);
        let realized = fixture.realize();
        assert_eq!(realized.source(), &fixture.intent);
        assert_eq!(
            *pronunciation.row_selection().index(),
            if pos == LanguageLexicalPos::Noun {
                0
            } else {
                1
            }
        );
        for event in fixture.intent.events().iter() {
            let SpeechUtteranceIntentEvent::Segment(segment) = event else {
                panic!("only selected phones")
            };
            assert_eq!(segment.sources().len(), 1);
            let LanguageSegmentRef::Text(reference) = &segment.sources()[0] else {
                panic!("exact text occurrence")
            };
            assert_eq!(
                reference.text_id(),
                selection.request().source().material().identity()
            );
            assert_eq!(
                reference.revision_id(),
                selection.request().source().material().revision()
            );
            assert_eq!(
                *reference.range().start(),
                if text == "a record" { 2 } else { 3 }
            );
            assert_eq!(
                *reference.range().end(),
                if text == "a record" { 8 } else { 9 }
            );
        }
        let phones = pronunciation
            .result()
            .phones()
            .iter()
            .map(|selected| {
                fixture
                    .inventory
                    .phones()
                    .iter()
                    .find(|phone| phone.identity() == selected.phone())
                    .unwrap()
                    .ipa()
                    .as_str()
            })
            .collect::<String>();
        assert_eq!(phones, ipa);
        let mut renderer = realized.renderer().unwrap();
        let mut pcm = Vec::new();
        while !renderer.is_complete() {
            let mut block = [0i16; 63];
            let count = renderer.render(&mut block).unwrap();
            pcm.extend_from_slice(&block[..count]);
        }
        assert!(pcm.iter().any(|sample| *sample != 0));
        recordings.push(pcm);
    }
    assert_ne!(&recordings[0][800..1600], &recordings[1][800..1600]);
    assert_ne!(recordings[0], recordings[1]);
}
#[test]
fn unlisted_duplicate_and_foreign_language_profiles_refuse() {
    let lexical = language_fixture::lexical("a record");
    let arc = language_fixture::arc(&lexical, LanguageUniversalDependencyRelation::Det);
    let selected = prepare_pronunciation_selection(
        &lexical,
        1,
        &language_fixture::analysis(),
        &arc,
        &language_fixture::profile(),
    )
    .unwrap();
    let profile = speech_fixture::profile();
    let unlisted = SpeechPronunciationProfile::new(
        profile.identity().clone(),
        profile.language().clone(),
        profile.provenance().clone(),
        BoundedSequence::try_from_iter([profile.rows()[1].clone()]).unwrap(),
    )
    .unwrap();
    let Err(PronunciationRefusal::Unresolved(rejected)) =
        prepare_pronunciation(&selected, &unlisted)
    else {
        panic!("unsupported exact candidate")
    };
    assert_eq!(rejected.request().profile(), &unlisted);
    assert_eq!(rejected.request().candidate(), selected.candidate());
    assert_eq!(*rejected.selection().index(), 4);
    let duplicate = SpeechPronunciationProfile::new(
        profile.identity().clone(),
        profile.language().clone(),
        profile.provenance().clone(),
        BoundedSequence::try_from_iter([profile.rows()[0].clone(), profile.rows()[0].clone()])
            .unwrap(),
    )
    .unwrap();
    assert!(matches!(
        prepare_pronunciation(&selected, &duplicate),
        Err(PronunciationRefusal::Unresolved(_))
    ));
    let foreign = SpeechPronunciationProfile::new(
        profile.identity().clone(),
        LanguageId::new("language/fr".into()).unwrap(),
        profile.provenance().clone(),
        profile.rows().clone(),
    )
    .unwrap();
    assert!(matches!(
        prepare_pronunciation(&selected, &foreign),
        Err(PronunciationRefusal::Native(_))
    ));
}

#[test]
fn immutable_intent_adapter_refuses_foreign_language_and_nonfresh_origin() {
    use conduit_speech::pronunciation_intent::*;
    let lexical = language_fixture::lexical("a record");
    let arc = language_fixture::arc(&lexical, LanguageUniversalDependencyRelation::Det);
    let selected = prepare_pronunciation_selection(
        &lexical,
        1,
        &language_fixture::analysis(),
        &arc,
        &language_fixture::profile(),
    )
    .unwrap();
    let pronunciation = prepare_pronunciation(&selected, &speech_fixture::profile()).unwrap();
    let fixture = speech_fixture::fixture(&selected, &pronunciation);
    let SpeechUtteranceIntentEvent::Segment(segment) = &fixture.intent.events()[0] else {
        panic!("selected segment")
    };
    let origin = segment.occurrence();
    let foreign = LanguageSpeechTokenRef::new(
        origin.inventory_id().clone(),
        LanguageId::new("language/fr".into()).unwrap(),
        0,
        origin.revision_id().clone(),
        origin.sequence_id().clone(),
        origin.utterance_id().clone(),
    )
    .unwrap();
    assert!(matches!(
        prepare_pronunciation_intent(
            &pronunciation,
            &foreign,
            segment.prosody(),
            segment.provenance()
        ),
        Err(PronunciationIntentRefusal::Language)
    ));
    let used = LanguageSpeechTokenRef::new(
        origin.inventory_id().clone(),
        origin.language().clone(),
        1,
        origin.revision_id().clone(),
        origin.sequence_id().clone(),
        origin.utterance_id().clone(),
    )
    .unwrap();
    assert!(matches!(
        prepare_pronunciation_intent(
            &pronunciation,
            &used,
            segment.prosody(),
            segment.provenance()
        ),
        Err(PronunciationIntentRefusal::Origin)
    ));
}
