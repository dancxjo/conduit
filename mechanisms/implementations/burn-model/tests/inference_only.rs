mod common;
use conduit_burn_model::{
    BurnAdapter, Cancellation, DeviceRequest, DirectoryCheckpointStore, InferenceBurnAdapter,
    InferenceContext, OptimizerRecipe,
};
#[test]
fn inference_export_reloads_without_dataset_session_or_optimizer() {
    let context = common::context();
    let artifact = context.artifact.clone();
    let determinism_profile = context.realization.deterministic_profile.clone();
    let mut trainer = BurnAdapter::initialize(
        common::RegressionDefinition,
        DeviceRequest::Cpu,
        OptimizerRecipe {
            learning_rate: 0.05,
            weight_decay: 0.,
            gradient_clip: 10.,
            seed: 42,
        },
        context,
    )
    .unwrap();
    trainer
        .train_step(
            &common::request(1),
            &common::batch(),
            &Cancellation::default(),
        )
        .unwrap();
    let expected = trainer.infer(&common::batch().inputs).unwrap();
    let runtime = trainer.runtime().clone();
    let dir = tempfile::tempdir().unwrap();
    let store = DirectoryCheckpointStore::new(dir.path(), 65536, 16).unwrap();
    let checkpoint = trainer
        .export_inference(&store, &Cancellation::default())
        .unwrap();
    drop(trainer);
    let context = InferenceContext {
        artifact,
        runtime,
        determinism_profile,
    };
    let mut inference = InferenceBurnAdapter::load(
        common::RegressionDefinition,
        DeviceRequest::Cpu,
        context,
        &store,
        &checkpoint.content.identity.digest(),
    )
    .unwrap();
    assert_eq!(inference.infer(&common::batch().inputs).unwrap(), expected);
    assert_eq!(
        inference.inference_checkpoint_identity(),
        Some(checkpoint.content.identity.digest())
    );
    inference.unload().unwrap();
    assert!(inference.infer(&common::batch().inputs).is_err());
}

#[test]
fn training_adapter_conversion_discards_training_contract() {
    let mut trainer = BurnAdapter::initialize(
        common::RegressionDefinition,
        DeviceRequest::Cpu,
        OptimizerRecipe {
            learning_rate: 0.05,
            weight_decay: 0.,
            gradient_clip: 10.,
            seed: 42,
        },
        common::context(),
    )
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let store = DirectoryCheckpointStore::new(dir.path(), 65536, 16).unwrap();
    let checkpoint = trainer
        .export_inference(&store, &Cancellation::default())
        .unwrap();
    trainer
        .load_inference(&store, &checkpoint.content.identity.digest())
        .unwrap();
    assert!(matches!(
        trainer.evaluate(&common::request(1).batch, &common::batch()),
        Err(conduit_burn_model::Error::IncompatibleCheckpoint)
    ));
    let mut inference = trainer.into_inference().unwrap();
    let cancelled = Cancellation::default();
    cancelled.cancel();
    assert!(matches!(
        inference.infer_cancellable(&common::batch().inputs, &cancelled),
        Err(conduit_burn_model::Error::Cancelled)
    ));
    assert!(inference.infer(&common::batch().inputs).is_ok());
    assert_eq!(
        inference.offer().supported_operations,
        vec![conduit_ai::ModelComputeOperation::Inference]
    );
}

#[test]
fn tutorial_tensor_shape_and_batch_bounds_refuse_before_model_state_changes() {
    use conduit_burn_model::Error;
    use conduit_plot::rust_binding::BoundedSequence;
    let mut model = BurnAdapter::initialize(
        common::RegressionDefinition,
        DeviceRequest::Cpu,
        OptimizerRecipe {
            learning_rate: 0.05,
            weight_decay: 0.,
            gradient_clip: 10.,
            seed: 42,
        },
        common::context(),
    )
    .unwrap();
    let before = model.snapshot_identity().unwrap();
    let expected = model.infer(&common::batch().inputs).unwrap();
    let mut wrong_features = common::value(&[1.; 8], 4);
    wrong_features.dimensions = BoundedSequence::try_from_iter([4, 2]).unwrap();
    // The tensors are valid data; the admitted model signature rejects them.
    assert!(wrong_features.validate().is_ok());
    let oversized_batch = common::value(&[1.; 9], 9);
    assert!(oversized_batch.validate().is_ok());
    for input in [wrong_features, oversized_batch] {
        assert_eq!(model.infer(&[input]), Err(Error::InvalidTensor));
        assert_eq!(model.snapshot_identity().unwrap(), before);
        assert_eq!(model.infer(&common::batch().inputs).unwrap(), expected);
    }
}
