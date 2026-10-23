//! Actual IndexedDB round-trip of production recording and snapshot bytes.
//! Identity/authority bindings are fixtures; no live Body or Line is claimed.
use conduit_browser_runtime::resource_snapshot::PreparedSnapshotRecord;
use conduit_environment_runtime::{observation::*, recording::*};
use std::{path::Path, process::Command};
#[path = "recording_browser/binding.rs"]
mod binding;

#[test]
#[ignore = "requires pinned Chromium; cargo xtask prove environment recording-browser"]
fn full_chunk_survives_indexeddb_reopen_and_recovers_first_class_quantities() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .unwrap();
    let output = root.join("target/environment-recording-browser");
    std::fs::create_dir_all(&output).unwrap();
    let restored_file = output.join("restored.json");
    if restored_file.exists() {
        std::fs::remove_file(&restored_file).unwrap();
    }
    let host = "fixture/producer".into();
    let boot = "fixture/producer/boot".into();
    let plan = "fixture/producer/plan".into();
    let play = "fixture/producer/play".into();
    let context = RecordingContext {
        host: &host,
        boot: &boot,
        plan: &plan,
        play: &play,
    };
    let contract = ObservationContract::prepare().unwrap();
    let extent = PreparedRecordingSnapshot::content_bytes(context, &contract, 64).unwrap();
    let (write, reference) = binding::placement("fixture/store/boot-one", true, extent);
    let mut snapshot =
        PreparedRecordingSnapshot::prepare(context, &contract, 64, &reference).unwrap();
    let mut forms = PreparedObservationQuantityForms::prepare(&contract).unwrap();
    let observation = forms.unpack(&[198, 0, 78, 2]).unwrap();
    for sequence in 1..=64 {
        snapshot
            .append(
                sequence,
                sequence * 60_000,
                RecordingDisposition::Available,
                Some(observation),
            )
            .unwrap();
    }
    let content = snapshot.seal().unwrap();
    conduit_web::validate_json_text(content.as_bytes()).unwrap();
    let mut writer = PreparedSnapshotRecord::prepare(&write, &reference).unwrap();
    let record = writer
        .publication(&write.authority[0], content.as_bytes())
        .unwrap()
        .to_vec();
    let fixture = serde_json::json!({"key": writer.storage_key(), "record": record});
    std::fs::write(
        output.join("publication.json"),
        serde_json::to_vec(&fixture).unwrap(),
    )
    .unwrap();
    let status = Command::new("node")
        .current_dir(&root)
        .arg("proof/browser/node_modules/@playwright/test/cli.js")
        .args([
            "test",
            "--config",
            "proof/browser/environment-recording.config.mjs",
            "--project",
            "chromium",
            "--workers=1",
            "--retries=0",
        ])
        .status()
        .expect("pinned Playwright must be installed through browser setup");
    assert!(status.success(), "IndexedDB proof failed; retained artifacts are under target/environment-recording-browser and proof/browser/test-results");
    let restored: Vec<u8> = serde_json::from_slice(&std::fs::read(restored_file).unwrap()).unwrap();
    assert_eq!(restored, record);
    let (read, _) = binding::placement("fixture/store/boot-two", false, extent);
    let reader = PreparedSnapshotRecord::prepare(&read, &reference).unwrap();
    let recovered = reader.restore(&read.authority[0], &restored).unwrap();
    assert_eq!(recovered, content.as_bytes());
    assert!(reader.restore(&write.authority[0], &restored).is_err());
    let mut corrupt = restored.clone();
    *corrupt.last_mut().unwrap() ^= 1;
    assert!(reader.restore(&read.authority[0], &corrupt).is_err());
    let mut history = PreparedRecordingReader::prepare(context, &contract).unwrap();
    history
        .read(std::str::from_utf8(recovered).unwrap())
        .unwrap();
    assert_eq!(history.len(), 64);
    for index in 0..64 {
        let sample = history.sample(index).unwrap();
        assert_eq!(sample.sequence, index as u64 + 1);
        assert_eq!(sample.sampled_at_boot_ms, (index as u64 + 1) * 60_000);
        assert_eq!(sample.observation, Some(observation));
    }
    println!("real_indexeddb_reopen_samples=64 snapshot_record_bytes={} first_class_observations_equal=true old_authority_refused=true corruption_refused=true", record.len());
}
