//! Actual IndexedDB round-trip of production recording and snapshot bytes.
//! Root seed, identities and authority are fixtures; no live Body/Line is claimed.
use conduit_browser_runtime::resource_snapshot::PreparedSnapshotRecord;
use conduit_environment_runtime::{observation::*, recording::*};
use std::{path::Path, process::Command};
#[path = "recording_browser/binding.rs"]
mod binding;

#[test]
#[ignore = "requires pinned Chromium; cargo xtask prove environment recording-browser"]
fn overnight_catalog_and_chunks_survive_indexeddb_reopen_with_exact_quantities_and_failures() {
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
    let (_, template) = binding::placement("fixture/store/boot-one", true, extent);
    let references: Vec<_> = (1..=15)
        .map(|version| {
            let mut reference = template.clone();
            reference.lifetime.version =
                conduit_core::ResourceVersionIdentity::from_digest([version; 32]);
            reference
        })
        .collect();
    let catalog = PreparedRecordingCatalog::prepare(&references, 16, 1_048_576).unwrap();
    let (_, mut catalog_reference) =
        binding::placement("fixture/store/boot-one", true, catalog.content().len());
    catalog_reference.identity = conduit_core::ResourceSemanticIdentity::from_digest([9; 32]);
    catalog_reference.lifetime.version =
        conduit_core::ResourceVersionIdentity::from_digest([99; 32]);
    let mut forms = PreparedObservationQuantityForms::prepare(&contract).unwrap();
    let cold = forms.unpack(&[198, 0, 78, 2]).unwrap().to_vec();
    let warm = forms.unpack(&[208, 0, 88, 2]).unwrap().to_vec();
    let batch =
        PreparedRecordingBatch::prepare(context, &contract, 960, &references, 15, 1_048_576)
            .unwrap();
    let mut publications = Vec::new();
    for (index, mut generation) in batch.into_generations().into_iter().enumerate() {
        for offset in 1..=64 {
            let sequence = index as u64 * 64 + offset;
            let (disposition, observation) = sample(sequence, &cold, &warm);
            generation
                .encoder
                .append(sequence, sequence * 60_000, disposition, observation)
                .unwrap();
        }
        publications.push(publication(
            &references[index],
            generation.encoder.seal().unwrap(),
        ));
    }
    publications.push(publication(&catalog_reference, catalog.content()));
    let fixture = serde_json::json!({"publications": publications});
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
    let restored: Vec<(String, Vec<u8>)> =
        serde_json::from_slice(&std::fs::read(restored_file).unwrap()).unwrap();
    assert_eq!(restored.len(), 16);
    // Root supplies the catalog seed separately. The restored catalog, rather
    // than the original in-memory list, selects every subsequent content check.
    let catalog_bytes = restore(&catalog_reference, &restored);
    let mut selected = PreparedRecordingCatalogReader::default();
    selected
        .read(std::str::from_utf8(&catalog_bytes).unwrap())
        .unwrap();
    assert_eq!(selected.len(), 15);
    let mut history = PreparedRecordingHistory::prepare(context, &contract, 15).unwrap();
    let mut failures = 0;
    for (index, expected) in references.iter().enumerate() {
        let reference =
            conduit_core::BoundedResourceRef::decode(selected.encoded_reference(index).unwrap())
                .unwrap();
        assert_eq!(&reference, expected);
        let content = restore(&reference, &restored);
        history
            .read_next(std::str::from_utf8(&content).unwrap())
            .unwrap();
        assert_eq!(history.loaded_samples(), 64);
        for offset in 0..64 {
            let actual = history.sample(offset).unwrap();
            let sequence = index as u64 * 64 + offset as u64 + 1;
            let (disposition, observation) = sample(sequence, &cold, &warm);
            assert_eq!(actual.sequence, sequence);
            assert_eq!(actual.sampled_at_boot_ms, sequence * 60_000);
            assert_eq!(actual.disposition, disposition);
            assert_eq!(actual.observation, observation);
            assert_eq!(actual.gap_before, None);
            failures += usize::from(observation.is_none());
        }
    }
    assert_eq!(
        history.recovered(),
        RecoveredHistory {
            chunks: 15,
            samples: 960,
            first_sequence: Some(1),
            last_sequence: Some(960)
        }
    );
    assert_eq!(failures, 56);
    println!("real_indexeddb_reopen_chunks=15 catalog_records=1 samples=960 logical_elapsed_ms=57600000 exact_available_observations=904 exact_sensor_failures=56 old_authority_refused=true corruption_refused=true");
}
fn sample<'a>(
    sequence: u64,
    cold: &'a [u8],
    warm: &'a [u8],
) -> (RecordingDisposition, Option<&'a [u8]>) {
    if sequence.is_multiple_of(17) {
        (RecordingDisposition::ProviderFailed, None)
    } else {
        (
            RecordingDisposition::Available,
            Some(if sequence.is_multiple_of(2) {
                cold
            } else {
                warm
            }),
        )
    }
}
fn publication(reference: &conduit_core::BoundedResourceRef, content: &str) -> serde_json::Value {
    conduit_web::validate_json_text(content.as_bytes()).unwrap();
    let placement = binding::for_reference("fixture/store/boot-one", true, reference);
    let mut writer = PreparedSnapshotRecord::prepare(&placement, reference).unwrap();
    let record = writer
        .publication(&placement.authority[0], content.as_bytes())
        .unwrap()
        .to_vec();
    serde_json::json!({"key": writer.storage_key(), "record": record})
}
fn restore(
    reference: &conduit_core::BoundedResourceRef,
    restored: &[(String, Vec<u8>)],
) -> Vec<u8> {
    let placement = binding::for_reference("fixture/store/boot-two", false, reference);
    let reader = PreparedSnapshotRecord::prepare(&placement, reference).unwrap();
    let record = &restored
        .iter()
        .find(|(key, _)| key == reader.storage_key())
        .expect("selected exact generation was restored")
        .1;
    let content = reader
        .restore(&placement.authority[0], record)
        .unwrap()
        .to_vec();
    let old = binding::for_reference("fixture/store/boot-one", false, reference);
    assert!(reader.restore(&old.authority[0], record).is_err());
    let mut corrupt = record.clone();
    *corrupt.last_mut().unwrap() ^= 1;
    assert!(reader.restore(&placement.authority[0], &corrupt).is_err());
    content
}
