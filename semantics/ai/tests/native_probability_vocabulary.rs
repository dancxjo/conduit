use conduit_ai::{
    DrawRelationship, LogProbability, LogScoreKind, ProbabilisticDisposition,
    ProbabilityClaimProfile, ProbabilityDigest, ProbabilityRefusal, ProbabilitySummary,
    RandomnessProfile, StochasticProvenance,
};
use conduit_form::rust_binding::NativeRustBinding;

fn round_trip<T>(value: T)
where
    T: NativeRustBinding + Clone + core::fmt::Debug + PartialEq,
{
    let structured = value.clone().into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn probability_vocabulary_has_native_identity_and_exact_round_trips() {
    round_trip(RandomnessProfile::Deterministic);
    round_trip(RandomnessProfile::explicit_seed(42).unwrap());
    round_trip(RandomnessProfile::provider_chosen("provider-draw/7".into(), 7).unwrap());

    round_trip(DrawRelationship::Independent);
    round_trip(DrawRelationship::correlated("shared-noise@1".into()).unwrap());

    round_trip(ProbabilisticDisposition::Exact);
    round_trip(ProbabilisticDisposition::approximate("finite-posterior@1".into()).unwrap());
    round_trip(ProbabilisticDisposition::truncated(8, 4).unwrap());

    for value in [
        LogScoreKind::ProbabilityMass,
        LogScoreKind::Density,
        LogScoreKind::UnnormalizedScore,
    ] {
        round_trip(value);
    }

    round_trip(
        LogProbability::new(
            -250_000,
            LogScoreKind::ProbabilityMass,
            ProbabilityDigest::new([4; 32]).unwrap(),
            StochasticProvenance::new(
                ProbabilityDigest::new([1; 32]).unwrap(),
                None,
                ProbabilityDigest::new([3; 32]).unwrap(),
                RandomnessProfile::Deterministic,
                DrawRelationship::Independent,
            )
            .unwrap(),
            ProbabilisticDisposition::Exact,
        )
        .unwrap(),
    );

    for value in [
        ProbabilityRefusal::MissingIdentity,
        ProbabilityRefusal::InvalidDisposition,
        ProbabilityRefusal::EmptySamples,
        ProbabilityRefusal::SampleCountOverflow,
        ProbabilityRefusal::InvalidSample,
        ProbabilityRefusal::ShapeMismatch,
        ProbabilityRefusal::WeightCountMismatch,
        ProbabilityRefusal::InvalidWeightSum,
        ProbabilityRefusal::InvalidVariance,
        ProbabilityRefusal::InvalidCovariance,
        ProbabilityRefusal::CovarianceDimensionOverflow,
        ProbabilityRefusal::InvalidLogProbability,
    ] {
        round_trip(value);
    }

    round_trip(
        StochasticProvenance::new(
            ProbabilityDigest::new([1; 32]).unwrap(),
            Some(ProbabilityDigest::new([2; 32]).unwrap()),
            ProbabilityDigest::new([3; 32]).unwrap(),
            RandomnessProfile::explicit_seed(42).unwrap(),
            DrawRelationship::Independent,
        )
        .unwrap(),
    );
    for profile in [
        ProbabilityClaimProfile::Samples,
        ProbabilityClaimProfile::WeightedSamples,
        ProbabilityClaimProfile::TrajectoryAlternatives,
    ] {
        round_trip(profile);
    }
    round_trip(
        ProbabilitySummary::new(
            ProbabilityClaimProfile::WeightedSamples,
            2,
            ProbabilityDigest::new([1; 32]).unwrap(),
            ProbabilityDigest::new([3; 32]).unwrap(),
            RandomnessProfile::explicit_seed(42).unwrap(),
            ProbabilisticDisposition::Exact,
        )
        .unwrap(),
    );
    assert!(!include_str!("../src/probability.rs")
        .contains(concat!("pub struct ", "StochasticProvenance")));
    assert!(!include_str!("../src/probability.rs")
        .contains(concat!("pub struct ", "ProbabilitySummary")));
    assert!(
        !include_str!("../src/probability.rs").contains(concat!("pub struct ", "LogProbability"))
    );
}

#[test]
fn stochastic_profile_text_is_bounded_by_the_native_type() {
    assert!(RandomnessProfile::provider_chosen(String::new(), 1).is_err());
    assert!(DrawRelationship::correlated("x".repeat(129)).is_err());
    assert!(ProbabilisticDisposition::approximate("x".repeat(129)).is_err());
}
