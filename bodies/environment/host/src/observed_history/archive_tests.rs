use super::super::tests::signed;
use super::*;
use ed25519_dalek::SigningKey;
use std::sync::atomic::{AtomicU64, Ordering};
struct Fixture {
    directory: std::path::PathBuf,
    collector: Collector,
    bytes: Vec<u8>,
}
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let directory = std::env::temp_dir().join(format!(
            "conduit-retained-archive-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).unwrap();
        let key = SigningKey::from_bytes(&[7; 32]);
        let collector = Collector {
            scope: [3; 96],
            forms: [4; 32],
            key: key.verifying_key(),
            after: 0,
            observed_ms: 0,
        };
        let record = ObservedRecord {
            sequence: 1,
            observed_at_boot_ms: 123,
            disposition:
                conduit_environment_runtime::recording::RecordingDisposition::ProviderFailed,
            quantities: None,
        }
        .encode()
        .unwrap();
        let bytes = signed(&key, collector.scope, collector.forms, 0, 1, &record);
        std::fs::write(directory.join("observed-history-0000-0001.json"), &bytes).unwrap();
        Self {
            directory,
            collector,
            bytes,
        }
    }
    fn limits(&self) -> Limits {
        Limits {
            directory_entries: 1,
            batches: 1,
            bytes: self.bytes.len(),
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}
#[test]
fn frozen_original_bytes_survive_source_change_and_revoke_refuses_atomically() {
    let f = Fixture::new();
    let mut archive = f
        .collector
        .prepare_archive(&f.directory, f.limits())
        .unwrap();
    assert_eq!(
        archive.retained_layout_bytes(),
        std::mem::size_of::<Archive>() + std::mem::size_of::<Entry>() + f.bytes.len()
    );
    std::fs::write(
        f.directory.join("observed-history-0000-0001.json"),
        b"changed",
    )
    .unwrap();
    let mut output = [0xa5; 2048];
    let len = archive.read_signed_batch(0, &mut output).unwrap();
    assert_eq!(&output[..len], f.bytes);
    assert!(output[len..].iter().all(|byte| *byte == 0xa5));
    let original = output;
    assert_eq!(
        archive.read_signed_batch(1, &mut output),
        Err(HistoryProviderRefusal::InvalidCursor)
    );
    assert_eq!(output, original);
    archive.revoke();
    assert_eq!(
        archive.read_signed_batch(0, &mut output),
        Err(HistoryProviderRefusal::Unavailable)
    );
    assert_eq!(output, original);
}
#[test]
fn directory_batch_and_byte_limits_refuse_before_exposure() {
    let f = Fixture::new();
    let mut limits = f.limits();
    limits.bytes -= 1;
    assert!(f.collector.prepare_archive(&f.directory, limits).is_err());
    std::fs::write(f.directory.join("unrelated"), b"not an observation").unwrap();
    assert!(f
        .collector
        .prepare_archive(&f.directory, f.limits())
        .is_err());
    std::fs::remove_file(f.directory.join("unrelated")).unwrap();
    let mut limits = f.limits();
    limits.batches = 0;
    assert!(f.collector.prepare_archive(&f.directory, limits).is_err());
}
#[test]
fn foreign_scope_corruption_and_symlinks_cannot_become_a_provider() {
    let mut f = Fixture::new();
    f.collector.scope[0] ^= 1;
    assert!(f
        .collector
        .prepare_archive(&f.directory, f.limits())
        .is_err());
    f.collector.scope[0] ^= 1;
    let path = f.directory.join("observed-history-0000-0001.json");
    let mut corrupted: serde_json::Value = serde_json::from_slice(&f.bytes).unwrap();
    let signature = corrupted["signature"].as_str().unwrap();
    let first = if signature.starts_with('0') { "1" } else { "0" };
    corrupted["signature"] = format!("{first}{}", &signature[1..]).into();
    std::fs::write(&path, serde_json::to_vec(&corrupted).unwrap()).unwrap();
    assert!(f
        .collector
        .prepare_archive(&f.directory, f.limits())
        .is_err());
    std::fs::write(&path, &f.bytes).unwrap();
    let original = f.directory.join("original");
    std::fs::rename(&path, &original).unwrap();
    std::os::unix::fs::symlink(&original, &path).unwrap();
    let mut limits = f.limits();
    limits.directory_entries = 2;
    assert!(f.collector.prepare_archive(&f.directory, limits).is_err());
    std::fs::remove_file(path).unwrap();
    std::fs::remove_file(original).unwrap();
}
#[test]
fn missing_advertised_tail_refuses_and_complete_multiple_batches_preserve_cursors() {
    let f = Fixture::new();
    let key = SigningKey::from_bytes(&[7; 32]);
    let record = |sequence| {
        ObservedRecord {
            sequence,
            observed_at_boot_ms: sequence * 100,
            disposition:
                conduit_environment_runtime::recording::RecordingDisposition::ProviderFailed,
            quantities: None,
        }
        .encode()
        .unwrap()
    };
    let records: Vec<_> = (1..=4).flat_map(record).collect();
    let first = signed(&key, f.collector.scope, f.collector.forms, 0, 5, &records);
    std::fs::remove_file(f.directory.join("observed-history-0000-0001.json")).unwrap();
    std::fs::write(f.directory.join("observed-history-0000-0004.json"), &first).unwrap();
    let limits = || Limits {
        directory_entries: 2,
        batches: 2,
        bytes: 4096,
    };
    assert!(f.collector.prepare_archive(&f.directory, limits()).is_err());
    let last = signed(&key, f.collector.scope, f.collector.forms, 4, 5, &record(5));
    std::fs::write(f.directory.join("observed-history-0004-0005.json"), &last).unwrap();
    let mut archive = f.collector.prepare_archive(&f.directory, limits()).unwrap();
    let mut out = [0; 2048];
    let len = archive.read_signed_batch(0, &mut out).unwrap();
    assert_eq!(&out[..len], &first);
    let len = archive.read_signed_batch(4, &mut out).unwrap();
    assert_eq!(&out[..len], &last);
    assert_eq!(
        archive.read_signed_batch(2, &mut out),
        Err(HistoryProviderRefusal::InvalidCursor)
    );
}

#[test]
#[ignore = "requires an explicitly pinned copy of physical signed evidence"]
fn physical_retained_batches_preserve_original_signed_bytes() {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        directory: std::path::PathBuf,
        scope: Vec<u8>,
        root: Vec<u8>,
        records: u32,
        records_sha256: String,
    }
    let path = std::env::var("CONDUIT_RETAINED_HISTORY_PROOF").unwrap();
    let mut bytes = Vec::new();
    File::open(path)
        .unwrap()
        .take(4097)
        .read_to_end(&mut bytes)
        .unwrap();
    assert!(bytes.len() <= 4096);
    let input: Input = serde_json::from_slice(&bytes).unwrap();
    let root: [u8; 32] = input.root.try_into().unwrap();
    let mut collector =
        Collector::from_authenticated_scope(input.scope.try_into().unwrap(), &root).unwrap();
    let mut archive = collector
        .prepare_archive(
            &input.directory,
            Limits {
                directory_entries: 960,
                batches: 960,
                bytes: 960 * 2048,
            },
        )
        .unwrap();
    let cursors: Vec<_> = archive.entries.iter().map(|entry| entry.after).collect();
    let mut output = [0; 2048];
    let mut digest = Sha256::new();
    for cursor in cursors {
        assert_eq!(cursor, collector.after);
        let len = archive.read_signed_batch(cursor, &mut output).unwrap();
        let (after, clock) = collector.validate(&output[..len]).unwrap();
        let batch: Batch = serde_json::from_slice(&output[..len]).unwrap();
        digest.update(hex(&batch.records).unwrap());
        collector.after = after;
        collector.observed_ms = clock;
    }
    let digest: String = digest
        .finalize()
        .iter()
        .map(|v| format!("{v:02x}"))
        .collect();
    assert_eq!(collector.after, input.records);
    assert_eq!(digest, input.records_sha256);
    println!(
        "physical retained provider: records={} original-records-sha256={digest}",
        collector.after
    );
}
