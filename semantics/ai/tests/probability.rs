use conduit_ai::*;
use conduit_core::{semantic_digest, Quantity, QuantityUnit};
use conduit_data::*;
use conduit_form::rust_binding::{BoundedBytes, BoundedSequence, NativeRustBinding};

fn assert_native_round_trip<T>(value: &T)
where
    T: NativeRustBinding + Clone + core::fmt::Debug + PartialEq,
{
    let structured = value.clone().into_structured().unwrap();
    assert_eq!(&T::from_structured(structured).unwrap(), value);
}

fn tensor(values: &[f32], dimensions: Vec<u64>, roles: Vec<TensorAxisRole>) -> TensorValue {
    let payload = values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect::<Vec<_>>();
    TensorValue {
        element: TensorElement::F32,
        dimensions: BoundedSequence::try_from_iter(dimensions).unwrap(),
        axes: BoundedSequence::try_from_iter(roles.into_iter().map(|role| TensorAxis {
            role,
            identity: None,
            unit: Some(QuantityUnit::One),
        }))
        .unwrap(),
        content_digest: tensor_content_digest(&payload),
        backing: TensorBacking::Inline(BoundedBytes::new(&payload).unwrap()),
    }
}

fn provenance() -> StochasticProvenance {
    StochasticProvenance::new(
        ProbabilityDigest::new([1; 32]).unwrap(),
        Some(ProbabilityDigest::new([2; 32]).unwrap()),
        ProbabilityDigest::new([3; 32]).unwrap(),
        RandomnessProfile::explicit_seed(42).unwrap(),
        DrawRelationship::Independent,
    )
    .unwrap()
}

fn trajectory(value: f32) -> SampledSignal {
    SampledSignal {
        clock_identity: "inference/query-clock".into(),
        start: SignalStart::at_sample(0),
        cadence: SignalCadence::regular(Quantity::new(1, QuantityUnit::Second), 100).unwrap(),
        sample_count: 2,
        continuity: SignalContinuity::Continuous,
        samples: tensor(
            &[value, value + 0.1, value + 0.2, value + 0.3],
            vec![2, 2],
            vec![TensorAxisRole::Time, TensorAxisRole::SpatialCoordinate],
        ),
    }
}

#[test]
fn weighted_alternatives_are_finite_normalized_and_seeded() {
    let first = tensor(&[1.0, 2.0], vec![2], vec![TensorAxisRole::Feature]);
    let second = tensor(&[1.2, 1.8], vec![2], vec![TensorAxisRole::Feature]);
    let weighted = WeightedSamples::new(
        BoundedSequence::try_from_iter([first.clone(), second.clone()]).unwrap(),
        BoundedSequence::try_from_iter([600_000_000, 400_000_000]).unwrap(),
        provenance(),
        ProbabilisticDisposition::approximate("empirical-posterior@1".into()).unwrap(),
    )
    .unwrap();
    weighted.validate().unwrap();
    assert_native_round_trip(&weighted);
    assert_native_round_trip(
        &ProbabilitySample::new(first.clone(), provenance(), ProbabilisticDisposition::Exact)
            .unwrap(),
    );
    assert_native_round_trip(
        &ProbabilitySampleSet::new(
            BoundedSequence::try_from_iter([first.clone(), second]).unwrap(),
            provenance(),
            ProbabilisticDisposition::Exact,
        )
        .unwrap(),
    );
    assert_ne!(weighted.semantic_digest().unwrap(), [0; 32]);
    assert_eq!(*weighted.summary().unwrap().result_count(), 2);

    let malformed = WeightedSamples::new(
        weighted.alternatives().clone(),
        BoundedSequence::try_from_iter([600_000_000, 399_999_999]).unwrap(),
        weighted.provenance().clone(),
        weighted.disposition().clone(),
    )
    .unwrap();
    assert_eq!(
        malformed.validate(),
        Err(ProbabilityRefusal::InvalidWeightSum)
    );
    let malformed = WeightedSamples::new(
        weighted.alternatives().clone(),
        BoundedSequence::try_from_iter([600_000_000]).unwrap(),
        weighted.provenance().clone(),
        weighted.disposition().clone(),
    )
    .unwrap();
    assert_eq!(
        malformed.validate(),
        Err(ProbabilityRefusal::WeightCountMismatch)
    );

    assert!(
        BoundedSequence::<_, MAXIMUM_PROBABILITY_SAMPLES>::try_from_iter(vec![
            first;
            MAXIMUM_PROBABILITY_SAMPLES
                + 1
        ])
        .is_err()
    );
}

#[test]
fn moments_covariance_log_scores_and_truncation_refuse_malformed_claims() {
    let mean = tensor(&[0.0, 1.0], vec![2], vec![TensorAxisRole::Feature]);
    let variance = tensor(&[0.1, 0.2], vec![2], vec![TensorAxisRole::Feature]);
    let mean_variance = MeanVariance::new(
        mean.clone(),
        variance,
        provenance(),
        ProbabilisticDisposition::Exact,
    )
    .unwrap();
    mean_variance.validate().unwrap();
    assert_native_round_trip(&mean_variance);
    let covariance = MeanCovariance::new(
        mean.clone(),
        tensor(
            &[1.0, 0.2, 0.2, 1.0],
            vec![2, 2],
            vec![TensorAxisRole::Feature, TensorAxisRole::Feature],
        ),
        provenance(),
        ProbabilisticDisposition::approximate("finite-sample-covariance@1".into()).unwrap(),
    )
    .unwrap();
    covariance.validate().unwrap();
    assert_native_round_trip(&covariance);
    let covariance = MeanCovariance::new(
        mean,
        tensor(&[1.0, 0.2], vec![2], vec![TensorAxisRole::Feature]),
        covariance.provenance().clone(),
        covariance.disposition().clone(),
    )
    .unwrap();
    assert_eq!(
        covariance.validate(),
        Err(ProbabilityRefusal::InvalidCovariance)
    );

    let invalid_mass = LogProbability::new(
        1,
        LogScoreKind::ProbabilityMass,
        ProbabilityDigest::new([4; 32]).unwrap(),
        provenance(),
        ProbabilisticDisposition::Exact,
    )
    .unwrap();
    assert_eq!(
        invalid_mass.validate(),
        Err(ProbabilityRefusal::InvalidLogProbability)
    );

    let truncated = ProbabilitySampleSet::new(
        BoundedSequence::try_from_iter([tensor(&[1.0], vec![1], vec![TensorAxisRole::Feature])])
            .unwrap(),
        provenance(),
        ProbabilisticDisposition::truncated(3, 2).unwrap(),
    )
    .unwrap();
    assert_eq!(
        truncated.validate(),
        Err(ProbabilityRefusal::InvalidDisposition)
    );
}

#[test]
fn one_observation_yields_multiple_plausible_articulations_not_one_truth() {
    let alternatives = TrajectoryAlternatives::new(
        ProbabilityDigest::new(semantic_digest("test/synthetic-audio@1", b"observation")).unwrap(),
        BoundedSequence::try_from_iter([trajectory(-0.4), trajectory(0.0), trajectory(0.4)])
            .unwrap(),
        provenance(),
        ProbabilisticDisposition::approximate("conditional-sampler@1".into()).unwrap(),
    )
    .unwrap();
    alternatives.validate().unwrap();
    assert_native_round_trip(&alternatives);
    assert_eq!(alternatives.plausible_alternatives().len(), 3);
    assert_eq!(*alternatives.summary().unwrap().result_count(), 3);
    assert_eq!(
        alternatives.provenance().randomness(),
        &RandomnessProfile::explicit_seed(42).unwrap()
    );
    assert_ne!(alternatives.semantic_digest().unwrap(), [0; 32]);

    let mut plausible_alternatives = alternatives.plausible_alternatives().clone();
    plausible_alternatives[2].start = SignalStart::at_sample(1);
    let misaligned = TrajectoryAlternatives::new(
        alternatives.observation_identity().clone(),
        plausible_alternatives,
        alternatives.provenance().clone(),
        alternatives.disposition().clone(),
    )
    .unwrap();
    assert_eq!(
        misaligned.validate(),
        Err(ProbabilityRefusal::ShapeMismatch)
    );
}

#[test]
fn model_signature_declares_a_bounded_probabilistic_signal_output() {
    let constraint = ModelTensorConstraint::from_parts(
        vec![TensorElement::F32],
        vec![
            ModelAxisConstraint {
                role: TensorAxisRole::Time,
                dimension: ModelDimensionConstraint::bounded(100, 1).unwrap(),
            },
            ModelAxisConstraint {
                role: TensorAxisRole::SpatialCoordinate,
                dimension: ModelDimensionConstraint::fixed(2).unwrap(),
            },
        ],
        800,
    )
    .unwrap();
    let signature = ModelSignature::from_parts(
        "tongues/inverse-articulation@1".into(),
        1,
        vec![ModelOperation::Sample, ModelOperation::LogProbability],
        vec![ModelPortConstraint::from_parts(
            "audio".into(),
            "data/sampled-signal@1".into(),
            ModelPortPresence::Required,
            ModelValueConstraint::sampled_signal(constraint.clone()).unwrap(),
        )
        .unwrap()],
        vec![ModelPortConstraint::from_parts(
            "articulation-alternatives".into(),
            "probability/trajectory-alternatives@1".into(),
            ModelPortPresence::Required,
            ModelValueConstraint::probabilistic_signal(constraint).unwrap(),
        )
        .unwrap()],
    )
    .unwrap();
    signature.validate().unwrap();
    assert_ne!(signature.semantic_digest().unwrap(), [0; 32]);
}
