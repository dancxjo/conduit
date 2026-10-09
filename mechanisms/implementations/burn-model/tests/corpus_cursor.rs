mod common;
#[path = "common/corpus.rs"]
mod corpus_fixture;
use conduit_ai::TrainStepOutcome;
use conduit_burn_model::*;

fn adapter(seed: u64, changed: bool) -> BurnAdapter<common::RegressionDefinition> {
    let context = common::context();
    let owner = corpus_fixture::finite_corpus(&context, seed, changed);
    let mut host = BurnAdapter::initialize(
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
    host.attach_corpus(owner).unwrap();
    host
}
fn metrics(
    host: &mut BurnAdapter<common::RegressionDefinition>,
) -> Vec<conduit_ai::TrainingMetric> {
    let b = host.next_corpus_batch().unwrap().clone();
    host.evaluate(&b.identity, &b.tensors).unwrap().metrics
}
#[cfg(not(feature = "cuda"))]
#[test]
fn actual_next_tensors_resume_across_seeded_epoch_boundary_without_replayed_caller_batch() {
    let dir = tempfile::tempdir().unwrap();
    let store = DirectoryCheckpointStore::new(dir.path(), 65536, 16).unwrap();
    let cancel = Cancellation::default();
    let mut uninterrupted = adapter(41, false);
    let mut interrupted = adapter(41, false);
    for _ in 0..3 {
        assert_eq!(
            uninterrupted.next_corpus_batch().unwrap().identity,
            interrupted.next_corpus_batch().unwrap().identity
        );
        uninterrupted.train_next(&cancel).unwrap();
        interrupted.train_next(&cancel).unwrap();
    }
    assert_eq!(interrupted.corpus_cursor().unwrap().epoch, 1);
    assert_eq!(interrupted.corpus_cursor().unwrap().next_batch, 1);
    let m = metrics(&mut interrupted);
    let checkpoint = interrupted.checkpoint(&store, m, &cancel).unwrap();
    let id = checkpoint.checkpoint.content.identity.digest();
    drop(interrupted);
    let mut fresh = adapter(41, false);
    fresh.resume(&store, &id).unwrap();
    for _ in 3..8 {
        let a = uninterrupted.next_corpus_batch().unwrap();
        let b = fresh.next_corpus_batch().unwrap();
        assert_eq!(a.identity, b.identity);
        assert_eq!(a.tensors.inputs, b.tensors.inputs);
        assert_eq!(a.tensors.targets, b.tensors.targets);
        assert!(matches!(
            uninterrupted.train_next(&cancel).unwrap(),
            TrainStepOutcome::Committed(_)
        ));
        assert!(matches!(
            fresh.train_next(&cancel).unwrap(),
            TrainStepOutcome::Committed(_)
        ));
        assert_eq!(uninterrupted.corpus_cursor(), fresh.corpus_cursor());
    }
    assert_eq!(uninterrupted.state(), fresh.state());
    assert_eq!(
        uninterrupted.infer(&common::batch().inputs).unwrap(),
        fresh.infer(&common::batch().inputs).unwrap()
    );
    assert!(fresh.next_corpus_batch().is_none());
    let before = fresh.snapshot_identity().unwrap();
    let cursor = fresh.corpus_cursor().cloned();
    assert!(fresh.train_next(&cancel).is_err());
    assert_eq!(before, fresh.snapshot_identity().unwrap());
    assert_eq!(cursor.as_ref(), fresh.corpus_cursor());
}
#[test]
fn cancellation_evaluation_and_failed_checkpoint_distinguish_live_and_durable_cursor() {
    let dir = tempfile::tempdir().unwrap();
    let store = DirectoryCheckpointStore::new(dir.path(), 65536, 1).unwrap();
    let mut host = adapter(41, false);
    let cancelled = Cancellation::default();
    cancelled.cancel();
    let before = host.snapshot_identity().unwrap();
    let cursor = host.corpus_cursor().cloned();
    assert!(!matches!(
        host.train_next(&cancelled).unwrap(),
        TrainStepOutcome::Committed(_)
    ));
    assert_eq!(before, host.snapshot_identity().unwrap());
    assert_eq!(cursor.as_ref(), host.corpus_cursor());
    let cancel = Cancellation::default();
    host.train_next(&cancel).unwrap();
    let cursor = host.corpus_cursor().cloned();
    let m = metrics(&mut host);
    assert_eq!(cursor.as_ref(), host.corpus_cursor());
    host.checkpoint(&store, m, &cancel).unwrap();
    let previous = store.latest_identity().unwrap();
    host.train_next(&cancel).unwrap();
    let live = host.corpus_cursor().cloned();
    let m = metrics(&mut host);
    assert!(host.checkpoint(&store, m, &cancel).is_err());
    assert_eq!(previous, store.latest_identity().unwrap());
    assert_eq!(live.as_ref(), host.corpus_cursor());
    let mut resumed = adapter(41, false);
    resumed.resume(&store, &previous).unwrap();
    assert_eq!(resumed.corpus_cursor().unwrap().next_batch, 1);
    assert_eq!(host.corpus_cursor().unwrap().epoch, 1);
    assert!(host
        .train_step(&common::request(3), &common::batch(), &cancel)
        .is_err());
}
#[test]
fn changed_actual_payload_recipe_and_missing_owner_refuse_before_any_state_mutation() {
    let dir = tempfile::tempdir().unwrap();
    let store = DirectoryCheckpointStore::new(dir.path(), 65536, 16).unwrap();
    let mut host = adapter(41, false);
    let cancel = Cancellation::default();
    host.train_next(&cancel).unwrap();
    let m = metrics(&mut host);
    let id = host
        .checkpoint(&store, m, &cancel)
        .unwrap()
        .checkpoint
        .content
        .identity
        .digest();
    for (seed, changed) in [(42, false), (41, true)] {
        let mut fresh = adapter(seed, changed);
        let before = fresh.snapshot_identity().unwrap();
        let cursor = fresh.corpus_cursor().cloned();
        assert_eq!(
            fresh.resume(&store, &id),
            Err(Error::IncompatibleCheckpoint)
        );
        assert_eq!(before, fresh.snapshot_identity().unwrap());
        assert_eq!(cursor.as_ref(), fresh.corpus_cursor());
    }
    let mut manual = BurnAdapter::initialize(
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
    assert_eq!(
        manual.resume(&store, &id),
        Err(Error::IncompatibleCheckpoint)
    );
}
#[test]
fn training_only_cadence_stable_order_and_cursor_tampering_are_explicit() {
    use sha2::{Digest, Sha256};
    let mut context = common::context();
    context.session.evaluation_policy = conduit_ai::EvaluationPolicy::None;
    let owner = corpus_fixture::finite_corpus(&context, 41, false);
    let mut host = BurnAdapter::initialize(
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
    host.attach_corpus(owner).unwrap();
    let mut order = Vec::new();
    let cancel = Cancellation::default();
    for _ in 0..8 {
        order.push(host.next_corpus_batch().unwrap().identity.identity[0]);
        host.train_next(&cancel).unwrap();
    }
    assert_eq!(order, [32, 31, 32, 31, 31, 32, 31, 32]);
    let dir = tempfile::tempdir().unwrap();
    let store = DirectoryCheckpointStore::new(dir.path(), 65536, 16).unwrap();
    let mut trained = adapter(41, false);
    trained.train_next(&cancel).unwrap();
    let m = metrics(&mut trained);
    let id = trained
        .checkpoint(&store, m, &cancel)
        .unwrap()
        .checkpoint
        .content
        .identity
        .digest();
    let original: serde_json::Value =
        serde_json::from_slice(&std::fs::read(store.descriptor_path(&id)).unwrap()).unwrap();
    for (field, value) in [
        ("next_batch", serde_json::json!(0)),
        ("epoch", serde_json::json!(4)),
        ("batch_count", serde_json::json!(3)),
        ("profile", serde_json::json!("foreign/order@1")),
    ] {
        let mut changed = original.clone();
        changed["resume"]["corpus_cursor"][field] = value;
        let bytes = serde_json::to_vec(&changed).unwrap();
        let forged: [u8; 32] = Sha256::digest(&bytes).into();
        std::fs::write(store.descriptor_path(&forged), bytes).unwrap();
        let mut fresh = adapter(41, false);
        let before = fresh.snapshot_identity().unwrap();
        let cursor = fresh.corpus_cursor().cloned();
        assert_eq!(
            fresh.resume(&store, &forged),
            Err(Error::IncompatibleCheckpoint)
        );
        assert_eq!(before, fresh.snapshot_identity().unwrap());
        assert_eq!(cursor.as_ref(), fresh.corpus_cursor());
    }
}
#[test]
fn stable_order_and_descriptor_version_are_checked_without_reopening_legacy_manual_state() {
    use sha2::{Digest, Sha256};
    let context = common::context();
    let owner = corpus_fixture::finite_corpus_order(&context, None, false);
    let mut stable = BurnAdapter::initialize(
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
    stable.attach_corpus(owner).unwrap();
    let cancel = Cancellation::default();
    for expected in [31, 32, 31, 32] {
        assert_eq!(
            stable.next_corpus_batch().unwrap().identity.identity,
            [expected; 32]
        );
        stable.train_next(&cancel).unwrap();
    }
    let dir = tempfile::tempdir().unwrap();
    let store = DirectoryCheckpointStore::new(dir.path(), 65536, 16).unwrap();
    let mut corpus = adapter(41, false);
    corpus.train_next(&cancel).unwrap();
    let m = metrics(&mut corpus);
    let id = corpus
        .checkpoint(&store, m, &cancel)
        .unwrap()
        .checkpoint
        .content
        .identity
        .digest();
    let mut descriptor: serde_json::Value =
        serde_json::from_slice(&std::fs::read(store.descriptor_path(&id)).unwrap()).unwrap();
    assert_eq!(descriptor["schema"], 2);
    descriptor["schema"] = serde_json::json!(1);
    let bytes = serde_json::to_vec(&descriptor).unwrap();
    let forged: [u8; 32] = Sha256::digest(&bytes).into();
    std::fs::write(store.descriptor_path(&forged), bytes).unwrap();
    let mut fresh = adapter(41, false);
    let before = fresh.snapshot_identity().unwrap();
    assert_eq!(
        fresh.resume(&store, &forged),
        Err(Error::IncompatibleCheckpoint)
    );
    assert_eq!(before, fresh.snapshot_identity().unwrap());
    descriptor["schema"] = serde_json::json!(2);
    descriptor["resume"]
        .as_object_mut()
        .unwrap()
        .remove("corpus_cursor");
    let bytes = serde_json::to_vec(&descriptor).unwrap();
    let forged: [u8; 32] = Sha256::digest(&bytes).into();
    std::fs::write(store.descriptor_path(&forged), bytes).unwrap();
    assert_eq!(
        fresh.resume(&store, &forged),
        Err(Error::IncompatibleCheckpoint)
    );
    let manual_context = common::context();
    let mut manual = BurnAdapter::initialize(
        common::RegressionDefinition,
        DeviceRequest::Cpu,
        OptimizerRecipe {
            learning_rate: 0.05,
            weight_decay: 0.,
            gradient_clip: 10.,
            seed: 42,
        },
        manual_context.clone(),
    )
    .unwrap();
    manual
        .train_step(&common::request(1), &common::batch(), &cancel)
        .unwrap();
    let m = manual
        .evaluate(&common::request(1).batch, &common::batch())
        .unwrap()
        .metrics;
    let legacy = manual
        .checkpoint(&store, m, &cancel)
        .unwrap()
        .checkpoint
        .content
        .identity
        .digest();
    let descriptor: serde_json::Value =
        serde_json::from_slice(&std::fs::read(store.descriptor_path(&legacy)).unwrap()).unwrap();
    assert_eq!(descriptor["schema"], 1);
    assert!(descriptor["resume"].get("corpus_cursor").is_none());
    let mut reload = BurnAdapter::initialize(
        common::RegressionDefinition,
        DeviceRequest::Cpu,
        OptimizerRecipe {
            learning_rate: 0.05,
            weight_decay: 0.,
            gradient_clip: 10.,
            seed: 42,
        },
        manual_context,
    )
    .unwrap();
    reload.resume(&store, &legacy).unwrap();
    assert_eq!(reload.state(), manual.state());
    assert_eq!(
        fresh.resume(&store, &legacy),
        Err(Error::IncompatibleCheckpoint)
    );
}
#[test]
fn preparation_enforces_exact_membership_payload_and_declared_tail_and_resource_bounds() {
    use conduit_ai::TrainingExampleIdentityPages;
    let context = common::context();
    fn entries() -> Vec<CorpusBatch> {
        [3usize, 1]
            .into_iter()
            .enumerate()
            .map(|(index, count)| {
                let mut identity = common::request(1).batch;
                identity.identity = [60 + index as u8; 32];
                let offset = if index == 0 { 0 } else { 3 };
                identity.example_identities = TrainingExampleIdentityPages::from_values(
                    (0..count).map(|i| [10 + (offset + i) as u8; 32]).collect(),
                )
                .unwrap();
                identity.encoded_bytes = (count * 8) as u64;
                CorpusBatch {
                    identity,
                    tensors: ModelBatch {
                        inputs: vec![common::value(&vec![1.; count], count)],
                        targets: vec![common::value(&vec![3.; count], count)],
                    },
                }
            })
            .collect()
    }
    let recipe = CorpusRecipe {
        shuffle_seed: None,
        epochs: 2,
        allow_short_final_batch: true,
    };
    let limits = CorpusLimits {
        maximum_batches: 2,
        maximum_tensor_bytes: 32,
        maximum_epochs: 2,
    };
    assert!(PreparedTrainingCorpus::prepare(&context, entries(), recipe.clone(), limits).is_ok());
    for bad in [
        CorpusLimits {
            maximum_batches: 1,
            ..limits
        },
        CorpusLimits {
            maximum_tensor_bytes: 31,
            ..limits
        },
        CorpusLimits {
            maximum_epochs: 1,
            ..limits
        },
    ] {
        assert!(PreparedTrainingCorpus::prepare(&context, entries(), recipe.clone(), bad).is_err());
    }
    let mut oversized = entries();
    oversized.reserve(100);
    assert!(PreparedTrainingCorpus::prepare(&context, oversized, recipe.clone(), limits).is_err());
    let mut oversized = entries();
    oversized[0].tensors.inputs.reserve(100);
    assert!(PreparedTrainingCorpus::prepare(&context, oversized, recipe.clone(), limits).is_err());
    let mut malformed = entries();
    // Preserve a valid tensor and byte count while its Batch axis has the
    // wrong relationship to the three admitted example identities.
    malformed[0].tensors.inputs[0] = common::value(&[1.], 1);
    malformed[0].identity.encoded_bytes = 16;
    assert!(PreparedTrainingCorpus::prepare(&context, malformed, recipe.clone(), limits).is_err());
    let mut bad_recipe = recipe.clone();
    bad_recipe.allow_short_final_batch = false;
    assert!(PreparedTrainingCorpus::prepare(&context, entries(), bad_recipe, limits).is_err());
    let mut bad = entries();
    bad[1].identity.example_identities =
        TrainingExampleIdentityPages::from_values(vec![[99; 32]]).unwrap();
    assert!(PreparedTrainingCorpus::prepare(&context, bad, recipe.clone(), limits).is_err());
    let mut bad = entries();
    bad[1].identity.example_identities =
        TrainingExampleIdentityPages::from_values(vec![[10; 32]]).unwrap();
    assert!(PreparedTrainingCorpus::prepare(&context, bad, recipe.clone(), limits).is_err());
    let mut bad = entries();
    bad[0].identity.encoded_bytes -= 1;
    assert!(PreparedTrainingCorpus::prepare(&context, bad, recipe, limits).is_err());
}

#[test]
fn corpus_memory_admission_and_unload_release_owned_progress() {
    let context = common::context();
    let estimate = corpus_fixture::finite_corpus(&context, 41, false).storage_estimate();
    let required = common::RegressionDefinition
        .descriptor()
        .resources
        .working_bytes()
        .unwrap()
        + estimate.retained_bytes
        + estimate.staging_bytes;
    for (budget, accepted) in [(required, true), (required - 1, false)] {
        let mut context = common::context();
        context.session.resources = conduit_ai::TrainingResourceEnvelope::new(
            4096, budget, 1, 8, 64, 100, 100_000, 65536, 1,
        )
        .unwrap();
        let owner = corpus_fixture::finite_corpus(&context, 41, false);
        let mut host = BurnAdapter::initialize(
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
        let before = host.state().clone();
        assert_eq!(host.attach_corpus(owner).is_ok(), accepted);
        assert_eq!(host.state(), &before);
        assert_eq!(host.corpus_cursor().is_some(), accepted);
        if accepted {
            host.unload().unwrap();
            assert!(host.corpus_cursor().is_none());
            assert!(host.next_corpus_batch().is_none());
            let before = host.state().clone();
            assert!(matches!(
                host.train_next(&Cancellation::default()),
                Err(Error::Unloaded)
            ));
            assert_eq!(host.state(), &before);
        }
    }
}
