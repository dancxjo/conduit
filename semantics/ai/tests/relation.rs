use conduit_ai::*;
use conduit_core::{
    BoundedResourceRef, KindId, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity,
};
use conduit_data::{
    tensor_content_digest, SampledSignal, SignalCadence, SignalContinuity, SignalStart, TensorAxis,
    TensorAxisRole, TensorBacking, TensorElement, TensorValue,
};

fn constraint() -> ModelValueConstraint {
    ModelValueConstraint::SampledSignal(ModelTensorConstraint {
        elements: vec![TensorElement::F32],
        axes: vec![
            ModelAxisConstraint {
                role: TensorAxisRole::Time,
                dimension: ModelDimensionConstraint::Bounded {
                    minimum: 1,
                    maximum: 8,
                },
            },
            ModelAxisConstraint {
                role: TensorAxisRole::Feature,
                dimension: ModelDimensionConstraint::Fixed(2),
            },
        ],
        maximum_bytes: 64,
    })
}

fn callable_signature() -> ModelSignature {
    ModelSignature {
        identity: "tongues/joint-callable".into(),
        compatibility_version: 1,
        operations: vec![
            ModelOperation::Infer,
            ModelOperation::Sample,
            ModelOperation::Decode,
        ],
        inputs: vec![ModelPortConstraint {
            identity: "evidence".into(),
            semantic_kind: "relation/evidence@1".into(),
            presence: ModelPortPresence::Required,
            value: constraint(),
        }],
        outputs: vec![ModelPortConstraint {
            identity: "result".into(),
            semantic_kind: "relation/result@1".into(),
            presence: ModelPortPresence::Required,
            value: ModelValueConstraint::ProbabilisticSignal(match constraint() {
                ModelValueConstraint::SampledSignal(value) => value,
                _ => unreachable!(),
            }),
        }],
    }
}

fn artifact(signature: &ModelSignature) -> ModelArtifact {
    ModelArtifact {
        architecture_profile: "joint-latent-relation".into(),
        format_profile: "model/reference".into(),
        precision_profile: "f32".into(),
        state_schema_version: 1,
        signature_identity: signature.semantic_digest().unwrap(),
        content: BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest([81; 32]),
            content_profile: KindId::from("model/reference"),
            access_class: ResourceClassId::from("model-store/read@1"),
            extent: ResourceExtent {
                bytes: 1024,
                items: None,
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest([82; 32]),
                expires_at: None,
            },
        },
    }
}

fn relation(signature: &ModelSignature) -> ModelRelationSignature {
    let probabilistic = RelationResultProfile::Probabilistic { maximum_samples: 4 };
    ModelRelationSignature {
        identity: "tongues/joint-relation".into(),
        compatibility_version: 1,
        callable_signature_identity: signature.semantic_digest().unwrap(),
        variables: [
            "acoustic-observation",
            "articulatory-observation",
            "latent-dynamics",
            "speaker-context",
        ]
        .into_iter()
        .map(|identity| RelationVariable {
            identity: identity.into(),
            semantic_role: format!("tongues/{identity}"),
            value: constraint(),
        })
        .collect(),
        supported_queries: vec![
            SupportedRelationQuery {
                evidence_variables: vec!["acoustic-observation".into()],
                target_variables: vec!["articulatory-observation".into()],
                mode: RelationQueryMode::InferPosterior,
                result_profile: probabilistic,
                maximum_work_units: 100,
                maximum_output_bytes: 256,
            },
            SupportedRelationQuery {
                evidence_variables: vec!["articulatory-observation".into()],
                target_variables: vec!["acoustic-observation".into()],
                mode: RelationQueryMode::DecodeGenerate,
                result_profile: probabilistic,
                maximum_work_units: 100,
                maximum_output_bytes: 256,
            },
            SupportedRelationQuery {
                evidence_variables: vec![
                    "acoustic-observation".into(),
                    "articulatory-observation".into(),
                ],
                target_variables: vec!["latent-dynamics".into()],
                mode: RelationQueryMode::InferPosterior,
                result_profile: probabilistic,
                maximum_work_units: 100,
                maximum_output_bytes: 256,
            },
        ],
    }
}

fn signal(byte: u8) -> SampledSignal {
    let bytes = vec![byte; 24];
    SampledSignal {
        clock_identity: "corpus/aligned".into(),
        start: SignalStart::SampleIndex(0),
        cadence: SignalCadence::Regular {
            samples: 1,
            per: conduit_core::Quantity::new(1, conduit_core::QuantityUnit::Millisecond),
        },
        sample_count: 3,
        continuity: SignalContinuity::Continuous,
        samples: TensorValue {
            element: TensorElement::F32,
            dimensions: vec![3, 2],
            axes: vec![
                TensorAxis {
                    role: TensorAxisRole::Time,
                    identity: Some("frame".into()),
                    unit: Some(conduit_core::QuantityUnit::Millisecond),
                },
                TensorAxis {
                    role: TensorAxisRole::Feature,
                    identity: Some("observation".into()),
                    unit: None,
                },
            ],
            content_digest: tensor_content_digest(&bytes),
            backing: TensorBacking::Inline(bytes),
        },
    }
}

fn query(
    artifact: &ModelArtifact,
    relation: &ModelRelationSignature,
    evidence: &str,
    target: &str,
    mode: RelationQueryMode,
    byte: u8,
) -> RelationQuery {
    RelationQuery {
        identity: [byte.max(1); 32],
        artifact_identity: artifact.content_identity(),
        checkpoint_identity: Some([82; 32]),
        relation_signature_identity: relation.semantic_digest().unwrap(),
        evidence: vec![RelationEvidence {
            variable: evidence.into(),
            value: RelationValue::SampledSignal(signal(byte)),
        }],
        targets: vec![target.into()],
        mode,
        requested_result: RelationResultProfile::Probabilistic { maximum_samples: 4 },
        randomness: RandomnessProfile::ExplicitSeed(42),
        admitted_work_units: 80,
        maximum_output_bytes: 200,
    }
}

fn candidate(target: &str, byte: u8) -> HostRelationTerminal {
    HostRelationTerminal::Candidate(Box::new(RelationCandidate {
        outputs: vec![RelationCandidateOutput {
            target_variable: target.into(),
            value_identity: [byte; 32],
            disposition: ProbabilisticDisposition::Approximate {
                method_profile: "conditional-samples".into(),
            },
            sample_count: 4,
        }],
        consumed_work_units: 60,
        encoded_output_bytes: 128,
        realization: RelationRealization {
            implementation_identity: "host/joint-model".into(),
            runtime_name: "reference-runtime".into(),
            runtime_version: "1".into(),
            runtime_build_identity: "build/1".into(),
            device_profile: "cpu/f32".into(),
        },
    }))
}

#[test]
fn one_artifact_answers_both_non_bijective_conditional_directions() {
    let callable = callable_signature();
    let artifact = artifact(&callable);
    let relation = relation(&callable);
    relation.validate().unwrap();
    let audio_to_artic = query(
        &artifact,
        &relation,
        "acoustic-observation",
        "articulatory-observation",
        RelationQueryMode::InferPosterior,
        11,
    );
    let artic_to_audio = query(
        &artifact,
        &relation,
        "articulatory-observation",
        "acoustic-observation",
        RelationQueryMode::DecodeGenerate,
        12,
    );
    let RelationQueryOutcome::Completed(first) = relation
        .realize(
            &artifact,
            &audio_to_artic,
            candidate("articulatory-observation", 91),
        )
        .unwrap()
    else {
        panic!()
    };
    let RelationQueryOutcome::Completed(second) = relation
        .realize(
            &artifact,
            &artic_to_audio,
            candidate("acoustic-observation", 92),
        )
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(first.artifact_identity, second.artifact_identity);
    assert_ne!(
        first.query_descriptor_identity,
        second.query_descriptor_identity
    );
    assert_eq!(
        first.requested_result,
        RelationResultProfile::Probabilistic { maximum_samples: 4 }
    );
}

#[test]
fn missing_is_not_zero_and_undeclared_reverse_or_singleton_refuses() {
    let callable = callable_signature();
    let artifact = artifact(&callable);
    let relation = relation(&callable);
    let zero = query(
        &artifact,
        &relation,
        "acoustic-observation",
        "articulatory-observation",
        RelationQueryMode::InferPosterior,
        0,
    );
    assert!(relation
        .realize(&artifact, &zero, candidate("articulatory-observation", 91))
        .is_ok());
    let mut missing = zero.clone();
    missing.evidence.clear();
    assert_eq!(
        relation.realize(
            &artifact,
            &missing,
            candidate("articulatory-observation", 91)
        ),
        Err(RelationRefusal::UnsupportedQuery)
    );
    let unsupported = query(
        &artifact,
        &relation,
        "latent-dynamics",
        "acoustic-observation",
        RelationQueryMode::DecodeGenerate,
        13,
    );
    assert_eq!(
        relation.realize(
            &artifact,
            &unsupported,
            candidate("acoustic-observation", 92)
        ),
        Err(RelationRefusal::UnsupportedQuery)
    );
    let mut singleton = zero;
    singleton.requested_result = RelationResultProfile::Deterministic;
    singleton.randomness = RandomnessProfile::Deterministic;
    assert_eq!(
        relation.realize(
            &artifact,
            &singleton,
            candidate("articulatory-observation", 91)
        ),
        Err(RelationRefusal::UnsupportedQuery)
    );

    let mut wrong_shape = query(
        &artifact,
        &relation,
        "acoustic-observation",
        "articulatory-observation",
        RelationQueryMode::InferPosterior,
        15,
    );
    let RelationValue::SampledSignal(value) = &mut wrong_shape.evidence[0].value else {
        unreachable!()
    };
    value.samples.axes[1].role = TensorAxisRole::Channel;
    assert_eq!(
        relation.realize(
            &artifact,
            &wrong_shape,
            candidate("articulatory-observation", 91)
        ),
        Err(RelationRefusal::ShapeMismatch)
    );
}

#[test]
fn terminal_and_bounds_are_finite_and_do_not_fabricate_results() {
    let callable = callable_signature();
    let artifact = artifact(&callable);
    let relation = relation(&callable);
    let query = query(
        &artifact,
        &relation,
        "acoustic-observation",
        "articulatory-observation",
        RelationQueryMode::InferPosterior,
        14,
    );
    for terminal in [
        RelationTerminal::Cancelled,
        RelationTerminal::ResourceExhausted,
        RelationTerminal::ProviderLost,
        RelationTerminal::Failed,
    ] {
        assert_eq!(
            relation
                .realize(&artifact, &query, HostRelationTerminal::NoResult(terminal))
                .unwrap(),
            RelationQueryOutcome::NoResult(terminal)
        );
    }
    let mut too_much = query;
    too_much.admitted_work_units = 101;
    assert_eq!(
        relation.realize(
            &artifact,
            &too_much,
            candidate("articulatory-observation", 91)
        ),
        Err(RelationRefusal::WorkBoundExceeded)
    );
}
