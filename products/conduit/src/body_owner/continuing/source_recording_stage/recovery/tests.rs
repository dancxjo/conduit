use super::super::publication::tests::{finish_worker, fixture, independent_read_grant};
use super::*;
use crate::durable_host as installed;
use conduit_std_host::{StdHost, StdHostConfig};
const ROOT_ENV: &str = "CONDUIT_CATALOG_RECOVERY_TEST_ROOT";
const TEST: &str = "durable_host::owner::controller::continuing::source_recording_stage::recovery::tests::fresh_process_recovers_unknown_catalog_reference_from_body_state";
#[test]
fn fresh_process_recovers_unknown_catalog_reference_from_body_state() {
    if let Some(root) = std::env::var_os(ROOT_ENV) {
        let root = std::path::PathBuf::from(root);
        let installation = installed::read_installation(&root.join("installation.json")).unwrap();
        let boot = installed::fresh_identity("boot", "catalog-recovery");
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
        let receipt = owner.last_execution.as_ref().unwrap();
        assert_eq!(
            receipt["source_catalog_recovery"]["state_current_observed"],
            true
        );
        assert_eq!(
            receipt["source_catalog_recovery"]["catalog_reference_recovered"],
            true
        );
        assert_eq!(
            receipt["source_catalog_recovery"]["producer_retirement_authorized"],
            false
        );
        let token = owner.recovered_source_catalog.as_ref().unwrap();
        assert_eq!(
            token.binding().body.as_str(),
            owner.session.evidence().body_id.as_str()
        );
        let _issued_play = token.play();
        installed::write_json_atomic(
            &root.join("fresh-recovery-result.json"),
            &serde_json::json!({
                "reference":token.reference().encode().unwrap(), "binding":binding_json(token.binding()),
                "body":owner.session.evidence().body_id, "boot":boot,
                "stage_absent":owner.source_recording_stage.is_none(),
            }),
        )
        .unwrap();
        return;
    }
    let (root, disk, mut owner, publish) = fixture();
    // Ordinary explicit installed selection records the real device/inode. The
    // child receives only the installation root, never a reference or grant.
    let source = include_str!("../../../../../../../targets/std/profiles/linux-computer.host.conduit").replace(
        r#"base = {kind: "storage/protected-file", implementation: "hosted/protected-file@1"}"#,
        &format!(r#"base = {{kind: "storage/protected-file", implementation: "linux/body-storage-directory@1", initialization: {{root_directory: {:?}, pool_namespace: "test/publication", maximum_snapshot_records: 2, maximum_record_bytes: 4096, generation_slots: 1, publication_slots: 1, reader_leases: 1, state_slots: 1, maximum_state_bytes: 4096, total_disk_bytes: 131072, preparation_bytes: 1048576}}}}"#, disk.to_str().unwrap()));
    let source_path = root.join("recovery.host.conduit");
    std::fs::write(&source_path, source).unwrap();
    installed::selected_host_configuration::configure(&root, Some(&source_path), false).unwrap();
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
    let selected = owner.host.selected_body_storage_generation().unwrap();
    let reference = selected.snapshot_reference().unwrap().encode().unwrap();
    let binding = binding_json(selected.state_binding().unwrap());
    let body = serde_json::to_value(&owner.session.evidence().body_id).unwrap();
    let old_boot = owner.host.advertisement().boot_id.as_str().to_owned();
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
        serde_json::from_slice(&std::fs::read(root.join("fresh-recovery-result.json")).unwrap())
            .unwrap();
    assert_eq!(
        result["reference"],
        serde_json::to_value(reference).unwrap()
    );
    assert_eq!(result["binding"], binding);
    assert_eq!(result["body"], body);
    assert_ne!(result["boot"], old_boot);
    assert_eq!(result["stage_absent"], true);
    std::fs::remove_dir_all(root).unwrap();
}

fn binding_json(binding: &conduit_std_host::state_value::DurableStateBinding) -> serde_json::Value {
    serde_json::json!({"body":binding.body,"state":binding.state,"value_kind":binding.value_kind,"maximum_value_bytes":binding.maximum_value_bytes})
}
