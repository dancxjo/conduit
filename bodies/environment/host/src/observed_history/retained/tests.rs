use super::*;
use conduit_core::{Quantity, Unit};
use conduit_environment_runtime::recording::RecordingDisposition;
use ed25519_dalek::{Signer, SigningKey};
use std::os::unix::fs::{symlink, PermissionsExt};

struct Fixture {
    directory: std::path::PathBuf,
    collector: Collector,
    key: SigningKey,
}
impl Fixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let index = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "conduit-retained-tail-{}-{index}",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
        let key = SigningKey::from_bytes(&[29; 32]);
        let collector =
            Collector::from_authenticated_scope([7; 96], &key.verifying_key().to_bytes()).unwrap();
        Self {
            directory,
            collector,
            key,
        }
    }
    fn batch(&self, after: u32, end: u32) -> std::path::PathBuf {
        let records: Vec<_> = (after + 1..=end)
            .flat_map(|sequence| {
                ObservedRecord {
                    sequence: sequence.into(),
                    observed_at_boot_ms: u64::from(sequence) * 1000,
                    disposition: RecordingDisposition::Available,
                    quantities: Some((
                        Quantity::new(23, Unit::Celsius),
                        Quantity::new(46, Unit::Percent),
                    )),
                }
                .encode()
                .unwrap()
            })
            .collect();
        let signature = self.key.sign(&wire::transcript(
            &self.collector.scope,
            &self.collector.forms,
            after,
            end,
            &records,
        ));
        let json = serde_json::json!({
            "profile":"observed-history@1", "scope":encoded(&self.collector.scope),
            "forms":encoded(&self.collector.forms), "after":after, "committed":end,
            "records":encoded(&records), "signature":encoded(&signature.to_bytes()),
        });
        let path = self
            .directory
            .join(format!("observed-history-{after:04}-{end:04}.json"));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .unwrap();
        file.write_all(&serde_json::to_vec(&json).unwrap()).unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.directory).unwrap();
    }
}
#[test]
fn bounded_tail_preserves_original_canonical_bytes_after_full_prefix_verification() {
    let f = Fixture::new();
    for start in (0..20).step_by(4) {
        f.batch(start, start + 4);
    }
    fs::write(f.directory.join("interrupted.pending"), b"torn input").unwrap();
    let (end, bytes) = tail(&f.collector, &f.directory).unwrap();
    assert_eq!(end, 20);
    assert_eq!(bytes.len(), 16 * observed_record::BYTES);
    let records: Vec<_> = bytes
        .as_chunks::<{ observed_record::BYTES }>()
        .0
        .iter()
        .map(|b| ObservedRecord::decode(b).unwrap())
        .collect();
    assert_eq!(records.first().unwrap().sequence, 5);
    assert_eq!(records.last().unwrap().sequence, 20);
    assert_eq!(records[0].observed_at_boot_ms, 5000);
    assert_eq!(
        records[0].quantities,
        Some((
            Quantity::new(23, Unit::Celsius),
            Quantity::new(46, Unit::Percent)
        ))
    );
}
#[test]
fn corruption_outside_visible_tail_still_refuses_the_entire_projection() {
    let f = Fixture::new();
    for start in (0..20).step_by(4) {
        f.batch(start, start + 4);
    }
    let path = f.directory.join("observed-history-0000-0004.json");
    let mut d: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    d["signature"] = "00".repeat(64).into();
    fs::write(path, serde_json::to_vec(&d).unwrap()).unwrap();
    assert!(tail(&f.collector, &f.directory).is_err());
}
#[test]
fn missing_prefix_and_symbolic_links_are_not_retained_evidence() {
    let f = Fixture::new();
    f.batch(1, 2);
    assert!(tail(&f.collector, &f.directory).is_err());
    fs::remove_file(f.directory.join("observed-history-0001-0002.json")).unwrap();
    let path = f.batch(0, 1);
    let other = f.directory.join("original.private");
    fs::rename(&path, &other).unwrap();
    symlink(&other, &path).unwrap();
    assert!(tail(&f.collector, &f.directory).is_err());
}
