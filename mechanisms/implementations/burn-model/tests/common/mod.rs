use conduit_ai::*;
use conduit_burn_model::burn;
use conduit_burn_model::{AuthoringDescriptor, ParameterGroup, ResourceEstimate};
use conduit_core::ComputeServiceGuarantee;
use conduit_data::{TensorAxisRole, TensorElement};

pub fn descriptor() -> AuthoringDescriptor {
    let tensor = ModelTensorConstraint::from_parts(
        vec![TensorElement::F32],
        vec![
            ModelAxisConstraint {
                role: TensorAxisRole::Batch,
                dimension: ModelDimensionConstraint::bounded(8, 1).unwrap(),
            },
            ModelAxisConstraint {
                role: TensorAxisRole::Feature,
                dimension: ModelDimensionConstraint::fixed(1).unwrap(),
            },
        ],
        32,
    )
    .unwrap();
    let port = |id: &str| {
        ModelPortConstraint::from_parts(
            id.into(),
            "fixture/regression@1".into(),
            ModelPortPresence::Required,
            ModelValueConstraint::tensor(tensor.clone()).unwrap(),
        )
        .unwrap()
    };
    AuthoringDescriptor {
        architecture: "fixture/regression@1".into(),
        config_identity: [1; 32],
        checkpoint_schema: 1,
        supported_profiles: vec!["burn/flex-cpu".into(), "burn/cuda/0".into()],
        signature: ModelSignature::from_parts(
            "fixture/regression@1".into(),
            1,
            vec![
                ModelOperation::Infer,
                ModelOperation::Train,
                ModelOperation::Evaluate,
            ],
            vec![port("x")],
            vec![port("y")],
        )
        .unwrap(),
        groups: vec![ParameterGroup {
            identity: "all".into(),
            trainable: true,
        }],
        resources: ResourceEstimate {
            model_bytes: 4096,
            optimizer_bytes: 4096,
            candidate_bytes: 16384,
            temporary_bytes: 65536,
            checkpoint_bytes: 65536,
            maximum_work_per_step: 1000,
        },
        limits: ModelComputeLimits {
            maximum_model_bytes: 4096,
            maximum_working_memory_bytes: 1024 * 1024,
            maximum_device_memory_bytes: 1024 * 1024,
            maximum_input_bytes: 32,
            maximum_output_bytes: 32,
            maximum_batch_items: 8,
            maximum_rank: 2,
            maximum_in_flight: 1,
            maximum_queue_items: 1,
            maximum_queue_bytes: 32,
            cancellation_supported: true,
            compute: ComputeCapacity {
                class: PortableComputeClass::GeneralCpu,
                minimum_lanes: 1,
                preferred_lanes: 1,
                maximum_lanes: 1,
                service: ComputeServiceGuarantee::Shared,
            },
        },
    }
}

use burn::{
    module::{Module, ParamGroup},
    nn::{Linear, LinearConfig},
    tensor::{Device, Tensor, TensorData},
};
use conduit_burn_model::{
    BurnModelDefinition, Error, ModelBatch, ObjectiveResult, TrainingContext,
};
use conduit_core::{
    BoundedResourceRef, KindId, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity,
};
use conduit_data::{
    tensor_content_digest, DatasetDescriptor, DatasetExampleIdentity, DatasetSplitMembership,
    TensorBacking, TensorValue, CORPUS_MANIFEST_PROFILE,
};
use conduit_plot::rust_binding::{BoundedBytes, BoundedSequence};

#[derive(Module, Debug)]
pub struct Regression {
    pub linear: Linear,
}
#[derive(Clone)]
pub struct RegressionDefinition;
impl BurnModelDefinition for RegressionDefinition {
    type Model = Regression;
    fn descriptor(&self) -> AuthoringDescriptor {
        descriptor()
    }
    fn initialize(&self, device: &Device) -> Result<Regression, Error> {
        Ok(Regression {
            linear: LinearConfig::new(1, 1)
                .with_initializer(burn::nn::Initializer::Zeros)
                .init(device),
        })
    }
    fn parameter_group(&self, model: &Regression, id: &str) -> Result<ParamGroup, Error> {
        if id != "all" {
            return Err(Error::InvalidDescriptor);
        }
        Ok(ParamGroup::ids_from_module(model.clone()))
    }
    fn forward(
        &self,
        model: &Regression,
        inputs: &[TensorValue],
        device: &Device,
    ) -> Result<Vec<TensorValue>, Error> {
        let result = model.linear.forward(tensor(&inputs[0], device)?);
        let rows = result.dims()[0];
        let values = result
            .into_data()
            .try_to_vec::<f32>()
            .map_err(|_| Error::InvalidTensor)?;
        Ok(vec![value(&values, rows)])
    }
    fn objective(
        &self,
        model: &Regression,
        batch: &ModelBatch,
        _: &[TrainingObjective],
        device: &Device,
    ) -> Result<ObjectiveResult, Error> {
        let prediction = model.linear.forward(tensor(&batch.inputs[0], device)?);
        let loss = (prediction - tensor(&batch.targets[0], device)?)
            .powf_scalar(2.0)
            .mean();
        let number: f32 = loss.clone().into_scalar();
        Ok(ObjectiveResult {
            loss,
            metrics: vec![TrainingMetric::new(
                TrainingObjectiveIdentity::new("loss/mse".into()).unwrap(),
                (number as f64 * 1_000_000.0).round() as i64,
            )
            .unwrap()],
        })
    }
}
fn tensor(value: &TensorValue, device: &Device) -> Result<Tensor<2>, Error> {
    let TensorBacking::Inline(bytes) = &value.backing else {
        return Err(Error::InvalidTensor);
    };
    let values = bytes
        .as_slice()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b))
        .collect::<Vec<_>>();
    Ok(Tensor::from_data(
        TensorData::new(values, [value.dimensions[0] as usize, 1]),
        device,
    ))
}
pub fn value(values: &[f32], rows: usize) -> TensorValue {
    let bytes = values
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect::<Vec<_>>();
    TensorValue {
        element: TensorElement::F32,
        dimensions: BoundedSequence::try_from_iter([rows as u64, 1]).unwrap(),
        axes: BoundedSequence::try_from_iter([TensorAxisRole::Batch, TensorAxisRole::Feature].map(
            |role| conduit_data::TensorAxis {
                role,
                identity: None,
                unit: None,
            },
        ))
        .unwrap(),
        content_digest: tensor_content_digest(&bytes),
        backing: TensorBacking::Inline(BoundedBytes::new(&bytes).unwrap()),
    }
}
fn resource(identity: u8, profile: &str, bytes: u64) -> BoundedResourceRef {
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
pub fn context() -> TrainingContext {
    let signature = descriptor().signature;
    let prepared = conduit_burn_model::PreparedBurnModel::initialize(
        RegressionDefinition,
        conduit_burn_model::DeviceRequest::Cpu,
        conduit_burn_model::OptimizerRecipe {
            learning_rate: 0.05,
            weight_decay: 0.,
            gradient_clip: 10.,
            seed: 42,
        },
    )
    .unwrap();
    let mut content = resource(
        1,
        "model/burn-safetensors@1",
        prepared.weights().len() as u64,
    );
    content.identity = ResourceSemanticIdentity::from_digest(prepared.content_identity());
    content.lifetime.version = ResourceVersionIdentity::from_digest(prepared.content_identity());
    let artifact = ModelArtifact {
        architecture_profile: "fixture/regression@1".into(),
        format_profile: "model/burn-safetensors@1".into(),
        precision_profile: "number/ieee754-f32-le".into(),
        state_schema_version: 1,
        signature_identity: signature.semantic_digest().unwrap(),
        content,
    };
    let dataset = DatasetDescriptor {
        identity: [2; 32],
        schema_profile: "fixture/regression-data@1".into(),
        citation_identity: None,
        license_profile: Some("fixture/generated@1".into()),
        example_count: 4,
        manifest: resource(3, CORPUS_MANIFEST_PROFILE, 128),
        shards: BoundedSequence::try_from_iter([resource(4, "data/corpus-shard@1", 128)]).unwrap(),
        split_identities: BoundedSequence::try_from_iter(["train".into()]).unwrap(),
    };
    let split = DatasetSplitMembership {
        dataset_identity: dataset.identity,
        split_identity: "train".into(),
        examples: DatasetSplitMembership::pages(
            [[10; 32], [11; 32], [12; 32], [13; 32]]
                .map(|id| DatasetExampleIdentity::new(id).unwrap()),
        )
        .unwrap(),
    };
    let session = TrainingSession {
        identity: [5; 32],
        base_artifact_identity: artifact.content_identity(),
        base_checkpoint_identity: None,
        dataset_manifest_identity: dataset.manifest.identity.digest(),
        split_membership_identity: split.semantic_digest().unwrap(),
        objective_profile: "fixture/mse@1".into(),
        objectives: TrainingObjectives::from_values(vec![TrainingObjective::new(
            TrainingObjectiveIdentity::new("regression".into()).unwrap(),
            1_000_000,
            TrainingObjectiveIdentity::new("objective/mse@1".into()).unwrap(),
            TrainingObjectiveIdentity::new("loss/mse".into()).unwrap(),
            ObjectiveParticipation::Optimize,
        )
        .unwrap()])
        .unwrap(),
        randomness: RandomnessProfile::explicit_seed(42).unwrap(),
        precision_profile: artifact.precision_profile.clone(),
        model_modalities: TrainingModalities::from_strings(vec!["regression".into()]).unwrap(),
        missing_modality_policy: MissingModalityPolicy::Reject,
        resources: TrainingResourceEnvelope::new(
            4096,
            1024 * 1024,
            1,
            8,
            64,
            100,
            100_000,
            65536,
            1,
        )
        .unwrap(),
        checkpoint_policy: CheckpointPolicy::EverySteps(1),
        evaluation_policy: EvaluationPolicy::EverySteps(1),
    };
    TrainingContext {
        realization: HostTrainingRealization {
            implementation_identity: "test/burn-adapter@1".into(),
            runtime_name: "Burn".into(),
            runtime_version: "0.22.0".into(),
            runtime_build_identity: "test/build@1".into(),
            device_profile: "burn/flex-cpu".into(),
            format_profile: artifact.format_profile.clone(),
            precision_profile: artifact.precision_profile.clone(),
            deterministic_profile: "seeded-order/f32-not-bitwise@1".into(),
        },
        artifact,
        dataset,
        split,
        session,
    }
}
pub fn batch() -> ModelBatch {
    ModelBatch {
        inputs: vec![value(&[-1., 0., 1., 2.], 4)],
        targets: vec![value(&[-1., 1., 3., 5.], 4)],
    }
}
pub fn request(step: u64) -> TrainStepRequest {
    TrainStepRequest {
        step,
        expected_generation: step - 1,
        admitted_work_units: 1000,
        batch: TrainingBatch {
            identity: [21; 32],
            dataset_identity: [2; 32],
            split_identity: "train".into(),
            example_identities: TrainingExampleIdentityPages::from_values(vec![
                [10; 32], [11; 32], [12; 32], [13; 32],
            ])
            .unwrap(),
            present_modalities: TrainingModalities::from_strings(vec!["regression".into()])
                .unwrap(),
            encoded_bytes: 32,
            order: BatchOrder::Stable,
            stochastic_seed: None,
        },
    }
}
