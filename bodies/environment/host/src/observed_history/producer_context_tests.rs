//! Real signature/decoder correspondence tests; synthetic membership pins only.
use super::*;
use ed25519_dalek::{Signer, SigningKey};
use std::{
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::PathBuf,
};

struct Fixture {
    directory: PathBuf,
    binding: ValidatedBinding,
    key: SigningKey,
    collector: Collector,
    active: ActivePlayIdentity,
}
impl Fixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let directory = std::env::temp_dir().join(format!(
            "conduit-producer-context-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .unwrap();
        let key = SigningKey::from_bytes(&[51; 32]);
        let host: conduit_core::HostId = "producer/actual".into();
        let boot: conduit_core::BootId = "boot/original".into();
        let active = conduit_core::bind_active_play(&"plan/original".into(), &host, &boot, 7);
        let credential = serde_json::from_value(serde_json::json!({
            "credential_id":"01".repeat(32), "body_id":"02".repeat(32),
            "part_id":"03".repeat(32), "host_id":"observer/actual",
            "boot_id":"boot/observer", "issued_at_millis":123,
        }))
        .unwrap();
        let mut receipt = crate::local_recorder::joined::JoinedHostReceipt {
            schema: crate::local_recorder::joined::PROFILE.into(),
            credential,
            membership_revision: 2,
            producer_host: host,
            producer_boot: boot,
            producer_key: key.verifying_key().to_bytes(),
            invitation_id: serde_json::from_value(serde_json::json!("08".repeat(32))).unwrap(),
            offer_generation: conduit_core::OfferGeneration(1),
            signature: Vec::new(),
        };
        receipt.signature = key
            .sign(&crate::local_recorder::joined::transcript(&receipt.scope()))
            .to_bytes()
            .to_vec();
        let binding_path = directory.join("binding.json");
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&binding_path)
            .unwrap();
        serde_json::to_writer(&mut file, &serde_json::json!({
            "schema":"conduit.environment/joined-recorder@1",
            "invitation":{"invitation_id":receipt.invitation_id,"body_id":receipt.credential.body_id,"nonce":vec![3;32],"expires_at_millis":999},
            "root_key":key.verifying_key().to_bytes(), "local_host":receipt.credential.host_id,
            "local_boot":receipt.credential.boot_id,"receipt":receipt,
        })).unwrap();
        let binding = crate::local_recorder::validated_binding(&binding_path).unwrap();
        let mut scope = [0; 96];
        for (part, value) in scope.as_chunks_mut::<32>().0.iter_mut().zip([
            binding.credential().body_id.as_str(),
            active.boot_id.as_str(),
            active.active_play_id.as_str(),
        ]) {
            part.copy_from_slice(&Sha256::digest(value.as_bytes()));
        }
        let collector = Collector::from_authenticated_scope(scope, binding.root()).unwrap();
        Self {
            directory,
            binding,
            key,
            collector,
            active,
        }
    }
    fn path(&self) -> PathBuf {
        self.directory.join("observed-history-0000-0001.json")
    }
    fn record(sequence: u64) -> [u8; observed_record::BYTES] {
        ObservedRecord {
            sequence,
            observed_at_boot_ms: sequence * 100,
            disposition:
                conduit_environment_runtime::recording::RecordingDisposition::ProviderFailed,
            quantities: None,
        }
        .encode()
        .unwrap()
    }
    fn write(&self, scope: [u8; 96], records: &[u8], committed: u32) -> PathBuf {
        let after = records.len() / observed_record::BYTES;
        let path = self
            .directory
            .join(format!("observed-history-0000-{after:04}.json"));
        let bytes = crate::observed_history::tests::signed(
            &self.key,
            scope,
            self.collector.forms,
            0,
            committed,
            records,
        );
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&path)
            .unwrap();
        file.write_all(&bytes).unwrap();
        path
    }
    fn prepare<'a>(
        &'a self,
        collector: &Collector,
        active: &'a ActivePlayIdentity,
    ) -> std::result::Result<VerifiedProducerContext<'a>, Refusal> {
        VerifiedProducerContext::prepare(&self.binding, collector, active, &self.directory)
    }
    fn assert_refused_unchanged(&self, collector: &Collector, active: &ActivePlayIdentity) {
        let before = std::fs::read(self.path()).unwrap();
        assert!(self.prepare(collector, active).is_err());
        assert_eq!(std::fs::read(self.path()).unwrap(), before);
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.directory).unwrap();
    }
}
#[test]
fn exact_bound_original_context_is_separate_from_observer_residence() {
    let f = Fixture::new();
    let record = Fixture::record(1);
    f.write(f.collector.scope, &record, 1);
    let verified = f.prepare(&f.collector, &f.active).unwrap();
    let context = verified.context();
    assert_eq!(context.host, &f.active.host_id);
    assert_eq!(context.boot, &f.active.boot_id);
    assert_ne!(context.boot, &f.binding.credential().boot_id);
    assert_eq!(context.plan, &f.active.plan_id);
    assert_eq!(context.play, &f.active.active_play_id);
    assert_eq!(
        context.timestamp_basis,
        RecordingTimestampBasis::ProducerObservation
    );
    assert_eq!(
        verified.verified_prefix(),
        (1, Sha256::digest(record).into())
    );
    assert_eq!(verified.producer_committed_watermark(), 1);
}
#[test]
fn candidates_cannot_replace_root_body_host_or_canonical_scope() {
    let f = Fixture::new();
    f.write(f.collector.scope, &Fixture::record(1), 1);
    let mut foreign = Collector { ..f.collector };
    foreign.key = SigningKey::from_bytes(&[52; 32]).verifying_key();
    f.assert_refused_unchanged(&foreign, &f.active);
    foreign = Collector { ..f.collector };
    foreign.scope[0] ^= 1;
    f.assert_refused_unchanged(&foreign, &f.active);
    for field in 0..5 {
        let mut changed = f.active.clone();
        match field {
            0 => changed.host_id = "producer/foreign".into(),
            1 => changed.boot_id = "boot/foreign".into(),
            2 => changed.plan_id = "plan/foreign".into(),
            3 => changed.play_sequence += 1,
            _ => changed.active_play_id = changed.active_play_id.as_str().to_uppercase().into(),
        }
        f.assert_refused_unchanged(&f.collector, &changed);
    }
}
#[test]
fn empty_corrupt_wrong_scope_and_holes_refuse_without_mutation() {
    let f = Fixture::new();
    assert_eq!(
        f.prepare(&f.collector, &f.active).unwrap_err(),
        Refusal::Empty
    );
    f.write(f.collector.scope, &Fixture::record(1), 1);
    let original = std::fs::read(f.path()).unwrap();
    std::fs::write(f.path(), b"corrupt").unwrap();
    f.assert_refused_unchanged(&f.collector, &f.active);
    std::fs::remove_file(f.path()).unwrap();
    let mut wrong = f.collector.scope;
    wrong[95] ^= 1;
    f.write(wrong, &Fixture::record(1), 1);
    f.assert_refused_unchanged(&f.collector, &f.active);
    std::fs::write(f.path(), &original).unwrap();
    std::fs::rename(
        f.path(),
        f.directory.join("observed-history-0001-0002.json"),
    )
    .unwrap();
    assert!(f.prepare(&f.collector, &f.active).is_err());
}
#[test]
fn retained_prefix_may_lag_signed_producer_watermark() {
    let f = Fixture::new();
    let records: Vec<_> = (1..=4).flat_map(Fixture::record).collect();
    let path = f.write(f.collector.scope, &records, 5);
    let before = std::fs::read(&path).unwrap();
    let verified = f.prepare(&f.collector, &f.active).unwrap();
    assert_eq!(verified.verified_prefix().0, 4);
    assert_eq!(verified.producer_committed_watermark(), 5);
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn nonregular_batch_is_refused_without_changing_other_files() {
    let f = Fixture::new();
    let binding = f.directory.join("binding.json");
    let before = std::fs::read(&binding).unwrap();
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(f.path())
        .unwrap();
    assert_eq!(
        f.prepare(&f.collector, &f.active).unwrap_err(),
        Refusal::Prefix
    );
    assert!(f.path().is_dir());
    assert_eq!(std::fs::read(binding).unwrap(), before);
}
#[test]
fn canonical_correspondence_does_not_add_execution_sequence_policy() {
    let mut f = Fixture::new();
    f.active =
        conduit_core::bind_active_play(&f.active.plan_id, &f.active.host_id, &f.active.boot_id, 0);
    f.collector.scope[64..]
        .copy_from_slice(&Sha256::digest(f.active.active_play_id.as_str().as_bytes()));
    f.write(f.collector.scope, &Fixture::record(1), 1);
    assert!(f.prepare(&f.collector, &f.active).is_ok());
}

#[test]
fn verified_records_are_immutable_after_source_replacement_or_removal() {
    let f = Fixture::new();
    let original = Fixture::record(1);
    let path = f.write(f.collector.scope, &original, 1);
    let verified = f.prepare(&f.collector, &f.active).unwrap();
    assert_eq!(verified.retained_record_bytes(), observed_record::BYTES);
    std::fs::write(&path, b"changed after verification").unwrap();
    assert_eq!(verified.records().len(), 1);
    assert_eq!(
        verified.records().next().unwrap().encode().unwrap(),
        original
    );
    std::fs::remove_file(path).unwrap();
    assert_eq!(
        verified.records().next().unwrap().encode().unwrap(),
        original
    );
    assert_eq!(
        verified.verified_prefix(),
        (1, Sha256::digest(original).into())
    );
}
