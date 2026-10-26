//! CPU NOR and cryptographic recovery proof; no live collector or radio.
use conduit_environment_runtime::{
    display_evidence_journal::{Journal, Refusal, Storage},
    observation::ObservationContract,
    observed_record::{self, ObservedRecord},
    recording::RecordingDisposition,
};
use ed25519_dalek::SigningKey;
#[allow(dead_code)]
#[path = "../../../../targets/esp32/proof/tm1637/src/native_local_history.rs"]
mod history;
#[allow(dead_code)]
#[path = "../../../../targets/esp32/proof/tm1637/src/native_observed_recovery.rs"]
mod recovery;
#[derive(Clone)]
struct Nor {
    bytes: Vec<u8>,
    remaining: Option<usize>,
}
impl Storage for Nor {
    fn capacity(&self) -> u32 {
        self.bytes.len() as u32
    }
    fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), Refusal> {
        out.copy_from_slice(
            self.bytes
                .get(at as usize..at as usize + out.len())
                .ok_or(Refusal::Media)?,
        );
        Ok(())
    }
    fn program(&mut self, at: u32, input: &[u8]) -> Result<(), Refusal> {
        assert!(at.is_multiple_of(4) && input.len().is_multiple_of(4));
        for (index, byte) in input.iter().enumerate() {
            if self.remaining == Some(0) {
                return Err(Refusal::Media);
            }
            let target = self
                .bytes
                .get_mut(at as usize + index)
                .ok_or(Refusal::Media)?;
            assert_eq!(*target & byte, *byte);
            *target &= byte;
            if let Some(remaining) = &mut self.remaining {
                *remaining -= 1;
            }
        }
        Ok(())
    }
}
fn record(
    key: &SigningKey,
    scope: &[u8; 96],
    forms: &[u8; 32],
    sequence: u64,
) -> [u8; history::SIGNED_BYTES] {
    let reading = ObservedRecord {
        sequence,
        observed_at_boot_ms: sequence * 60_000,
        disposition: RecordingDisposition::ProviderFailed,
        quantities: None,
    }
    .encode()
    .unwrap();
    let mut signed = [0; history::SIGNED_BYTES];
    signed[..observed_record::BYTES].copy_from_slice(&reading);
    signed[observed_record::BYTES..]
        .copy_from_slice(&history::sign(key, scope, forms, &reading).unwrap());
    signed
}
#[test]
fn exact_original_scope_forms_key_and_record_are_required_after_reboot() {
    let key = SigningKey::from_bytes(&[7; 32]);
    let scope = [9; 96];
    let forms = [3; 32];
    let signed = record(&key, &scope, &forms, 1);
    let (reading, signature) = signed.split_at(observed_record::BYTES);
    let signature = signature.try_into().unwrap();
    assert!(history::verify(&key.verifying_key(), &scope, &forms, reading, signature).is_ok());
    for component in 0..3 {
        let mut changed = scope;
        changed[component * 32] ^= 1;
        assert!(
            history::verify(&key.verifying_key(), &changed, &forms, reading, signature).is_err()
        );
    }
    assert!(history::verify(&key.verifying_key(), &scope, &[4; 32], reading, signature).is_err());
    assert!(history::verify(
        &SigningKey::from_bytes(&[8; 32]).verifying_key(),
        &scope,
        &forms,
        reading,
        signature
    )
    .is_err());
    let mut changed = reading.to_vec();
    changed[8] ^= 1;
    assert!(history::verify(&key.verifying_key(), &scope, &forms, &changed, signature).is_err());
}
#[test]
fn signed_epoch_fits_960_and_every_torn_append_preserves_original_prefix() {
    let key = SigningKey::from_bytes(&[7; 32]);
    let scope = [9; 96];
    let basis = observed_record::descriptor(&ObservationContract::prepare().unwrap()).unwrap();
    use sha2::{Digest, Sha256};
    let forms: [u8; 32] = Sha256::digest(basis).into();
    let mut work = [0; 536];
    let media = Nor {
        bytes: vec![255; 128 * 1024],
        remaining: None,
    };
    let mut journal = Journal::create(media, &scope, &basis, 960, &mut work).unwrap();
    journal
        .append(&record(&key, &scope, &forms, 1), &mut work)
        .unwrap();
    let old = journal.into_storage();
    for cut in 0..=112 {
        let mut media = old.clone();
        media.remaining = Some(cut);
        let (mut journal, restored_scope, sequence) =
            recovery::reopen_profile(media, &basis, &mut work, true).unwrap();
        assert_eq!(restored_scope, scope);
        assert_eq!(sequence, 1);
        let complete = journal
            .append(&record(&key, &scope, &forms, 2), &mut work)
            .is_ok();
        let mut media = journal.into_storage();
        media.remaining = None;
        let (mut recovered, restored_scope, sequence) =
            recovery::reopen_profile(media, &basis, &mut work, true).unwrap();
        assert_eq!(restored_scope, scope);
        assert_eq!(sequence, if complete { 2 } else { 1 });
        let mut cursor = recovered.replay_cursor();
        let mut bytes = [0; history::SIGNED_BYTES];
        while recovered
            .replay(&mut cursor, &mut bytes, &mut work)
            .unwrap()
            .is_some()
        {
            history::verify(
                &key.verifying_key(),
                &scope,
                &forms,
                &bytes[..observed_record::BYTES],
                bytes[observed_record::BYTES..].try_into().unwrap(),
            )
            .unwrap();
        }
    }
    let mut journal = Journal::recover(old, &scope, &basis, 960, &mut work).unwrap();
    for sequence in 2..=960 {
        journal
            .append(&record(&key, &scope, &forms, sequence), &mut work)
            .unwrap();
    }
    assert_eq!(journal.committed_count(), 960);
    assert!(journal.retained_extent() < 128 * 1024);
    assert!(journal
        .append(&record(&key, &scope, &forms, 961), &mut work)
        .is_err());
}
