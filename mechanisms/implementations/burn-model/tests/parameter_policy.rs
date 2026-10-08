#![cfg(not(feature = "cuda"))]
mod common;
use burn::{module::ParamGroup, tensor::Device};
use conduit_ai::TrainingObjective;
use conduit_burn_model::{
    burn, AuthoringDescriptor, BurnAdapter, BurnModelDefinition, Cancellation, DeviceRequest,
    DirectoryCheckpointStore, Error, ModelBatch, ObjectiveResult, OptimizerRecipe, ParameterGroup,
};
use conduit_data::TensorValue;
struct FrozenWeight;
impl BurnModelDefinition for FrozenWeight {
    type Model = common::Regression;
    fn descriptor(&self) -> AuthoringDescriptor {
        let mut descriptor = common::descriptor();
        descriptor.groups = vec![
            ParameterGroup {
                identity: "weight".into(),
                trainable: false,
            },
            ParameterGroup {
                identity: "bias".into(),
                trainable: true,
            },
        ];
        descriptor
    }
    fn initialize(&self, device: &Device) -> Result<Self::Model, Error> {
        common::RegressionDefinition.initialize(device)
    }
    fn parameter_group(&self, model: &Self::Model, id: &str) -> Result<ParamGroup, Error> {
        match id {
            "weight" => Ok(ParamGroup::from_ids(vec![model.linear.weight.id])),
            "bias" => Ok(ParamGroup::from_ids(vec![
                model.linear.bias.as_ref().unwrap().id,
            ])),
            _ => Err(Error::InvalidDescriptor),
        }
    }
    fn forward(
        &self,
        model: &Self::Model,
        inputs: &[TensorValue],
        device: &Device,
    ) -> Result<Vec<TensorValue>, Error> {
        common::RegressionDefinition.forward(model, inputs, device)
    }
    fn objective(
        &self,
        model: &Self::Model,
        batch: &ModelBatch,
        objectives: &[TrainingObjective],
        device: &Device,
    ) -> Result<ObjectiveResult, Error> {
        common::RegressionDefinition.objective(model, batch, objectives, device)
    }
}
fn adapter() -> BurnAdapter<FrozenWeight> {
    BurnAdapter::initialize(
        FrozenWeight,
        DeviceRequest::Cpu,
        OptimizerRecipe {
            learning_rate: 0.05,
            weight_decay: 0.,
            gradient_clip: 10.,
            seed: 42,
        },
        common::context(),
    )
    .unwrap()
}
#[test]
fn frozen_parameter_group_remains_frozen_after_resume() {
    let mut host = adapter();
    let cancel = Cancellation::default();
    let batch = common::batch();
    for step in 1..=5 {
        host.train_step(&common::request(step), &batch, &cancel)
            .unwrap();
    }
    let dir = tempfile::tempdir().unwrap();
    let store = DirectoryCheckpointStore::new(dir.path(), 65536, 16).unwrap();
    let checkpoint = host
        .checkpoint(
            &store,
            host.evaluate(&common::request(1).batch, &batch)
                .unwrap()
                .metrics,
            &cancel,
        )
        .unwrap();
    let mut resumed = adapter();
    resumed
        .resume(&store, &checkpoint.checkpoint.content.identity.digest())
        .unwrap();
    host.train_step(&common::request(6), &batch, &cancel)
        .unwrap();
    resumed
        .train_step(&common::request(6), &batch, &cancel)
        .unwrap();
    let original = host.infer(&batch.inputs).unwrap();
    let actual = resumed.infer(&batch.inputs).unwrap();
    assert_eq!(original, actual);
    let conduit_data::TensorBacking::Inline(bytes) = &actual[0].backing else {
        panic!("inline fixture");
    };
    let values = bytes
        .as_slice()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b))
        .collect::<Vec<_>>();
    assert!(values[0] > 0.0, "bias learned");
    assert!(
        values.iter().all(|v| (*v - values[0]).abs() < 0.000001),
        "zero frozen weight keeps every prediction equal"
    );
}
