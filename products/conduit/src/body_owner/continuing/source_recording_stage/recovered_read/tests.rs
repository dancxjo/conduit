use super::super::publication::tests::{finish_worker, fixture, independent_read_grant};
use super::*;
use crate::durable_host as installed;
use conduit_std_host::{StdHost, StdHostConfig};
const ROOT_ENV: &str = "CONDUIT_RECOVERED_SOURCE_READ_ROOT";
const TEST:&str="durable_host::owner::controller::continuing::source_recording_stage::recovered_read::tests::fresh_process_reads_recovered_catalog_and_chunk_through_admitted_worker";

// Fixture Root authorizes each current exact read offer, independently of State
// discovery and previous publication. This is no production grant factory.
fn authorize_read(owner: &mut Owner, reference: c::BoundedResourceRef) -> c::AuthorityGrant {
    owner
        .host
        .current_mut()
        .select_body_storage_generation(BodyStorageGenerationRequest {
            snapshot: Some(BodyStorageSnapshotRequest {
                reference,
                access: c::ResourceAccessMode::ReadPublished,
            }),
            state: None,
        })
        .unwrap();
    let resource = owner
        .host
        .selected_body_storage_generation()
        .unwrap()
        .snapshot_resource()
        .unwrap();
    let offer = conduit_std_host::linux_snapshot::offer_for_resource(resource).unwrap();
    assert_eq!(
        offer.implementation.implementation_id.as_str(),
        conduit_std_host::linux_snapshot::READ
    );
    let required = &offer.authority_requirements[0];
    c::AuthorityGrant {
        grant_id: "fixture/recovered-source-independent-read".into(),
        contract_id: required.contract_id.clone(),
        host_call_contract_id: required.host_call_contract_id.clone(),
        subject_kind: required.subject_kind.clone(),
        host_id: owner.host.advertisement().host_id.clone(),
        boot_id: owner.host.advertisement().boot_id.clone(),
        capability_id: offer.capability_id,
    }
}
fn retain_selection(root: &Path, disk: &Path) {
    let source=include_str!("../../../../../../../targets/std/profiles/linux-computer.host.conduit").replace(
        r#"base = {kind: "storage/protected-file", implementation: "hosted/protected-file@1"}"#,
        &format!(r#"base = {{kind: "storage/protected-file", implementation: "linux/body-storage-directory@1", initialization: {{root_directory: {:?}, pool_namespace: "test/publication", maximum_snapshot_records: 2, maximum_record_bytes: 4096, generation_slots: 1, publication_slots: 1, reader_leases: 1, state_slots: 1, maximum_state_bytes: 4096, total_disk_bytes: 131072, preparation_bytes: 1048576}}}}"#,disk.to_str().unwrap()));
    let path = root.join("recovered-read.host.conduit");
    std::fs::write(&path, source).unwrap();
    installed::selected_host_configuration::configure(root, Some(&path), false).unwrap();
}
#[test]
fn missing_private_recovery_refuses_before_generation_or_proposal_change() {
    let (root, disk, mut owner, grant) = fixture();
    let generation = owner.host.advertisement().offer_generation;
    let resident = owner.resident.clone();
    for index in [None, Some(0)] {
        assert!(owner
            .prepare_recovered_source_read(&root, &grant, index, 1024 * 1024)
            .err()
            .unwrap()
            .contains("State reference is absent"));
    }
    assert_eq!(owner.host.advertisement().offer_generation, generation);
    assert_eq!(owner.resident, resident);
    assert!(owner.session.realization().is_none());
    assert_eq!(std::fs::read_dir(disk).unwrap().count(), 0);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn fresh_process_reads_recovered_catalog_and_chunk_through_admitted_worker() {
    if let Some(root) = std::env::var_os(ROOT_ENV) {
        let root = std::path::PathBuf::from(root);
        let installation = installed::read_installation(&root.join("installation.json")).unwrap();
        let boot = installed::fresh_identity("boot", "recovered-source-read");
        let mut host = StdHost::new_with_config(StdHostConfig {
            host_id: installation.host_id.clone().into(),
            boot_id: boot.clone().into(),
            offer_generation: c::OfferGeneration(1),
        });
        installation
            .selected_host_configuration
            .as_ref()
            .unwrap()
            .attach_to_fresh_host(&mut host)
            .unwrap();
        let mut owner = installed::owner::resume_service(host, &root).unwrap();
        assert!(owner.source_recording_stage.is_none());
        assert!(owner.recovered_source_catalog.is_none());
        let worker = owner
            .start_service_source_catalog_recovery(&root, 5000, 1024 * 1024)
            .unwrap();
        finish_worker(&root, &mut owner, worker);
        let reference = owner
            .recovered_source_catalog
            .as_ref()
            .unwrap()
            .reference()
            .clone();
        let grant = authorize_read(&mut owner, reference.clone());
        // Old Boot authority refuses before selecting a new generation or resident.
        let mut stale = grant.clone();
        stale.boot_id = "boot/obsolete".into();
        let generation = owner.host.advertisement().offer_generation;
        let resident = owner.resident.clone();
        assert!(owner
            .prepare_recovered_source_read(&root, &stale, None, 1024 * 1024)
            .err()
            .unwrap()
            .contains("another Host/Boot"));
        assert_eq!(owner.host.advertisement().offer_generation, generation);
        assert_eq!(owner.resident, resident);
        assert!(owner
            .prepare_recovered_source_read(&root, &grant, Some(0), 1024 * 1024)
            .err()
            .unwrap()
            .contains("catalog content is absent"));
        let worker = owner
            .start_service_recovered_source_read(&root, 5000, &grant, None, 1024 * 1024)
            .unwrap();
        finish_worker(&root, &mut owner, worker);
        let contents = owner.recovered_source_catalog_contents.as_ref().unwrap();
        assert_eq!(contents.catalog_reference(), &reference);
        assert_eq!(contents.references().len(), 1);
        let chunk_reference = contents.references()[0].clone();
        assert!(owner
            .prepare_recovered_source_read(&root, &grant, Some(1), 1024 * 1024)
            .err()
            .unwrap()
            .contains("index exceeds"));
        let grant = authorize_read(&mut owner, chunk_reference.clone());
        let worker = owner
            .start_service_recovered_source_read(&root, 5000, &grant, Some(0), 1024 * 1024)
            .unwrap();
        finish_worker(&root, &mut owner, worker);
        let receipt = owner.last_execution.as_ref().unwrap();
        assert_eq!(receipt["source_recovered_read"]["read_completed"], true);
        assert_eq!(
            receipt["source_recovered_read"]["producer_retirement_authorized"],
            false
        );
        let chunk = owner.recovered_source_chunk.as_ref().unwrap();
        assert_eq!(chunk.index(), 0);
        assert_eq!(chunk.reference(), &chunk_reference);
        assert!(owner
            .recovered_source_catalog_contents
            .as_ref()
            .unwrap()
            .all_chunks_verified());
        assert_ne!(chunk.snapshot().context().boot_id.as_str(), boot);
        let canonical = chunk.snapshot().content().unwrap();
        installed::write_json_atomic(&root.join("fresh-source-read-result.json"),&serde_json::json!({"catalog":reference.encode().unwrap(),"chunk":chunk_reference.encode().unwrap(),"content":canonical,"body":owner.session.evidence().body_id,"boot":boot,"stage_absent":owner.source_recording_stage.is_none()})).unwrap();
        return;
    }
    let (root, disk, mut owner, publish) = fixture();
    retain_selection(&root, &disk);
    let stage = owner.source_recording_stage.as_ref().unwrap();
    let prepared = stage.prepared.as_ref().unwrap();
    let expected_content = prepared.snapshot.content().unwrap().to_vec();
    let expected_chunk = prepared.reference.encode().unwrap();
    let body = serde_json::to_value(&owner.session.evidence().body_id).unwrap();
    let boot = owner.host.advertisement().boot_id.as_str().to_owned();
    let worker = owner
        .start_service_source_publication(&root, 5000, &publish, 1024 * 1024)
        .unwrap();
    finish_worker(&root, &mut owner, worker);
    let read = independent_read_grant(&mut owner);
    let worker = owner
        .start_service_source_readback(&root, 5000, &read, 1024 * 1024)
        .unwrap();
    finish_worker(&root, &mut owner, worker);
    let worker = owner
        .start_service_source_catalog(&root, 5000, &publish, 1024 * 1024)
        .unwrap();
    finish_worker(&root, &mut owner, worker);
    let expected_catalog = owner
        .host
        .selected_body_storage_generation()
        .unwrap()
        .snapshot_reference()
        .unwrap()
        .encode()
        .unwrap();
    assert_eq!(owner.source_recording_stage.as_ref().unwrap().len(), 1);
    drop(owner);
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", TEST, "--nocapture"])
        .env(ROOT_ENV, &root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "child failed: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("fresh-source-read-result.json")).unwrap())
            .unwrap();
    assert_eq!(
        result["catalog"],
        serde_json::to_value(expected_catalog).unwrap()
    );
    assert_eq!(
        result["chunk"],
        serde_json::to_value(expected_chunk).unwrap()
    );
    assert_eq!(
        result["content"],
        serde_json::to_value(expected_content).unwrap()
    );
    assert_eq!(result["body"], body);
    assert_ne!(result["boot"], boot);
    assert_eq!(result["stage_absent"], true);
    std::fs::remove_dir_all(root).unwrap();
}
