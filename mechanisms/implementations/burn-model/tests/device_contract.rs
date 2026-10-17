#[cfg(feature = "cuda")]
mod common;
use conduit_burn_model::DeviceRequest;
#[cfg(feature = "cuda")]
use conduit_burn_model::{BurnAdapter, Cancellation, OptimizerRecipe};
#[test]
fn cuda_request_refuses_when_not_compiled() {
    #[cfg(not(feature = "cuda"))]
    assert_eq!(
        DeviceRequest::Cuda(0).prepare().unwrap_err(),
        conduit_burn_model::Error::UnsupportedDevice
    );
}
#[test]
#[ignore = "requires an explicitly selected CUDA device"]
#[cfg(feature = "cuda")]
fn admitted_cuda_training_and_resume() {
    let mut context = common::context();
    context.realization.device_profile = "burn/cuda/0".into();
    let recipe = OptimizerRecipe {
        learning_rate: 0.05,
        weight_decay: 0.,
        gradient_clip: 10.,
        seed: 42,
    };
    let mut host = BurnAdapter::initialize(
        common::RegressionDefinition,
        DeviceRequest::Cuda(0),
        recipe.clone(),
        context.clone(),
    )
    .unwrap();
    let before = *host
        .evaluate(&common::request(1).batch, &common::batch())
        .unwrap()
        .metrics[0]
        .value_millionths();
    for step in 1..=80 {
        host.train_step(
            &common::request(step),
            &common::batch(),
            &Cancellation::default(),
        )
        .unwrap();
    }
    let after = *host
        .evaluate(&common::request(1).batch, &common::batch())
        .unwrap()
        .metrics[0]
        .value_millionths();
    assert!(after < before / 10);
    let mut cpu = BurnAdapter::initialize(
        common::RegressionDefinition,
        DeviceRequest::Cpu,
        recipe.clone(),
        common::context(),
    )
    .unwrap();
    for step in 1..=80 {
        cpu.train_step(
            &common::request(step),
            &common::batch(),
            &Cancellation::default(),
        )
        .unwrap();
    }
    let cuda_output = host.infer(&common::batch().inputs).unwrap();
    let cpu_output = cpu.infer(&common::batch().inputs).unwrap();
    let values = |value: &conduit_data::TensorValue| {
        let conduit_data::TensorBacking::Inline(bytes) = &value.backing else {
            panic!("fixture outputs are inline");
        };
        bytes
            .as_slice()
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| f32::from_le_bytes(*b))
            .collect::<Vec<_>>()
    };
    // Separate accumulated training drift from inference on an identical snapshot.
    let trained_drift = values(&cuda_output[0])
        .into_iter()
        .zip(values(&cpu_output[0]))
        .map(|(cuda, cpu)| (cuda - cpu).abs())
        .fold(0_f32, f32::max);
    println!("independent CPU/CUDA training: max absolute output drift {trained_drift}; bound 0.001 after 80 steps");
    assert!(trained_drift <= 0.001);
    let dir = tempfile::tempdir().unwrap();
    let store = conduit_burn_model::DirectoryCheckpointStore::new(dir.path(), 65536, 16).unwrap();
    let checkpoint_metrics = host
        .evaluate(&common::request(1).batch, &common::batch())
        .unwrap()
        .metrics;
    let receipt = host
        .checkpoint(&store, checkpoint_metrics, &Cancellation::default())
        .unwrap();
    let inference_dir = tempfile::tempdir().unwrap();
    let inference_store =
        conduit_burn_model::DirectoryCheckpointStore::new(inference_dir.path(), 65536, 16).unwrap();
    let export = host
        .export_inference(&inference_store, &Cancellation::default())
        .unwrap();
    let mut cpu_runtime = host.runtime().clone();
    cpu_runtime.device_evidence = DeviceRequest::Cpu.evidence();
    let mut cpu_same_weights = conduit_burn_model::InferenceBurnAdapter::load(
        common::RegressionDefinition,
        DeviceRequest::Cpu,
        conduit_burn_model::InferenceContext {
            artifact: context.artifact.clone(),
            runtime: cpu_runtime,
            determinism_profile: context.realization.deterministic_profile.clone(),
        },
        &inference_store,
        &export.content.identity.digest(),
    )
    .unwrap();
    let cpu_same_output = cpu_same_weights.infer(&common::batch().inputs).unwrap();
    let inference_drift = values(&cuda_output[0])
        .into_iter()
        .zip(values(&cpu_same_output[0]))
        .map(|(cuda, cpu)| (cuda - cpu).abs())
        .fold(0_f32, f32::max);
    println!("same-checkpoint CPU/CUDA inference: max absolute output drift {inference_drift}; bound 0.001");
    assert!(inference_drift <= 0.001);
    let mut resumed = BurnAdapter::initialize(
        common::RegressionDefinition,
        DeviceRequest::Cuda(0),
        recipe,
        context,
    )
    .unwrap();
    resumed
        .resume(&store, &receipt.checkpoint.content.identity.digest())
        .unwrap();
    host.train_step(
        &common::request(81),
        &common::batch(),
        &Cancellation::default(),
    )
    .unwrap();
    resumed
        .train_step(
            &common::request(81),
            &common::batch(),
            &Cancellation::default(),
        )
        .unwrap();
    let a = host.infer(&common::batch().inputs).unwrap();
    let b = resumed.infer(&common::batch().inputs).unwrap();
    assert_eq!(a, b);
    println!(
        "device={:?}, before={before}, after={after}, resumed_steps={}",
        host.runtime(),
        resumed.state().completed_steps
    );
}
