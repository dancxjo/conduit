use super::*;
use crate::durable_host as installed;
use conduit_environment_measurement::source_observation::{
    PreparedObservationCodec, SourceFailure, SourceObservation, SourceOutcome,
};
use conduit_std_host::body_storage::{BodyStorageBounds, SelectedBodyStorage};
use conduit_std_host::{StdHost, StdHostConfig};
use std::{
    os::unix::fs::PermissionsExt,
    time::{Duration, Instant},
};
pub(crate) fn fixture() -> (
    std::path::PathBuf,
    std::path::PathBuf,
    Owner,
    c::AuthorityGrant,
) {
    let root = std::env::temp_dir().join(installed::fresh_identity("source-publication", "owner"));
    std::fs::create_dir_all(&root).unwrap();
    let disk = root.join("selected-storage");
    std::fs::create_dir(&disk).unwrap();
    std::fs::set_permissions(&disk, std::fs::Permissions::from_mode(0o700)).unwrap();
    let installation: installed::Installation = serde_json::from_value(serde_json::json!({"schema":installed::INSTALL_SCHEMA,"host_id":"host/publication","release_source_identity":"source/test","release_bundle_sha256":installed::digest(b"bundle/test"),"product_executable":"fixture-unused","body_state":null,"joined_body_state":null})).unwrap();
    installed::write_json_atomic(&root.join("installation.json"), &installation).unwrap();
    let mut host = StdHost::new_with_config(StdHostConfig {
        host_id: "host/publication".into(),
        boot_id: "boot/publication".into(),
        offer_generation: c::OfferGeneration(1),
    });
    host.attach_selected_body_storage(
        SelectedBodyStorage::open(
            &disk,
            "test/publication",
            BodyStorageBounds {
                maximum_snapshot_records: 2,
                maximum_record_bytes: 4096,
                generation_slots: 1,
                publication_slots: 1,
                reader_leases: 1,
                state_slots: 1,
                maximum_state_bytes: 4096,
                total_disk_bytes: 131072,
                preparation_bytes: 1024 * 1024,
            },
            None,
        )
        .unwrap(),
    )
    .unwrap();
    let text = "plot old {\n show: face/text(\"Old\")\n \"old\" >> show\n}\n";
    let plot = crate::plot_source::parse(text)
        .unwrap()
        .expand_entry_for_authoring()
        .unwrap();
    let mut owner = Owner::open(
        host,
        conduit_body::ResidentPlot::new(
            plot.expanded.source_document_id,
            plot.expanded.checked_plot_id,
        ),
        None,
        "Recorder fixture",
    )
    .unwrap();
    owner.persist(&root).unwrap();
    std::fs::write(root.join("body/source.conduit"), text).unwrap();
    let selected = Arc::new(SourceSelection {
        attachment_id: "green".into(),
        host_id: "host/publication".into(),
        boot_id: "boot/publication".into(),
        capability_id: "cap/source".into(),
        provider_instance_id: "base/source".into(),
        provider_generation: 1,
        plan_id: "plan/source".into(),
        fragment_id: "fragment/source".into(),
        placement_id: "placement/source".into(),
        configuration_id: "configuration/source".into(),
        firmware_sha256: [7; 32],
    });
    // Synthetic accepted codec prefix; this test qualifies publication custody,
    // not sensing or the prior native Source admission (proved separately).
    let mut stage =
        SourceRecordingStage::prepare(owner.session.evidence().body_id.clone(), selected.clone())
            .unwrap();
    let contract = ObservationContract::prepare().unwrap();
    let mut codec = PreparedObservationCodec::prepare(&contract).unwrap();
    let value = codec
        .encode(SourceObservation {
            observed_at_host_boot_ms: 10,
            outcome: SourceOutcome::Unavailable(SourceFailure::CaptureTimeout),
        })
        .unwrap();
    let mut bytes = [0; MAXIMUM_BYTES];
    bytes[..value.len()].copy_from_slice(value);
    stage
        .accept(&LiveFore {
            scope: Arc::new(CompletedScope {
                native_play: c::bind_active_play(
                    &selected.plan_id,
                    &selected.host_id,
                    &selected.boot_id,
                    1,
                ),
                selection: selected,
            }),
            sequence: 0,
            length: value.len(),
            bytes,
        })
        .unwrap();
    stage.prepare_checkpoint_input().unwrap();
    let reference = stage.prepared.as_ref().unwrap().reference.clone();
    owner.source_recording_stage = Some(stage);
    // Fixture Root explicitly authorizes publication on its actual selected
    // directory. Production helper accepts this grant; it never manufactures it.
    owner
        .host
        .current_mut()
        .select_body_storage_generation(BodyStorageGenerationRequest {
            snapshot: Some(BodyStorageSnapshotRequest {
                reference,
                access: c::ResourceAccessMode::WriteCandidatePublish,
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
    let requirement = &offer.authority_requirements[0];
    let grant = c::AuthorityGrant {
        grant_id: "fixture/explicit-publication-grant".into(),
        contract_id: requirement.contract_id.clone(),
        host_call_contract_id: requirement.host_call_contract_id.clone(),
        subject_kind: requirement.subject_kind.clone(),
        host_id: owner.host.advertisement().host_id.clone(),
        boot_id: owner.host.advertisement().boot_id.clone(),
        capability_id: offer.capability_id.clone(),
    };
    (root, disk, owner, grant)
}
#[test]
fn source_publication_worker_uses_original_host_and_retains_unacknowledged_prefix() {
    let (root, disk, mut owner, grant) = fixture();
    let body = owner.session.evidence().body_id.clone();
    let original = owner
        .source_recording_stage
        .as_ref()
        .unwrap()
        .observations()
        .next()
        .unwrap()
        .canonical()
        .to_vec();
    let prepared_pointer = owner
        .source_recording_stage
        .as_ref()
        .unwrap()
        .prepared
        .as_ref()
        .unwrap()
        .snapshot
        .content()
        .unwrap()
        .as_ptr();
    let mut worker = owner
        .start_service_source_publication(&root, 5000, &grant, 1024 * 1024)
        .unwrap();
    let until = Instant::now() + Duration::from_secs(10);
    while !worker.progress(&mut owner, &root).unwrap() {
        assert!(Instant::now() < until, "publication worker did not finish");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(!owner.host.is_playing());
    assert!(owner.current_play_id().is_none());
    assert_eq!(owner.session.evidence().body_id, body);
    assert_eq!(
        owner.host.advertisement().host_id.as_str(),
        "host/publication"
    );
    let stage = owner.source_recording_stage.as_ref().unwrap();
    assert_eq!(stage.len(), 1);
    assert_eq!(stage.observations().next().unwrap().canonical(), original);
    assert_eq!(
        stage
            .prepared
            .as_ref()
            .unwrap()
            .snapshot
            .content()
            .unwrap()
            .as_ptr(),
        prepared_pointer
    );
    let receipt = owner.last_execution.as_ref().unwrap();
    assert_eq!(
        receipt["source_publication"]["publication_completed"], true,
        "{receipt}"
    );
    assert_eq!(
        receipt["source_publication"]["matching_publication_fore"],
        true
    );
    assert_eq!(
        receipt["source_publication"]["producer_retirement_authorized"],
        false
    );
    assert_eq!(receipt["source_recording_pending"]["count"], 1);
    assert_eq!(std::fs::read_dir(&disk).unwrap().count(), 1);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn source_publication_stale_or_missing_grant_retains_original_prefix() {
    let (root, disk, mut owner, grant) = fixture();
    let resident = owner.resident.clone();
    let generation = owner.host.advertisement().offer_generation;
    let mut stale = grant.clone();
    stale.boot_id = "boot/stale".into();
    assert!(owner
        .prepare_source_recording_publication(&root, &stale, 1024 * 1024)
        .err()
        .unwrap()
        .contains("Host/Boot"));
    assert_eq!(owner.host.advertisement().offer_generation, generation);
    let mut absent = grant;
    absent.grant_id = "".into();
    assert!(owner
        .prepare_source_recording_publication(&root, &absent, 1024 * 1024)
        .err()
        .unwrap()
        .contains("independent exact"));
    assert!(owner.session.realization().is_none());
    assert_eq!(owner.resident, resident);
    assert_eq!(owner.source_recording_stage.as_ref().unwrap().len(), 1);
    assert!(owner
        .source_recording_stage
        .as_ref()
        .unwrap()
        .prepared
        .is_some());
    assert_eq!(std::fs::read_dir(&disk).unwrap().count(), 0);
    std::fs::remove_dir_all(root).unwrap();
}

fn finish_worker(root: &Path, owner: &mut Owner, mut worker: super::super::super::RunWorker) {
    let until = Instant::now() + Duration::from_secs(10);
    while !worker.progress(owner, root).unwrap() {
        assert!(
            Instant::now() < until,
            "selected storage worker did not finish"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn independent_read_grant(owner: &mut Owner) -> c::AuthorityGrant {
    let reference = owner
        .source_recording_stage
        .as_ref()
        .unwrap()
        .prepared
        .as_ref()
        .unwrap()
        .reference
        .clone();
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
    let requirement = &offer.authority_requirements[0];
    // Explicit fixture Root read authorization is independent of the prior
    // publication grant; neither the reference nor helper creates authority.
    c::AuthorityGrant {
        grant_id: "fixture/independent-snapshot-read-grant".into(),
        contract_id: requirement.contract_id.clone(),
        host_call_contract_id: requirement.host_call_contract_id.clone(),
        subject_kind: requirement.subject_kind.clone(),
        host_id: owner.host.advertisement().host_id.clone(),
        boot_id: owner.host.advertisement().boot_id.clone(),
        capability_id: offer.capability_id.clone(),
    }
}
#[test]
fn source_readback_worker_uses_independent_authority_and_retains_original_canonical_bytes() {
    let (root, disk, mut owner, publish) = fixture();
    let body = owner.session.evidence().body_id.clone();
    let boot = owner.host.advertisement().boot_id.clone();
    let stage = owner.source_recording_stage.as_ref().unwrap();
    let original = stage.observations().next().unwrap().canonical().to_vec();
    let content = stage
        .prepared
        .as_ref()
        .unwrap()
        .snapshot
        .content()
        .unwrap()
        .to_vec();
    let pointer = stage
        .prepared
        .as_ref()
        .unwrap()
        .snapshot
        .content()
        .unwrap()
        .as_ptr();
    let worker = owner
        .start_service_source_publication(&root, 5000, &publish, 1024 * 1024)
        .unwrap();
    finish_worker(&root, &mut owner, worker);
    assert_eq!(
        owner.last_execution.as_ref().unwrap()["source_publication"]["publication_completed"],
        true
    );
    assert!(owner
        .start_service_source_readback(&root, 5000, &publish, 1024 * 1024)
        .is_err());
    assert!(owner.session.realization().is_none());
    assert_eq!(owner.source_recording_stage.as_ref().unwrap().len(), 1);
    let read = independent_read_grant(&mut owner);
    assert_ne!(read.host_call_contract_id, publish.host_call_contract_id);
    assert_ne!(read.capability_id, publish.capability_id);
    // The admitted file/read authority suffices without a retained publication
    // reply. Clearing this process-local receipt does not manufacture a file.
    owner.last_execution = None;
    owner.persist(&root).unwrap();
    let worker = owner
        .start_service_source_readback(&root, 5000, &read, 1024 * 1024)
        .unwrap();
    finish_worker(&root, &mut owner, worker);
    assert!(!owner.host.is_playing());
    assert!(owner.current_play_id().is_none());
    assert_eq!(owner.session.evidence().body_id, body);
    assert_eq!(
        owner.host.advertisement().host_id.as_str(),
        "host/publication"
    );
    assert_eq!(owner.host.advertisement().boot_id, boot);
    let stage = owner.source_recording_stage.as_ref().unwrap();
    assert_eq!(stage.len(), 1);
    assert_eq!(stage.observations().next().unwrap().canonical(), original);
    assert_eq!(
        stage.prepared.as_ref().unwrap().snapshot.content().unwrap(),
        content
    );
    assert_eq!(
        stage
            .prepared
            .as_ref()
            .unwrap()
            .snapshot
            .content()
            .unwrap()
            .as_ptr(),
        pointer
    );
    let receipt = owner.last_execution.as_ref().unwrap();
    assert_eq!(
        receipt["source_readback"]["snapshot_readback_completed"], true,
        "{receipt}"
    );
    assert_eq!(receipt["source_readback"]["matching_content_fore"], true);
    assert_eq!(
        receipt["source_readback"]["producer_retirement_authorized"],
        false
    );
    assert_eq!(receipt["source_recording_pending"]["count"], 1);
    assert_eq!(std::fs::read_dir(&disk).unwrap().count(), 1);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn source_readback_corrupt_snapshot_never_claims_matching_content_or_discards_prefix() {
    let (root, disk, mut owner, publish) = fixture();
    let worker = owner
        .start_service_source_publication(&root, 5000, &publish, 1024 * 1024)
        .unwrap();
    finish_worker(&root, &mut owner, worker);
    assert_eq!(
        owner.last_execution.as_ref().unwrap()["source_publication"]["publication_completed"],
        true
    );
    let read = independent_read_grant(&mut owner);
    let file = std::fs::read_dir(&disk)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    // Deliberate fixture fault only. Success still requires the ordinary
    // admitted resource/read-json-snapshot Body Play, never this direct IO.
    std::fs::write(file, b"corrupt snapshot envelope").unwrap();
    let worker = owner
        .start_service_source_readback(&root, 5000, &read, 1024 * 1024)
        .unwrap();
    finish_worker(&root, &mut owner, worker);
    assert!(!owner.host.is_playing());
    assert!(owner.current_play_id().is_none());
    let receipt = owner.last_execution.as_ref().unwrap();
    assert_eq!(
        receipt["source_readback"]["snapshot_readback_completed"], false,
        "{receipt}"
    );
    assert_eq!(receipt["source_readback"]["matching_content_fore"], false);
    assert_eq!(
        receipt["source_readback"]["producer_retirement_authorized"],
        false
    );
    assert_eq!(owner.source_recording_stage.as_ref().unwrap().len(), 1);
    assert!(owner
        .source_recording_stage
        .as_ref()
        .unwrap()
        .prepared
        .is_some());
    std::fs::remove_dir_all(root).unwrap();
}
