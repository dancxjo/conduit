#![cfg(all(feature = "semantic-bindings", feature = "kernel"))]
#[path = "common/carried_formant_fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "common/frequency_gesture.rs"]
mod frequency;
#[path = "common/shared_projection_model.rs"]
mod model;
use conduit_audio::*;
use conduit_speech::{
    complete_carrier::*, intent_frequency_trajectory::*, linguistic_rate::*, semantic::*,
};

#[test]
fn supplied_targets_observations_and_rate_remain_on_both_model_entrances() {
    let f = fixture::Fixture::new();
    let inventory = f.inventory().unwrap();
    let correspondence = f.correspondence(&inventory).unwrap();
    let structure = f.structure();
    let evidence = f.complete_evidence();
    let scope = SpeechAcousticScope::utterance(
        SpeechAcousticCarrierBasis::new(
            frequency::anchor(),
            inventory.basis().clone(),
            f.alignment.source().clone(),
        )
        .unwrap(),
    )
    .unwrap();
    let set = frequency::set(
        vec![frequency::target(
            scope.clone(),
            SpeechFrequencyRole::Fundamental,
            "F0",
            "complete",
            0,
            120,
            (0, 3),
        )],
        AudioControlConflictPolicy::RefuseOverlap,
    );
    let gestures = prepare_frequency_gestures(&correspondence, &structure, &set).unwrap();
    let observations = [(
        0,
        SpeechFrequencyObservation::measured(frequency::observation(
            scope,
            SpeechFrequencyRole::Fundamental,
            SpeechFrequencyErrorSpecification::unknown(),
        ))
        .unwrap(),
    )];
    let target = SpeechLinguisticRateTarget::new(
        SpeechLinguisticRateProfile::new(1).unwrap(),
        fixture::evidence(),
        SpeechLinguisticRateScope::new(
            frequency::anchor(),
            f.phones.basis().clone(),
            inventory.basis().clone(),
            f.alignment.source().clone(),
        )
        .unwrap(),
        AudioExactTimeOffset::new(4, 2).unwrap(),
        2,
    )
    .unwrap();
    let rate = prepare_linguistic_rate(&correspondence, &structure, &frequency::anchor(), &target)
        .unwrap();
    let context = |acoustics| {
        PreparedCompleteSpeechCarrier::prepare(
            &correspondence,
            &structure,
            &evidence.lexical,
            &f.word_phonemes,
            &evidence.request,
            &evidence.result,
            acoustics,
        )
        .unwrap()
    };
    let absent = context(CompleteCarrierAcoustics::not_supplied());
    let targets = context(CompleteCarrierAcoustics {
        frequency: Some(&gestures),
        rate: Some(&rate),
        ..CompleteCarrierAcoustics::not_supplied()
    });
    let observed = context(CompleteCarrierAcoustics {
        frequency: Some(&gestures),
        rate: Some(&rate),
        frequency_observations: &observations,
        ..CompleteCarrierAcoustics::not_supplied()
    });
    assert_ne!(absent.identity(), targets.identity());
    assert_ne!(targets.identity(), observed.identity());
    let absent_model = model::terminal_evidence(&absent).unwrap();
    let observed_model = model::terminal_evidence(&observed).unwrap();
    assert_ne!(absent_model.source_identity, observed_model.source_identity);
    assert_eq!(
        absent_model.correspondence_identity,
        observed_model.correspondence_identity
    );
    assert_eq!(observed_model.original_bytes, observed.canonical_bytes());
    assert!(core::ptr::eq(
        observed.acoustics().frequency.unwrap(),
        &gestures
    ));
    assert!(core::ptr::eq(observed.acoustics().rate.unwrap(), &rate));
    // Equal values from another admission do not authorize mixing owner receipts.
    let other = f.correspondence(&inventory).unwrap();
    assert!(PreparedCompleteSpeechCarrier::prepare(
        &other,
        &structure,
        &evidence.lexical,
        &f.word_phonemes,
        &evidence.request,
        &evidence.result,
        CompleteCarrierAcoustics {
            frequency: Some(&gestures),
            ..CompleteCarrierAcoustics::not_supplied()
        },
    )
    .is_err());
    assert!(PreparedCompleteSpeechCarrier::prepare(
        &correspondence,
        &structure,
        &evidence.lexical,
        &f.word_phonemes,
        &evidence.request,
        &evidence.result,
        CompleteCarrierAcoustics {
            frequency_observations: &observations,
            ..CompleteCarrierAcoustics::not_supplied()
        },
    )
    .is_err());
}
