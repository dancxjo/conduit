#![cfg(feature = "semantic-bindings")]
#[path = "common_acoustic/common.rs"]
mod common;
use common::*;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_speech::semantic::*;
use conduit_speech::*;
fn evidence(start: u64, end: u64) -> SpeechCommonAcousticEvidence {
    SpeechCommonAcousticEvidence::new(
        SpeechFrameConfidenceSpecification::Unknown,
        SpeechDecibelLevelSpecification::Unknown,
        SpeechFrequencySpecification::NotApplicable,
        BoundedSequence::try_from_iter(vec![SpeechObservedFormant::new(
            SpeechFrequencySpecification::Unknown,
            SpeechFrequencySpecification::known(1, 500).unwrap(),
            1,
        )
        .unwrap()])
        .unwrap(),
        SpeechHarmonicitySpecification::known(
            "Tongues lacks harmonicity units".into(),
            SpeechFeatureValue::number(2.5.into()).unwrap(),
            "tongues/b037027/acoustic-frame".into(),
        )
        .unwrap(),
        SpeechProbabilitySpecification::known(4, 3).unwrap(),
        provenance(),
        Some(SpeechSegmentSpan::seconds(1.0.into(), 0.0.into()).unwrap()),
        SpeechCommonAcousticSpan::new(anchor(), time(end, 1), time(start, 1)).unwrap(),
        SpeechSpectralCentroidSpecification::known(1, 0).unwrap(),
        SpeechSpectralTiltSpecification::Unknown,
        BoundedSequence::try_from_iter(vec![SpeechUnsupportedAcousticField::new(
            "energy_db".into(),
            "No declared upstream reference; do not infer SPL".into(),
            FeatureSpecification::known(SpeechFeatureValue::number(2.5.into()).unwrap()).unwrap(),
            "tongues/b037027/acoustic-frame".into(),
        )
        .unwrap()])
        .unwrap(),
        BoundedSequence::try_from_iter(vec![SpeechDeclaredAcousticVector::new(
            "mfcc".into(),
            BoundedSequence::try_from_iter(vec![1.0f32.into(), (-2.0f32).into()]).unwrap(),
            provenance(),
            "tongues/b037027/acoustic-vector".into(),
            "Source kind only; numeric interpretation unsupported".into(),
        )
        .unwrap()])
        .unwrap(),
        SpeechProbabilitySpecification::Unknown,
        SpeechZeroCrossingSpecification::known(
            "positive-going crossings per elapsed second".into(),
            1,
            100,
        )
        .unwrap(),
    )
    .unwrap()
}
#[test]
fn observed_evidence_retains_independent_uncertainty_units_and_uninterpreted_harmonicity() {
    let original = evidence(0, 1);
    let frame = original.clone().encode().unwrap();
    let r = prepare_speech_acoustic_evidence(&frame).unwrap();
    assert_eq!(r.original(), &original);
    assert_eq!(r.original_canonical(), frame);
    reproduce(r.executions());
    assert_eq!(
        r.original().unsupported_fields().as_slice()[0].field(),
        "energy_db"
    );
    assert!(r.original().source_span().is_some());
    assert_eq!(
        r.original().vectors().as_slice()[0]
            .original()
            .as_slice()
            .len(),
        2
    );
    assert_eq!(
        r.original().formants().as_slice()[0].bandwidth_hz(),
        &SpeechFrequencySpecification::Unknown
    );
    assert!(matches!(
        r.original().formants().as_slice()[0].center_hz(),
        SpeechFrequencySpecification::Known(_)
    ));
    assert!(matches!(
        r.require_interpreted_measurements(),
        Err(SpeechCommonAcousticRefusal::UnsupportedMeasurementProfile)
    ));
    assert!(prepare_speech_acoustic_evidence(&evidence(2, 1).encode().unwrap()).is_err());
}
#[test]
fn static_native_type_and_representative_value_sizes_are_measured_without_flow_claim() {
    for (name, ty) in [
        (
            "timing_pitch",
            SpeechTimingPitchTargets::semantic_type().unwrap(),
        ),
        (
            "linguistic_rate",
            SpeechLinguisticRateTargets::semantic_type().unwrap(),
        ),
        (
            "intensity",
            SpeechIntensityTargets::semantic_type().unwrap(),
        ),
        ("formants", SpeechFormantTargets::semantic_type().unwrap()),
        (
            "voice_quality",
            SpeechVoiceQualityTargets::semantic_type().unwrap(),
        ),
        (
            "evidence",
            SpeechCommonAcousticEvidence::semantic_type().unwrap(),
        ),
        (
            "probability_curve",
            SpeechProbabilityCurve::semantic_type().unwrap(),
        ),
        (
            "rate_curve",
            SpeechSyllabicRateCurve::semantic_type().unwrap(),
        ),
    ] {
        let bytes = ty.canonical_bytes().unwrap();
        assert!(bytes.len() < 65_536);
        assert_eq!(
            conduit_core::StructuredInfoType::from_canonical_bytes(&bytes).unwrap(),
            ty
        );
        println!("{name} Type={}", bytes.len());
    }
    println!("evidence value={}", evidence(0, 1).encode().unwrap().len());
}

#[test]
fn each_factored_target_component_actually_native_roundtrips() {
    fn roundtrip<T: NativeRustBinding + Clone + PartialEq + core::fmt::Debug>(
        name: &str,
        original: T,
    ) {
        let bytes = original.clone().encode().unwrap();
        assert_eq!(T::decode(&bytes).unwrap(), original);
        println!("{name} value={}", bytes.len());
    }
    roundtrip(
        "timing_pitch",
        SpeechTimingPitchTargets::new(
            anchor(),
            SpeechDurationSpecification::Unknown,
            SpeechDurationCurveSpecification::Unknown,
            SpeechAudioTrajectorySpecification::Unknown,
            provenance(),
            scope(),
        )
        .unwrap(),
    );
    roundtrip(
        "linguistic_rate",
        SpeechLinguisticRateTargets::new(
            anchor(),
            provenance(),
            scope(),
            SpeechRateCurveSpecification::Unknown,
        )
        .unwrap(),
    );
    roundtrip(
        "intensity",
        SpeechIntensityTargets::new(
            SpeechAudioTrajectorySpecification::Unknown,
            anchor(),
            SpeechDecibelCurveSpecification::Unknown,
            provenance(),
            scope(),
        )
        .unwrap(),
    );
    roundtrip(
        "formants",
        SpeechFormantTargets::new(anchor(), BoundedSequence::new(), provenance(), scope()).unwrap(),
    );
    roundtrip(
        "voice_quality",
        SpeechVoiceQualityTargets::new(
            anchor(),
            SpeechProbabilityCurveSpecification::Unknown,
            provenance(),
            scope(),
            SpeechSpectralTiltCurveSpecification::Unknown,
            SpeechProbabilityCurveSpecification::Unknown,
        )
        .unwrap(),
    );
}

#[path = "common_acoustic/probability_parent_law.rs"]
mod probability_parent_law;
#[test]
fn variable_and_gradient_cannot_reinject_invalid_known_parent_through_evidence() {
    for candidate in probability_parent_law::forged_candidates() {
        for field in ["voicing_probability", "periodicity"] {
            let original = evidence(0, 1).into_structured().unwrap();
            let forged = probability_parent_law::replace_record(original, field, candidate.clone());
            assert!(prepare_speech_acoustic_evidence(&forged.canonical_bytes().unwrap()).is_err());
        }
    }
}
