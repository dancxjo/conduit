#![cfg(feature = "semantic-bindings")]
#[path = "../../language/tests/common/prosody.rs"]
mod fixture;
use conduit_language::{
    prosody::*, LanguageProsodyBoundary, LanguageProsodyPitch, LanguageProsodyProfile,
    LanguageProsodyProminence,
};
use conduit_speech::{linguistic_prosody::*, semantic::*};
fn binding(
    profile: &LanguageProsodyProfile,
    choice: conduit_language::LanguageProsodyChoice,
    phrase: bool,
) -> SpeechLinguisticProsodyBinding {
    let segment = SpeechSegmentProsodyIntent::new(
        SpeechDurationSpecification::known(25, 3).unwrap(),
        SpeechCycleSpecification::known(130, 1).unwrap(),
        SpeechIntensitySpecification::known(5, 4).unwrap(),
    )
    .unwrap();
    let realization = SpeechLinguisticProsodyRealization::new(
        SpeechDurationSpecification::known(100, if phrase { 3 } else { 0 }).unwrap(),
        SpeechBoundarySpecification::Known(if phrase {
            SpeechBoundaryKind::Phrase
        } else {
            SpeechBoundaryKind::Word
        }),
        segment,
    )
    .unwrap();
    SpeechLinguisticProsodyBinding::new(
        choice,
        "voice/profile/1".into(),
        profile.language().clone(),
        profile.identity().clone(),
        SpeechEvidenceProvenance::new(
            "authored/voice-profile".into(),
            SpeechEvidenceSource::Manual,
            Some("1".into()),
        )
        .unwrap(),
        realization,
    )
    .unwrap()
}
#[test]
fn exact_rich_plan_lowers_into_existing_rate_independent_segment_and_boundary_seams() {
    let fixture = fixture::fixture("Hello Travis", 1, 0);
    let rich = prepare_rich_prosody(
        &fixture.lexical,
        1,
        fixture.discourse.fact(),
        &fixture.profile,
    )
    .unwrap();
    let binding = binding(&fixture.profile, rich.accepted().choice().clone(), true);
    let prepared =
        prepare_linguistic_prosody(LinguisticProsodyBasis::Rich(&rich), &binding).unwrap();
    assert!(matches!(
        prepared.requested(),
        LinguisticProsodyBasis::Rich(_)
    ));
    assert_eq!(prepared.accepted().requested(), rich.accepted().choice());
    assert_eq!(prepared.selected_realization(), binding.realization());
    let formant = prepared.prepare_formant_segment().unwrap();
    assert_eq!(*formant.spans()[0].frame_count(), 960);
    let duration = formant.spans()[0].duration().clone();
    let sixteen =
        conduit_speech::duration::duration_spans(std::slice::from_ref(&duration), 16000).unwrap();
    assert_eq!(*sixteen[0].frame_count(), 1920);
    assert_eq!(sixteen[0].duration(), &duration);
    let boundary_profile = SpeechFormantBoundaryBinding::new(
        SpeechBoundaryKind::Phrase,
        SpeechFormantBoundary::Phrase,
    )
    .unwrap();
    let boundary = conduit_speech::boundary_admission::prepare_boundary(
        prepared.boundary(),
        &boundary_profile,
    )
    .unwrap();
    assert_eq!(*boundary.duration().numerator_seconds(), 3);
    assert_eq!(*boundary.duration().denominator(), 100);
    let LanguageSegmentRef::Text(source) = &prepared.boundary().sources()[0] else {
        panic!()
    };
    assert_eq!(
        source.text_id(),
        fixture.lexical.tape().source().material().identity()
    );
    assert_eq!(*source.range().start(), 6);
    assert_eq!(*source.range().end(), 12);
}
#[test]
fn fallback_does_not_satisfy_rich_choice_and_none_boundary_has_no_silent_pause() {
    let fixture = fixture::fixture("Hello Travis", 1, 0);
    let rich = prepare_rich_prosody(
        &fixture.lexical,
        1,
        fixture.discourse.fact(),
        &fixture.profile,
    )
    .unwrap();
    let fallback = prepare_fallback_prosody(&fixture.lexical, 1, &fixture.profile).unwrap();
    let binding = binding(
        &fixture.profile,
        fallback.accepted().choice().clone(),
        false,
    );
    assert!(matches!(
        prepare_linguistic_prosody(LinguisticProsodyBasis::Rich(&rich), &binding),
        Err(LinguisticProsodyRefusal::Admission { .. })
    ));
    let prepared =
        prepare_linguistic_prosody(LinguisticProsodyBasis::Fallback(&fallback), &binding).unwrap();
    assert!(matches!(
        prepared.requested(),
        LinguisticProsodyBasis::Fallback(_)
    ));
    let SpeechDurationSpecification::Known(duration) = prepared.boundary().duration() else {
        panic!()
    };
    assert_eq!(*duration.numerator_seconds(), 0);
}
#[test]
fn pitch_movement_and_foreign_profile_refuse_instead_of_being_hidden() {
    let fixture = fixture::fixture("Hello Travis", 1, 0);
    let rising = LanguageProsodyProfile::new(
        fixture.profile.fallback().clone(),
        "prosody/rising".into(),
        fixture.profile.language().clone(),
        fixture::provenance(),
        fixture::choice(
            LanguageProsodyBoundary::MinorPhrase,
            LanguageProsodyProminence::Prominent,
            LanguageProsodyPitch::Rising,
        ),
    )
    .unwrap();
    let rich =
        prepare_rich_prosody(&fixture.lexical, 1, fixture.discourse.fact(), &rising).unwrap();
    let offered = binding(&rising, rich.accepted().choice().clone(), true);
    assert!(matches!(
        prepare_linguistic_prosody(LinguisticProsodyBasis::Rich(&rich), &offered),
        Err(LinguisticProsodyRefusal::Admission { .. })
    ));
    let Err(LinguisticProsodyRefusal::Admission {
        requested,
        source,
        profile_id,
        ..
    }) = prepare_linguistic_prosody(LinguisticProsodyBasis::Rich(&rich), &offered)
    else {
        panic!()
    };
    assert!(matches!(requested.pitch(), LanguageProsodyPitch::Rising));
    assert_eq!(&profile_id, rising.identity());
    assert_eq!(
        source.revision_id(),
        rich.requested().source().material().revision()
    );
    let level = prepare_rich_prosody(
        &fixture.lexical,
        1,
        fixture.discourse.fact(),
        &fixture.profile,
    )
    .unwrap();
    assert!(matches!(
        prepare_linguistic_prosody(LinguisticProsodyBasis::Rich(&level), &offered),
        Err(LinguisticProsodyRefusal::Admission { .. })
    ));
}
