use conduit_ai::*;
use conduit_core::{
    BoundedResourceRef, KindId, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity,
};
use conduit_data::{
    DatasetDescriptor, DatasetSplitMembership, TensorAxisRole, TensorElement,
    CORPUS_MANIFEST_PROFILE,
};

pub fn resource(identity: u8, profile: &str, bytes: u64) -> BoundedResourceRef {
    BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([identity; 32]),
        content_profile: KindId::from(profile),
        access_class: ResourceClassId::from("training-store/read@1"),
        extent: ResourceExtent { bytes, items: None },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([identity + 32; 32]),
            expires_at: None,
        },
    }
}

fn tensor_constraint() -> ModelTensorConstraint {
    ModelTensorConstraint {
        elements: vec![TensorElement::F32],
        axes: vec![
            ModelAxisConstraint {
                role: TensorAxisRole::Time,
                dimension: ModelDimensionConstraint::Bounded {
                    minimum: 1,
                    maximum: 256,
                },
            },
            ModelAxisConstraint {
                role: TensorAxisRole::Feature,
                dimension: ModelDimensionConstraint::Fixed(12),
            },
        ],
        maximum_bytes: 12_288,
    }
}

pub fn signature() -> ModelSignature {
    let value = ModelValueConstraint::SampledSignal(tensor_constraint());
    ModelSignature {
        identity: "tongues/shared-latent@1".into(),
        compatibility_version: 1,
        operations: vec![
            ModelOperation::Encode,
            ModelOperation::Decode,
            ModelOperation::Evaluate,
            ModelOperation::Train,
        ],
        inputs: vec![ModelPortConstraint {
            identity: "observation".into(),
            semantic_kind: "science/observation-set@1".into(),
            presence: ModelPortPresence::Required,
            value: value.clone(),
        }],
        outputs: vec![ModelPortConstraint {
            identity: "prediction".into(),
            semantic_kind: "science/probability-samples@1".into(),
            presence: ModelPortPresence::Required,
            value,
        }],
    }
}

pub fn artifact(signature: &ModelSignature) -> ModelArtifact {
    ModelArtifact {
        architecture_profile: "tongues/shared-latent-reference@1".into(),
        format_profile: "model/artifact/reference-matrix@1".into(),
        precision_profile: "number/ieee754-f32-le".into(),
        state_schema_version: 1,
        signature_identity: signature.semantic_digest().unwrap(),
        content: resource(1, "model/artifact/reference-matrix@1", 4096),
    }
}

pub fn corpus() -> (DatasetDescriptor, DatasetSplitMembership) {
    let dataset = DatasetDescriptor {
        identity: [2; 32],
        schema_profile: "science/paired-audio-ema@1".into(),
        citation_identity: Some("doi/10.synthetic.tongues".into()),
        license_profile: Some("license/research-example@1".into()),
        example_count: 4,
        manifest: resource(3, CORPUS_MANIFEST_PROFILE, 1024),
        shards: vec![resource(4, "data/corpus-shard@1", 8192)],
        split_identities: vec!["train".into(), "evaluation".into()],
    };
    let split = DatasetSplitMembership {
        dataset_identity: dataset.identity,
        split_identity: "train".into(),
        examples: vec![[10; 32], [11; 32], [12; 32]],
    };
    (dataset, split)
}

fn objectives() -> Vec<TrainingObjective> {
    [
        ("acoustic-reconstruction", 1_000_000, "loss/acoustic", true),
        (
            "articulatory-reconstruction",
            1_000_000,
            "loss/articulatory",
            true,
        ),
        ("acoustic-to-articulatory", 500_000, "loss/a-to-ema", true),
        ("articulatory-to-acoustic", 500_000, "loss/ema-to-a", true),
        ("paired-latent-agreement", 250_000, "loss/latent", true),
        ("probabilistic-regularization", 100_000, "loss/kl", true),
        // Plausible inverse consistency, not false unique articulation.
        (
            "acoustically-equivalent-cycle",
            125_000,
            "loss/plausibility",
            true,
        ),
        ("held-out-log-score", 0, "metric/log-score", false),
    ]
    .into_iter()
    .map(|(role, weight, output, optimize)| TrainingObjective {
        role: role.into(),
        weight_millionths: weight,
        configuration_identity: format!("tongues/{role}@1"),
        output_identity: output.into(),
        participation: if optimize {
            ObjectiveParticipation::Optimize
        } else {
            ObjectiveParticipation::ObserveOnly
        },
    })
    .collect()
}

pub fn session(
    artifact: &ModelArtifact,
    dataset: &DatasetDescriptor,
    split: &DatasetSplitMembership,
) -> TrainingSession {
    TrainingSession {
        identity: [5; 32],
        base_artifact_identity: artifact.content_identity(),
        base_checkpoint_identity: None,
        dataset_manifest_identity: dataset.manifest.identity.digest(),
        split_membership_identity: split.semantic_digest().unwrap(),
        objective_profile: "tongues/shared-latent-objectives@1".into(),
        objectives: objectives(),
        randomness: RandomnessProfile::ExplicitSeed(42),
        precision_profile: artifact.precision_profile.clone(),
        model_modalities: vec!["audio".into(), "ema".into()],
        missing_modality_policy: MissingModalityPolicy::PermitDeclared {
            optional_modalities: vec!["audio".into(), "ema".into()],
        },
        resources: TrainingResourceEnvelope {
            model_bytes: 4096,
            working_memory_bytes: 1_048_576,
            compute_lanes: 2,
            maximum_batch_items: 2,
            maximum_batch_bytes: 65_536,
            maximum_steps: 3,
            maximum_work_units: 10_000,
            maximum_checkpoint_bytes: 16_384,
            maximum_in_flight_steps: 1,
        },
        checkpoint_policy: CheckpointPolicy::AtCompletion,
        evaluation_policy: EvaluationPolicy::EverySteps(1),
    }
}

pub fn realization() -> HostTrainingRealization {
    HostTrainingRealization {
        implementation_identity: "host/reference-training-adapter@1".into(),
        runtime_name: "reference-linear-runtime".into(),
        runtime_version: "1".into(),
        runtime_build_identity: "build/training-proof-1".into(),
        device_profile: "cpu/two-lane".into(),
        format_profile: "model/artifact/reference-matrix@1".into(),
        precision_profile: "number/ieee754-f32-le".into(),
        deterministic_profile: "runtime/reproducible-order-not-bitwise@1".into(),
    }
}

pub fn batch(step: u8, modalities: &[&str]) -> TrainingBatch {
    TrainingBatch {
        identity: [20 + step; 32],
        dataset_identity: [2; 32],
        split_identity: "train".into(),
        example_identities: vec![[10 + step; 32]],
        present_modalities: modalities.iter().map(|value| (*value).into()).collect(),
        encoded_bytes: 4096,
        order: BatchOrder::Shuffled,
        stochastic_seed: Some(100 + u64::from(step)),
    }
}

pub fn metrics(step: u8) -> Vec<TrainingMetric> {
    vec![
        TrainingMetric {
            output_identity: "loss/acoustic".into(),
            value_millionths: 1_000_000 - i64::from(step) * 100_000,
        },
        TrainingMetric {
            output_identity: "loss/plausibility".into(),
            value_millionths: 500_000 - i64::from(step) * 10_000,
        },
    ]
}
