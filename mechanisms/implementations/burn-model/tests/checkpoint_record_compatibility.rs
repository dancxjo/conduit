mod common;
use burn::{
    store::burn_pack::{Reader, Tensor, Writer},
    tensor::Bytes,
};
use conduit_burn_model::{
    burn, BurnAdapter, Cancellation, DeviceRequest, DirectoryCheckpointStore, Error,
    OptimizerRecipe,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn renamed(key: &str, ids: &BTreeMap<u64, u64>) -> String {
    let (prefix, suffix) = key.split_once('.').unwrap_or((key, ""));
    match prefix.parse::<u64>().ok().and_then(|id| ids.get(&id)) {
        Some(id) if suffix.is_empty() => id.to_string(),
        Some(id) => format!("{id}.{suffix}"),
        None => key.into(),
    }
}
#[test]
fn foreign_optimizer_parameter_ids_refuse_even_with_valid_content_digests() {
    tampered_optimizer_refuses(false);
}
#[test]
fn incompatible_optimizer_shapes_refuse_even_with_valid_content_digests() {
    tampered_optimizer_refuses(true);
}
fn tampered_optimizer_refuses(change_shape: bool) {
    let recipe = OptimizerRecipe {
        learning_rate: 0.05,
        weight_decay: 0.,
        gradient_clip: 10.,
        seed: 42,
    };
    let mut host = BurnAdapter::initialize(
        common::RegressionDefinition,
        DeviceRequest::Cpu,
        recipe.clone(),
        common::context(),
    )
    .unwrap();
    let cancel = Cancellation::default();
    host.train_step(&common::request(1), &common::batch(), &cancel)
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let store = DirectoryCheckpointStore::new(dir.path(), 65536, 16).unwrap();
    let checkpoint = host
        .checkpoint(
            &store,
            host.evaluate(&common::request(1).batch, &common::batch())
                .unwrap()
                .metrics,
            &cancel,
        )
        .unwrap();
    let mut descriptor: serde_json::Value = serde_json::from_slice(
        &std::fs::read(store.descriptor_path(&checkpoint.checkpoint.content.identity.digest()))
            .unwrap(),
    )
    .unwrap();
    let optimizer_id: [u8; 32] =
        serde_json::from_value(descriptor["resume"]["optimizer"]["identity"].clone()).unwrap();
    let bytes = std::fs::read(dir.path().join(format!("{}.blob", hex(&optimizer_id)))).unwrap();
    let reader = Reader::from_bytes(Bytes::from_bytes_vec(bytes)).unwrap();
    let metadata = reader.metadata().clone();
    let scalars = reader.scalars().clone();
    let tensors = reader.into_tensors().unwrap();
    let old_ids = tensors
        .iter()
        .map(|t| t.param_id.unwrap())
        .collect::<BTreeSet<_>>();
    let mut next = 0;
    let mut ids = BTreeMap::new();
    for old in &old_ids {
        while old_ids.contains(&next) {
            next += 1;
        }
        ids.insert(*old, if change_shape { *old } else { next });
        next += 1;
    }
    let changed = tensors
        .into_iter()
        .map(|t| {
            let (name, dtype, mut shape, id, bytes) = t.into_parts().unwrap();
            if change_shape {
                let mut dimensions = shape.to_vec();
                dimensions.push(1);
                shape = burn::tensor::Shape::from(dimensions);
            }
            Tensor::new(
                renamed(&name, &ids),
                dtype,
                shape,
                Some(ids[&id.unwrap()]),
                bytes,
            )
        })
        .collect();
    let mut writer = Writer::new(changed);
    for (key, value) in scalars {
        writer = writer.with_scalar(&renamed(&key, &ids), value);
    }
    for (key, value) in metadata {
        writer = writer.with_metadata(&renamed(&key, &ids), &value);
    }
    let changed = writer.into_bytes().unwrap();
    let changed_id = digest(&changed);
    std::fs::write(
        dir.path().join(format!("{}.blob", hex(&changed_id))),
        &*changed,
    )
    .unwrap();
    descriptor["resume"]["optimizer"]["identity"] = serde_json::json!(changed_id);
    descriptor["resume"]["optimizer"]["bytes"] = serde_json::json!(changed.len());
    let descriptor = serde_json::to_vec(&descriptor).unwrap();
    let identity = digest(&descriptor);
    std::fs::write(store.descriptor_path(&identity), descriptor).unwrap();
    let mut fresh = BurnAdapter::initialize(
        common::RegressionDefinition,
        DeviceRequest::Cpu,
        recipe,
        common::context(),
    )
    .unwrap();
    let before = fresh.snapshot_identity().unwrap();
    assert_eq!(
        fresh.resume(&store, &identity),
        Err(Error::CorruptCheckpoint)
    );
    assert_eq!(before, fresh.snapshot_identity().unwrap());
    assert_eq!(fresh.state().completed_steps, 0);
}
