#![cfg(all(feature = "semantic-bindings", feature = "kernel"))]
#[path = "common/carried_formant_fixture.rs"]
mod fixture;
#[path = "common/shared_projection_model.rs"]
mod model;
use conduit_audio::*;
use conduit_core::Unit;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{
    complete_carrier::*, model_linguistic_projection::ModelBackendRefusal, scoped_energy::*,
    semantic::*,
};

fn anchor() -> AudioTrajectoryAnchor {
    AudioTrajectoryAnchor::new(
        AudioOriginIdentity::new(1).unwrap(),
        AudioTimelineIdentity::new(1).unwrap(),
    )
    .unwrap()
}
fn time(n: u64) -> AudioExactTimeOffset {
    AudioExactTimeOffset::new(1, n).unwrap()
}
fn provenance() -> AudioTrajectoryProvenance {
    AudioTrajectoryProvenance::new(
        AudioTrajectoryProvenanceKind::Authored,
        "complete-carrier energy reference".into(),
        None,
    )
    .unwrap()
}
fn basis() -> AudioDecibelBasis {
    AudioDecibelBasis::new(
        AudioDecibelConvention::AmplitudeTwentyLog10,
        AudioDecibelReference::new(
            1,
            "complete/relative-amplitude".into(),
            1,
            provenance(),
            AudioDecibelReferenceRole::AmplitudeMagnitude,
            Unit::One,
        )
        .unwrap(),
    )
    .unwrap()
}
fn target(scope: SpeechAcousticScope) -> SpeechScopedEnergyGestureSet {
    let value = AudioTrajectoryQuantity::amplitude(1, 2).unwrap();
    let trajectory = AudioQuantityTrajectory::new(
        anchor(),
        AudioTrajectoryEndpoints::RightContinuousFinalIncluded,
        AudioTrajectoryOutside::Refuse,
        provenance(),
        BoundedSequence::try_from_iter([AudioTrajectorySegment::new(
            time(1),
            AudioTrajectoryInterpolation::Step,
            value.clone(),
            value,
            time(0),
        )
        .unwrap()])
        .unwrap(),
    )
    .unwrap();
    SpeechScopedEnergyGestureSet::new(
        AudioControlConflictPolicy::RefuseOverlap,
        AudioControlGapPolicy::RefuseAnyGap,
        BoundedSequence::try_from_iter([SpeechScopedEnergyTarget::new(
            basis(),
            AudioScopedControlTrajectory::new(
                "energy".into(),
                AudioTrajectoryDomain::Amplitude,
                0,
                "complete/energy-target".into(),
                trajectory,
            )
            .unwrap(),
            scope,
        )
        .unwrap()])
        .unwrap(),
    )
    .unwrap()
}
fn observation(
    scope: SpeechAcousticScope,
    interpretation: SpeechEnergyObservationInterpretation,
) -> SpeechScopedEnergyObservation {
    SpeechScopedEnergyObservation::new(
        SpeechEnergyErrorSpecification::Unknown,
        fixture::confidence(),
        time(1),
        interpretation,
        fixture::evidence(),
        scope,
        time(0),
        AudioReferencedLevelRatio::new(basis(), AudioLevelRatio::amplitude(1, 1).unwrap()).unwrap(),
    )
    .unwrap()
}
#[test]
fn measured_and_inferred_energy_remain_distinct_full_carrier_evidence() {
    let f = fixture::Fixture::new();
    let inventory = f.inventory().unwrap();
    let correspondence = f.correspondence(&inventory).unwrap();
    let structure = f.structure();
    let evidence = f.complete_evidence();
    let scope = SpeechAcousticScope::utterance(
        SpeechAcousticCarrierBasis::new(
            anchor(),
            inventory.basis().clone(),
            f.alignment.source().clone(),
        )
        .unwrap(),
    )
    .unwrap();
    let supplied = target(scope.clone());
    let energy = prepare_scoped_energy(&correspondence, &structure, &supplied).unwrap();
    let measured = [(
        0,
        observation(
            scope.clone(),
            SpeechEnergyObservationInterpretation::Measured,
        ),
    )];
    let inferred = [(
        0,
        observation(scope, SpeechEnergyObservationInterpretation::Inferred),
    )];
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
    let intended = context(CompleteCarrierAcoustics {
        energy: Some(&energy),
        ..CompleteCarrierAcoustics::not_supplied()
    });
    let measured_context = context(CompleteCarrierAcoustics {
        energy: Some(&energy),
        energy_observations: &measured,
        ..CompleteCarrierAcoustics::not_supplied()
    });
    let inferred_context = context(CompleteCarrierAcoustics {
        energy: Some(&energy),
        energy_observations: &inferred,
        ..CompleteCarrierAcoustics::not_supplied()
    });
    assert_ne!(absent.identity(), intended.identity());
    assert_ne!(intended.identity(), measured_context.identity());
    assert_ne!(measured_context.identity(), inferred_context.identity());
    assert_ne!(
        measured_context.canonical_bytes(),
        inferred_context.canonical_bytes()
    );
    assert!(core::ptr::eq(
        measured_context.acoustics().energy.unwrap(),
        &energy
    ));
    let measured_model = model::terminal_evidence(&measured_context).unwrap();
    let inferred_model = model::terminal_evidence(&inferred_context).unwrap();
    assert_eq!(
        measured_model.correspondence_identity,
        inferred_model.correspondence_identity
    );
    assert_ne!(
        measured_model.source_identity,
        inferred_model.source_identity
    );
    assert_eq!(
        measured_model.original_bytes,
        measured_context.canonical_bytes()
    );
    assert_eq!(
        inferred_model.original_bytes,
        inferred_context.canonical_bytes()
    );
    assert_eq!(measured_model.source_identity, measured_context.identity());
    assert_eq!(inferred_model.source_identity, inferred_context.identity());
    for terminal in [&measured_model, &inferred_model] {
        assert_eq!(
            terminal.refusal,
            ModelBackendRefusal::NoAdmittedAcousticBack
        );
        assert_eq!(terminal.selected_backend, None);
    }
    // Identical component values admitted through another correspondence are
    // foreign owner receipts, and observations require an admitted target.
    let other = f.correspondence(&inventory).unwrap();
    assert!(PreparedCompleteSpeechCarrier::prepare(
        &other,
        &structure,
        &evidence.lexical,
        &f.word_phonemes,
        &evidence.request,
        &evidence.result,
        CompleteCarrierAcoustics {
            energy: Some(&energy),
            ..CompleteCarrierAcoustics::not_supplied()
        }
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
            energy_observations: &measured,
            ..CompleteCarrierAcoustics::not_supplied()
        }
    )
    .is_err());
}
