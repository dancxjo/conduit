#![cfg(not(feature = "cuda"))]
//! The public library journey, with optional retained artifacts for xtask proof.
mod common;
use conduit_burn_model::{
    BurnAdapter, Cancellation, DeviceRequest, DirectoryCheckpointStore, InferenceBurnAdapter,
    InferenceContext, OptimizerRecipe,
};

#[test]
fn retained_cpu_train_resume_export_reload_journey() {
    let temporary = tempfile::tempdir().unwrap();
    let root = match std::env::var_os("CONDUIT_MODEL_PROOF_ARTIFACTS") {
        Some(path) => {
            let path = std::path::PathBuf::from(path);
            std::fs::create_dir(&path).expect("proof artifacts require a new directory");
            path
        }
        None => temporary.path().to_owned(),
    };
    let context = common::context();
    let artifact = context.artifact.clone();
    let determinism_profile = context.realization.deterministic_profile.clone();
    let recipe = OptimizerRecipe {
        learning_rate: 0.05,
        weight_decay: 0.,
        gradient_clip: 10.,
        seed: 42,
    };
    let create = || {
        BurnAdapter::initialize(
            common::RegressionDefinition,
            DeviceRequest::Cpu,
            recipe.clone(),
            context.clone(),
        )
        .unwrap()
    };
    let mut trainer = create();
    let batch = common::batch();
    let cancel = Cancellation::default();
    let metric = |model: &mut BurnAdapter<common::RegressionDefinition>| {
        *model
            .evaluate(&common::request(1).batch, &batch)
            .unwrap()
            .metrics[0]
            .value_millionths()
    };
    let before = metric(&mut trainer);
    for step in 1..=40 {
        trainer
            .train_step(&common::request(step), &batch, &cancel)
            .unwrap();
    }
    let store = DirectoryCheckpointStore::new(root.join("resume"), 65536, 16).unwrap();
    let checkpoint_metrics = trainer
        .evaluate(&common::request(1).batch, &batch)
        .unwrap()
        .metrics;
    let checkpoint = trainer
        .checkpoint(&store, checkpoint_metrics, &cancel)
        .unwrap();
    let checkpoint_id = checkpoint.checkpoint.content.identity.digest();
    let at_checkpoint = trainer.infer(&batch.inputs).unwrap();
    drop(trainer);
    let mut resumed = create();
    resumed.resume(&store, &checkpoint_id).unwrap();
    assert_eq!(resumed.state().completed_steps, 40);
    assert_eq!(resumed.infer(&batch.inputs).unwrap(), at_checkpoint);
    for step in 41..=80 {
        resumed
            .train_step(&common::request(step), &batch, &cancel)
            .unwrap();
    }
    let after = metric(&mut resumed);
    assert!(after < before / 10);
    let predicted = resumed.infer(&batch.inputs).unwrap();
    let inference_store = DirectoryCheckpointStore::new(root.join("inference"), 65536, 16).unwrap();
    let exported = resumed.export_inference(&inference_store, &cancel).unwrap();
    let inference_context = InferenceContext {
        artifact,
        runtime: resumed.runtime().clone(),
        determinism_profile,
    };
    drop(resumed);
    let mut inference = InferenceBurnAdapter::load(
        common::RegressionDefinition,
        DeviceRequest::Cpu,
        inference_context,
        &inference_store,
        &exported.content.identity.digest(),
    )
    .unwrap();
    assert_eq!(inference.infer(&batch.inputs).unwrap(), predicted);
    let evidence = serde_json::json!({
        "schema":"conduit.fixture/model-authoring-journey@1",
        "scope":"synthetic-regression-fixture",
        "before_loss_millionths":before,"after_loss_millionths":after,
        "durable_resume_step":40,"final_committed_step":80,
        "resume_checkpoint_identity":checkpoint_id,
        "inference_checkpoint_identity":exported.content.identity.digest(),
        "inference_artifact_extent_bytes":exported.content.extent.bytes,
        "exact_same_device_reload":true,
        "runtime":{
            "provider":inference.runtime().provider_name,
            "name":inference.runtime().runtime_name,
            "version":inference.runtime().runtime_version,
            "build":inference.runtime().runtime_build_identity,
            "implementation":inference.runtime().adapter_artifact_identity,
            "device":inference.runtime().device_evidence,
            "precision":inference.runtime().precision_profile
        },
        "does_not_establish":["corpus batch cursor","ConduitVoice","natural speech","ordinary Plot/HostCall execution"]
    });
    std::fs::write(
        root.join("journey.json"),
        serde_json::to_vec_pretty(&evidence).unwrap(),
    )
    .unwrap();
    println!(
        "retained CPU journey: loss {before} -> {after} millionths; durable step 40, final step 80"
    );
}
