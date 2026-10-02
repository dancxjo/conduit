use conduit_ai::*;
use conduit_core::{
    BoundedResourceRef, KindId, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity,
};
use conduit_data::{
    tensor_content_digest, SampledSignal, SignalCadence, SignalContinuity, SignalStart, TensorAxis,
    TensorAxisRole, TensorBacking, TensorElement, TensorValue,
};
use conduit_plot::rust_binding::{BoundedBytes, BoundedSequence, NativeRustBinding};

fn constraint() -> ModelValueConstraint {
    ModelValueConstraint::sampled_signal(
        ModelTensorConstraint::from_parts(
            vec![TensorElement::F32],
            vec![
                ModelAxisConstraint {
                    role: TensorAxisRole::Time,
                    dimension: ModelDimensionConstraint::bounded(8, 1).unwrap(),
                },
                ModelAxisConstraint {
                    role: TensorAxisRole::Feature,
                    dimension: ModelDimensionConstraint::fixed(2).unwrap(),
                },
            ],
            64,
        )
        .unwrap(),
    )
    .unwrap()
}

fn callable_signature() -> ModelSignature {
    let tensor = match constraint() {
        ModelValueConstraint::SampledSignal(value) => value.constraint().clone(),
        _ => unreachable!(),
    };
    ModelSignature::from_parts(
        "tongues/joint-callable".into(),
        1,
        vec![
            ModelOperation::Infer,
            ModelOperation::Sample,
            ModelOperation::Decode,
        ],
        vec![ModelPortConstraint::from_parts(
            "evidence".into(),
            "relation/evidence@1".into(),
            ModelPortPresence::Required,
            constraint(),
        )
        .unwrap()],
        vec![ModelPortConstraint::from_parts(
            "result".into(),
            "relation/result@1".into(),
            ModelPortPresence::Required,
            ModelValueConstraint::probabilistic_signal(tensor).unwrap(),
        )
        .unwrap()],
    )
    .unwrap()
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
    let probabilistic = RelationResultProfile::probabilistic(4).unwrap();
    ModelRelationSignature::from_parts(
        "tongues/joint-relation".into(),
        1,
        signature.semantic_digest().unwrap(),
        [
            "acoustic-observation",
            "articulatory-observation",
            "latent-dynamics",
            "speaker-context",
        ]
        .into_iter()
        .map(|identity| {
            RelationVariable::from_parts(
                identity.into(),
                format!("tongues/{identity}"),
                constraint(),
            )
            .unwrap()
        })
        .collect(),
        vec![
            SupportedRelationQuery::new(
                variables(&["acoustic-observation"]),
                variables(&["articulatory-observation"]),
                RelationQueryMode::InferPosterior,
                probabilistic.clone(),
                100,
                256,
            )
            .unwrap(),
            SupportedRelationQuery::new(
                variables(&["articulatory-observation"]),
                variables(&["acoustic-observation"]),
                RelationQueryMode::DecodeGenerate,
                probabilistic.clone(),
                100,
                256,
            )
            .unwrap(),
            SupportedRelationQuery::new(
                variables(&["acoustic-observation", "articulatory-observation"]),
                variables(&["latent-dynamics"]),
                RelationQueryMode::InferPosterior,
                probabilistic,
                100,
                256,
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

fn variables(values: &[&str]) -> RelationVariableIdentities {
    let values = values
        .iter()
        .map(|value| RelationVariableIdentity::new((*value).into()).unwrap());
    RelationVariableIdentities::new(BoundedSequence::try_from_iter(values).unwrap()).unwrap()
}

fn signal(byte: u8) -> SampledSignal {
    signal_with_role(byte, TensorAxisRole::Feature)
}

fn signal_with_role(byte: u8, feature_role: TensorAxisRole) -> SampledSignal {
    let bytes = vec![byte; 24];
    SampledSignal {
        clock_identity: "corpus/aligned".into(),
        start: SignalStart::at_sample(0),
        cadence: SignalCadence::regular(
            conduit_core::Quantity::new(1, conduit_core::QuantityUnit::Millisecond),
            1,
        )
        .unwrap(),
        sample_count: 3,
        continuity: SignalContinuity::Continuous,
        samples: TensorValue {
            element: TensorElement::F32,
            dimensions: BoundedSequence::try_from_iter([3, 2]).unwrap(),
            axes: BoundedSequence::try_from_iter([
                TensorAxis {
                    role: TensorAxisRole::Time,
                    identity: Some("frame".into()),
                    unit: Some(conduit_core::QuantityUnit::Millisecond),
                },
                TensorAxis {
                    role: feature_role,
                    identity: Some("observation".into()),
                    unit: None,
                },
            ])
            .unwrap(),
            content_digest: tensor_content_digest(&bytes),
            backing: TensorBacking::Inline(BoundedBytes::new(&bytes).unwrap()),
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
    query_with_signal(
        artifact,
        relation,
        evidence,
        target,
        mode,
        byte,
        signal(byte),
    )
}

fn query_with_signal(
    artifact: &ModelArtifact,
    relation: &ModelRelationSignature,
    evidence: &str,
    target: &str,
    mode: RelationQueryMode,
    byte: u8,
    signal: SampledSignal,
) -> RelationQuery {
    RelationQuery::from_parts(
        [byte.max(1); 32],
        artifact.content_identity(),
        Some([82; 32]),
        relation.semantic_digest().unwrap(),
        vec![RelationEvidence::from_parts(
            evidence.into(),
            RelationValue::sampled_signal(signal).unwrap(),
        )
        .unwrap()],
        vec![target.into()],
        mode,
        RelationResultProfile::probabilistic(4).unwrap(),
        RandomnessProfile::explicit_seed(42).unwrap(),
        80,
        200,
    )
    .unwrap()
}

fn candidate(target: &str, byte: u8) -> HostRelationTerminal {
    HostRelationTerminal::Candidate(Box::new(RelationCandidate {
        outputs: vec![RelationCandidateOutput::new(
            RelationVariableIdentity::new(target.into()).unwrap(),
            ProbabilityDigest::new([byte; 32]).unwrap(),
            ProbabilisticDisposition::approximate("conditional-samples".into()).unwrap(),
            4,
        )
        .unwrap()],
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
fn native_relation_family_round_trips_and_owns_intrinsic_bounds() {
    let callable = callable_signature();
    let artifact = artifact(&callable);
    let relation = relation(&callable);
    let query = query(
        &artifact,
        &relation,
        "acoustic-observation",
        "articulatory-observation",
        RelationQueryMode::InferPosterior,
        17,
    );
    let structured = relation.clone().into_structured().unwrap();
    assert_eq!(
        ModelRelationSignature::from_structured(structured).unwrap(),
        relation
    );
    let structured = query.clone().into_structured().unwrap();
    assert_eq!(RelationQuery::from_structured(structured).unwrap(), query);

    assert!(
        RelationVariable::from_parts("x".repeat(129), "relation/value@1".into(), constraint(),)
            .is_err()
    );
    assert!(ModelRelationSignature::from_parts(
        "relation/too-many-variables@1".into(),
        1,
        callable.semantic_digest().unwrap(),
        (0..33)
            .map(|index| {
                RelationVariable::from_parts(
                    format!("v-{index}"),
                    "relation/value@1".into(),
                    constraint(),
                )
                .unwrap()
            })
            .collect(),
        vec![SupportedRelationQuery::from_parts(
            vec!["v-0".into()],
            vec!["v-1".into()],
            RelationQueryMode::InferPosterior,
            RelationResultProfile::Deterministic,
            1,
            1,
        )
        .unwrap()],
    )
    .is_err());
    assert!(RelationQuery::from_parts(
        [1; 32],
        artifact.content_identity(),
        None,
        relation.semantic_digest().unwrap(),
        Vec::new(),
        vec!["articulatory-observation".into()],
        RelationQueryMode::InferPosterior,
        RelationResultProfile::Deterministic,
        RandomnessProfile::Deterministic,
        1,
        1,
    )
    .is_err());
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
        RelationResultProfile::probabilistic(4).unwrap()
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

    let wrong_shape = query_with_signal(
        &artifact,
        &relation,
        "acoustic-observation",
        "articulatory-observation",
        RelationQueryMode::InferPosterior,
        15,
        signal_with_role(15, TensorAxisRole::Channel),
    );
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
fn terminal_and_bounds_are_finite_and_do_not_make_results() {
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
