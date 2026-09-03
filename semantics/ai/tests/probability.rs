use conduit_ai::*;
use conduit_core::{semantic_digest, Quantity, QuantityUnit};
use conduit_data::*;

fn tensor(values: &[f32], dimensions: Vec<u64>, roles: Vec<TensorAxisRole>) -> TensorValue {
    let payload = values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect::<Vec<_>>();
    TensorValue {
        element: TensorElement::F32,
        dimensions,
        axes: roles
            .into_iter()
            .map(|role| TensorAxis {
                role,
                identity: None,
                unit: Some(QuantityUnit::One),
            })
            .collect(),
        content_digest: tensor_content_digest(&payload),
        backing: TensorBacking::Inline(payload),
    }
}

fn provenance() -> StochasticProvenance {
    StochasticProvenance {
        model_artifact_identity: [1; 32],
        checkpoint_identity: Some([2; 32]),
        query_identity: [3; 32],
        randomness: RandomnessProfile::ExplicitSeed(42),
        draws: DrawRelationship::Independent,
    }
}

fn trajectory(value: f32) -> SampledSignal {
    SampledSignal {
        clock_identity: "inference/query-clock".into(),
        start: SignalStart::SampleIndex(0),
        cadence: SignalCadence::Regular {
            samples: 100,
            per: Quantity::new(1, QuantityUnit::Second),
        },
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
    let weighted = WeightedSamples {
        alternatives: vec![first.clone(), second.clone()],
        weights: vec![600_000_000, 400_000_000],
        provenance: provenance(),
        disposition: ProbabilisticDisposition::Approximate {
            method_profile: "empirical-posterior@1".into(),
        },
    };
    weighted.validate().unwrap();
    assert_ne!(weighted.semantic_digest().unwrap(), [0; 32]);
    assert_eq!(weighted.summary().unwrap().result_count, 2);

    let mut malformed = weighted.clone();
    malformed.weights[1] = 399_999_999;
    assert_eq!(
        malformed.validate(),
        Err(ProbabilityRefusal::InvalidWeightSum)
    );
    malformed = weighted;
    malformed.weights.pop();
    assert_eq!(
        malformed.validate(),
        Err(ProbabilityRefusal::WeightCountMismatch)
    );

    let overflow = ProbabilitySampleSet {
        alternatives: vec![first; MAXIMUM_PROBABILITY_SAMPLES + 1],
        provenance: provenance(),
        disposition: ProbabilisticDisposition::Exact,
    };
    assert_eq!(
        overflow.validate(),
        Err(ProbabilityRefusal::SampleCountOverflow)
    );
}

#[test]
fn moments_covariance_log_scores_and_truncation_refuse_malformed_claims() {
    let mean = tensor(&[0.0, 1.0], vec![2], vec![TensorAxisRole::Feature]);
    let variance = tensor(&[0.1, 0.2], vec![2], vec![TensorAxisRole::Feature]);
    MeanVariance {
        mean: mean.clone(),
        variance,
        provenance: provenance(),
        disposition: ProbabilisticDisposition::Exact,
    }
    .validate()
    .unwrap();
    let mut covariance = MeanCovariance {
        mean,
        covariance: tensor(
            &[1.0, 0.2, 0.2, 1.0],
            vec![2, 2],
            vec![TensorAxisRole::Feature, TensorAxisRole::Feature],
        ),
        provenance: provenance(),
        disposition: ProbabilisticDisposition::Approximate {
            method_profile: "finite-sample-covariance@1".into(),
        },
    };
    covariance.validate().unwrap();
    covariance.covariance = tensor(&[1.0, 0.2], vec![2], vec![TensorAxisRole::Feature]);
    assert_eq!(
        covariance.validate(),
        Err(ProbabilityRefusal::InvalidCovariance)
    );

    let invalid_mass = LogProbability {
        natural_log_millionths: 1,
        score_kind: LogScoreKind::ProbabilityMass,
        support_identity: [4; 32],
        provenance: provenance(),
        disposition: ProbabilisticDisposition::Exact,
    };
    assert_eq!(
        invalid_mass.validate(),
        Err(ProbabilityRefusal::InvalidLogProbability)
    );

    let truncated = ProbabilitySampleSet {
        alternatives: vec![tensor(&[1.0], vec![1], vec![TensorAxisRole::Feature])],
        provenance: provenance(),
        disposition: ProbabilisticDisposition::Truncated {
            retained_samples: 2,
            requested_samples: 3,
        },
    };
    assert_eq!(
        truncated.validate(),
        Err(ProbabilityRefusal::InvalidDisposition)
    );
}

#[test]
fn one_observation_yields_multiple_plausible_articulations_not_one_truth() {
    let alternatives = TrajectoryAlternatives {
        observation_identity: semantic_digest("test/synthetic-audio@1", b"observation"),
        plausible_alternatives: vec![trajectory(-0.4), trajectory(0.0), trajectory(0.4)],
        provenance: provenance(),
        disposition: ProbabilisticDisposition::Approximate {
            method_profile: "conditional-sampler@1".into(),
        },
    };
    alternatives.validate().unwrap();
    assert_eq!(alternatives.plausible_alternatives.len(), 3);
    assert_eq!(alternatives.summary().unwrap().result_count, 3);
    assert_eq!(
        alternatives.provenance.randomness,
        RandomnessProfile::ExplicitSeed(42)
    );
    assert_ne!(alternatives.semantic_digest().unwrap(), [0; 32]);

    let mut misaligned = alternatives;
    misaligned.plausible_alternatives[2].start = SignalStart::SampleIndex(1);
    assert_eq!(
        misaligned.validate(),
        Err(ProbabilityRefusal::ShapeMismatch)
    );
}

#[test]
fn model_signature_declares_a_bounded_probabilistic_signal_output() {
    let constraint = ModelTensorConstraint {
        elements: vec![TensorElement::F32],
        axes: vec![
            ModelAxisConstraint {
                role: TensorAxisRole::Time,
                dimension: ModelDimensionConstraint::Bounded {
                    minimum: 1,
                    maximum: 100,
                },
            },
            ModelAxisConstraint {
                role: TensorAxisRole::SpatialCoordinate,
                dimension: ModelDimensionConstraint::Fixed(2),
            },
        ],
        maximum_bytes: 800,
    };
    let signature = ModelSignature {
        identity: "tongues/inverse-articulation@1".into(),
        compatibility_version: 1,
        operations: vec![ModelOperation::Sample, ModelOperation::LogProbability],
        inputs: vec![ModelPortConstraint {
            identity: "audio".into(),
            semantic_kind: "data/sampled-signal@1".into(),
            presence: ModelPortPresence::Required,
            value: ModelValueConstraint::SampledSignal(constraint.clone()),
        }],
        outputs: vec![ModelPortConstraint {
            identity: "articulation-alternatives".into(),
            semantic_kind: "probability/trajectory-alternatives@1".into(),
            presence: ModelPortPresence::Required,
            value: ModelValueConstraint::ProbabilisticSignal(constraint),
        }],
    };
    signature.validate().unwrap();
    assert_ne!(signature.semantic_digest().unwrap(), [0; 32]);
}
