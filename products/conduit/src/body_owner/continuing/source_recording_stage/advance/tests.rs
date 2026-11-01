use super::super::publication::tests::{finish_worker, fixture_sixteen, independent_read_grant};
use super::*;
#[test]
fn whole_sixteen_retains_slots_and_requires_verified_contiguous_chunks() {
    let (root, _, mut owner, publish) = fixture_sixteen();
    let originals: Vec<_> = owner
        .source_recording_stage
        .as_ref()
        .unwrap()
        .observations()
        .map(|o| (o.sequence(), o.canonical().to_vec()))
        .collect();
    let digest = owner
        .source_recording_stage
        .as_ref()
        .unwrap()
        .prepared
        .as_ref()
        .unwrap()
        .reference
        .identity
        .digest();
    assert!(
        owner
            .source_recording_stage
            .as_ref()
            .unwrap()
            .prepared
            .as_ref()
            .unwrap()
            .end
            < 16
    );
    assert!(owner
        .prepare_next_source_recording_chunk(&root, 0, digest, 1048576)
        .is_err());
    let worker = owner
        .start_service_source_publication(&root, 5000, &publish, 1048576)
        .unwrap();
    finish_worker(&root, &mut owner, worker);
    let read = independent_read_grant(&mut owner);
    let worker = owner
        .start_service_source_readback(&root, 5000, &read, 1048576)
        .unwrap();
    finish_worker(&root, &mut owner, worker);
    assert!(owner
        .prepare_source_recording_catalog(&root, &publish, 1048576)
        .err()
        .unwrap()
        .contains("whole frozen prefix"));
    assert!(owner
        .prepare_next_source_recording_chunk(&root, 0, [0; 32], 1048576)
        .is_err());
    assert!(owner
        .prepare_next_source_recording_chunk(&root, 0, digest, 1)
        .is_err());
    let next = owner
        .prepare_next_source_recording_chunk(&root, 0, digest, 1048576)
        .unwrap();
    assert_eq!(next["chunk_index"], 1);
    assert_eq!(
        owner
            .prepare_next_source_recording_chunk(&root, 0, digest, 1048576)
            .unwrap(),
        next
    );
    let stage = owner.source_recording_stage.as_ref().unwrap();
    assert_eq!(stage.prepared.as_ref().unwrap().end, 16);
    assert_eq!(
        stage
            .observations()
            .map(|o| (o.sequence(), o.canonical().to_vec()))
            .collect::<Vec<_>>(),
        originals
    );
    assert!(stage.verified_reference().is_none());
    let worker = owner
        .start_service_source_publication(&root, 5000, &publish, 1048576)
        .unwrap();
    finish_worker(&root, &mut owner, worker);
    let read = independent_read_grant(&mut owner);
    let worker = owner
        .start_service_source_readback(&root, 5000, &read, 1048576)
        .unwrap();
    finish_worker(&root, &mut owner, worker);
    assert_eq!(
        owner
            .source_recording_stage
            .as_ref()
            .unwrap()
            .verified_catalog_references()
            .unwrap()
            .len(),
        2
    );
    let worker = owner
        .start_service_source_catalog(&root, 5000, &publish, 1048576)
        .unwrap();
    finish_worker(&root, &mut owner, worker);
    assert_eq!(
        owner.last_execution.as_ref().unwrap()["source_catalog"]["catalog_checkpoint_observed"],
        true
    );
    assert_eq!(owner.source_recording_stage.as_ref().unwrap().len(), 16);
    std::fs::remove_dir_all(root).unwrap();
}
