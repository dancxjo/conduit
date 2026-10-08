mod common;
use conduit_burn_model::{
    BurnAdapter, Cancellation, DeviceRequest, DirectoryCheckpointStore, Error, OptimizerRecipe,
};
fn recipe() -> OptimizerRecipe {
    OptimizerRecipe {
        learning_rate: 0.05,
        weight_decay: 0.,
        gradient_clip: 10.,
        seed: 42,
    }
}
fn adapter() -> BurnAdapter<common::RegressionDefinition> {
    BurnAdapter::initialize(
        common::RegressionDefinition,
        DeviceRequest::Cpu,
        recipe(),
        common::context(),
    )
    .unwrap()
}
#[test]
fn checkpoint_reload_preserves_inference() {
    let dir = tempfile::tempdir().unwrap();
    let store = DirectoryCheckpointStore::new(dir.path(), 65536, 16).unwrap();
    let mut host = adapter();
    let cancel = Cancellation::default();
    let batch = common::batch();
    host.train_step(&common::request(1), &batch, &cancel)
        .unwrap();
    let before = host.infer(&batch.inputs).unwrap();
    let receipt = host
        .checkpoint(
            &store,
            host.evaluate(&common::request(1).batch, &common::batch())
                .unwrap()
                .metrics,
            &cancel,
        )
        .unwrap();
    assert_eq!(receipt.completed_steps, 1);
    let mut loaded = adapter();
    loaded
        .load_inference(&store, &receipt.checkpoint.content.identity.digest())
        .unwrap();
    assert_eq!(before, loaded.infer(&batch.inputs).unwrap());
}
#[cfg(not(feature = "cuda"))]
#[test]
fn fresh_runtime_resumes_recorded_step_cursor() {
    let dir = tempfile::tempdir().unwrap();
    let store = DirectoryCheckpointStore::new(dir.path(), 65536, 16).unwrap();
    let mut host = adapter();
    let cancel = Cancellation::default();
    let batch = common::batch();
    for step in 1..=8 {
        host.train_step(&common::request(step), &batch, &cancel)
            .unwrap();
    }
    let checkpoint = host
        .checkpoint(
            &store,
            host.evaluate(&common::request(1).batch, &common::batch())
                .unwrap()
                .metrics,
            &cancel,
        )
        .unwrap();
    let mut resumed = adapter();
    resumed
        .resume(&store, &checkpoint.checkpoint.content.identity.digest())
        .unwrap();
    assert_eq!(resumed.state(), host.state());
    host.train_step(&common::request(9), &batch, &cancel)
        .unwrap();
    resumed
        .train_step(&common::request(9), &batch, &cancel)
        .unwrap();
    assert_eq!(
        host.infer(&batch.inputs).unwrap(),
        resumed.infer(&batch.inputs).unwrap()
    );
}
#[test]
fn checkpoint_failure_retains_durable_cursor() {
    let dir = tempfile::tempdir().unwrap();
    let store = DirectoryCheckpointStore::new(dir.path(), 65536, 1).unwrap();
    let mut host = adapter();
    let cancel = Cancellation::default();
    let batch = common::batch();
    host.train_step(&common::request(1), &batch, &cancel)
        .unwrap();
    host.checkpoint(
        &store,
        host.evaluate(&common::request(1).batch, &common::batch())
            .unwrap()
            .metrics,
        &cancel,
    )
    .unwrap();
    let previous = store.latest_identity().unwrap();
    host.train_step(&common::request(2), &batch, &cancel)
        .unwrap();
    assert!(host
        .checkpoint(
            &store,
            host.evaluate(&common::request(1).batch, &common::batch())
                .unwrap()
                .metrics,
            &cancel
        )
        .is_err());
    assert_eq!(store.latest_identity().unwrap(), previous);
}
#[test]
fn incompatible_recipe_and_corrupt_checkpoint_refuse_without_mutation() {
    let dir = tempfile::tempdir().unwrap();
    let store = DirectoryCheckpointStore::new(dir.path(), 65536, 16).unwrap();
    let mut host = adapter();
    let cancel = Cancellation::default();
    host.train_step(&common::request(1), &common::batch(), &cancel)
        .unwrap();
    let receipt = host
        .checkpoint(
            &store,
            host.evaluate(&common::request(1).batch, &common::batch())
                .unwrap()
                .metrics,
            &cancel,
        )
        .unwrap();
    let id = receipt.checkpoint.content.identity.digest();
    let mut changed = recipe();
    changed.learning_rate = 0.01;
    let mut other = BurnAdapter::initialize(
        common::RegressionDefinition,
        DeviceRequest::Cpu,
        changed,
        common::context(),
    )
    .unwrap();
    let before = other.snapshot_identity().unwrap();
    assert_eq!(
        other.resume(&store, &id),
        Err(Error::IncompatibleCheckpoint)
    );
    assert_eq!(before, other.snapshot_identity().unwrap());
    std::fs::write(store.descriptor_path(&id), b"{}").unwrap();
    assert_eq!(
        other.load_inference(&store, &id),
        Err(Error::CorruptCheckpoint)
    );
    assert_eq!(before, other.snapshot_identity().unwrap());
}

#[test]
fn inference_off_ramp_has_no_resume_authority() {
    let dir = tempfile::tempdir().unwrap();
    let store = DirectoryCheckpointStore::new(dir.path(), 65536, 16).unwrap();
    let mut host = adapter();
    let cancel = Cancellation::default();
    host.train_step(&common::request(1), &common::batch(), &cancel)
        .unwrap();
    let exported = host.export_inference(&store, &cancel).unwrap();
    let id = exported.content.identity.digest();
    let mut fresh = adapter();
    let before = fresh.snapshot_identity().unwrap();
    assert_eq!(
        fresh.resume(&store, &id),
        Err(Error::IncompatibleCheckpoint)
    );
    assert_eq!(before, fresh.snapshot_identity().unwrap());
    fresh.load_inference(&store, &id).unwrap();
    assert_eq!(
        fresh.train_step(&common::request(1), &common::batch(), &cancel),
        Err(Error::IncompatibleCheckpoint)
    );
    assert_eq!(
        host.infer(&common::batch().inputs).unwrap(),
        fresh.infer(&common::batch().inputs).unwrap()
    );
}

#[test]
fn failed_pointer_replacement_never_advertises_partial_bundle() {
    let dir = tempfile::tempdir().unwrap();
    let store = DirectoryCheckpointStore::new(dir.path(), 65536, 16).unwrap();
    let host = adapter();
    let cancel = Cancellation::default();
    // Force rename failure after immutable blob/descriptor durability.
    std::fs::create_dir(dir.path().join("latest.json")).unwrap();
    assert!(matches!(
        host.checkpoint(
            &store,
            host.evaluate(&common::request(1).batch, &common::batch())
                .unwrap()
                .metrics,
            &cancel
        ),
        Err(Error::CheckpointIo(_))
    ));
    assert!(store.latest_identity().is_err());
    std::fs::remove_dir(dir.path().join("latest.json")).unwrap();
    let recovered = host
        .checkpoint(
            &store,
            host.evaluate(&common::request(1).batch, &common::batch())
                .unwrap()
                .metrics,
            &cancel,
        )
        .unwrap();
    assert_eq!(
        store.latest_identity().unwrap(),
        recovered.checkpoint.content.identity.digest()
    );
}

#[test]
fn inference_reexport_preserves_checkpoint_generation_and_identity() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let first_store = DirectoryCheckpointStore::new(first.path(), 65536, 16).unwrap();
    let second_store = DirectoryCheckpointStore::new(second.path(), 65536, 16).unwrap();
    let mut host = adapter();
    let cancel = Cancellation::default();
    host.train_step(&common::request(1), &common::batch(), &cancel)
        .unwrap();
    let exported = host.export_inference(&first_store, &cancel).unwrap();
    let mut loaded = adapter();
    loaded
        .load_inference(&first_store, &exported.content.identity.digest())
        .unwrap();
    let reexport = loaded.export_inference(&second_store, &cancel).unwrap();
    assert_eq!(exported, reexport);
}
#[test]
fn cancellation_and_stale_resume_preserve_live_and_durable_progress() {
    let dir = tempfile::tempdir().unwrap();
    let store = DirectoryCheckpointStore::new(dir.path(), 65536, 16).unwrap();
    let mut host = adapter();
    let cancel = Cancellation::default();
    host.train_step(&common::request(1), &common::batch(), &cancel)
        .unwrap();
    let checkpoint = host
        .checkpoint(
            &store,
            host.evaluate(&common::request(1).batch, &common::batch())
                .unwrap()
                .metrics,
            &cancel,
        )
        .unwrap();
    let previous = store.latest_identity().unwrap();
    host.train_step(&common::request(2), &common::batch(), &cancel)
        .unwrap();
    let before = host.snapshot_identity().unwrap();
    assert_eq!(
        host.resume(&store, &checkpoint.checkpoint.content.identity.digest()),
        Err(Error::IncompatibleCheckpoint)
    );
    let cancelled = Cancellation::default();
    cancelled.cancel();
    assert_eq!(
        host.checkpoint(
            &store,
            host.evaluate(&common::request(1).batch, &common::batch())
                .unwrap()
                .metrics,
            &cancelled
        ),
        Err(Error::Cancelled)
    );
    assert_eq!(store.latest_identity().unwrap(), previous);
    assert_eq!(before, host.snapshot_identity().unwrap());
    assert_eq!(host.state().completed_steps, 2);
}
#[test]
fn corrupt_weight_member_refuses_without_replacing_model() {
    let dir = tempfile::tempdir().unwrap();
    let store = DirectoryCheckpointStore::new(dir.path(), 65536, 16).unwrap();
    let host = adapter();
    let exported = host
        .export_inference(&store, &Cancellation::default())
        .unwrap();
    let id = exported.content.identity.digest();
    let descriptor: serde_json::Value =
        serde_json::from_slice(&std::fs::read(store.descriptor_path(&id)).unwrap()).unwrap();
    let digest = descriptor["inference"]["identity"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| format!("{:02x}", v.as_u64().unwrap()))
        .collect::<String>();
    std::fs::write(dir.path().join(format!("{digest}.blob")), b"partial").unwrap();
    let mut fresh = adapter();
    let before = fresh.snapshot_identity().unwrap();
    assert_eq!(
        fresh.load_inference(&store, &id),
        Err(Error::CorruptCheckpoint)
    );
    assert_eq!(before, fresh.snapshot_identity().unwrap());
}
