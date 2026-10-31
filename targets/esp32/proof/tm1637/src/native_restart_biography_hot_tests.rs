//! CPU Store/parked-owner semantics; never a real receiver ACK or hardware proof.
use super::*;
use crate::native_body_checkpoint::Storage;
use ed25519_dalek::{Signer, SigningKey};
#[cfg(feature = "observation-store")]
#[path = "native_restart_biography_hot_private_tests.rs"]
mod private;
#[cfg(feature = "observation-store")]
#[path = "native_restart_biography_hot_window_tests.rs"]
mod window;
fn fixture() -> (
    Store,
    ParkedBody,
    conduit_core::HostAdvertisement,
    [u8; 32],
    BodyBiographyEvidence,
    Vec<u8>,
) {
    let (mut store, mut owner, host, root) = super::super::super::park_tests::fixture();
    let evidence = owner.as_ref().unwrap().evidence().clone();
    let admissions = serde_json::to_vec(&store.admissions).unwrap();
    let token = park_store(&mut store, &mut owner, &root, &host.host_id, &host.boot_id).unwrap();
    (store, token, host, root, evidence, admissions)
}
fn endorsed(transaction: &Transaction) -> payload::wire::Receipt {
    let offered = &transaction.offered.offer;
    let key = SigningKey::from_bytes(&[7; 32]);
    let root = key.verifying_key().to_bytes();
    let mut receipt = payload::wire::Receipt {
        body: offered.body.clone(),
        summary: offered.end.clone(),
        offer: Hex(offered.transcript(&root)),
        consumer: payload::wire::Consumer::Owner { key: Hex([9; 32]) },
        acknowledged: Hex([10; 32]),
        signature: Hex([0; 64]),
    };
    receipt.signature = Hex(key.sign(&receipt.transcript(&root)).to_bytes());
    receipt
}
struct Access {
    calls: usize,
    fail: Option<usize>,
}
impl Access {
    fn next(&mut self) -> Result<(), Refusal> {
        self.calls += 1;
        if self.fail == Some(self.calls) {
            Err(Refusal::Storage)
        } else {
            Ok(())
        }
    }
}
impl SectorStorage<BodyRegion> for Access {
    // This shared CPU adapter has only whole-bank erase. The incremental
    // writer's own sector/NOR cut suite owns physical sector-granularity proof.
    fn erase_sector(
        &mut self,
        storage: &mut BodyRegion,
        bank: u8,
        sector: u8,
    ) -> Result<(), Refusal> {
        self.next()?;
        if sector == 0 {
            storage.erase_bank(bank)?;
        }
        Ok(())
    }
    fn program(&mut self, storage: &mut BodyRegion, at: u32, bytes: &[u8]) -> Result<(), Refusal> {
        self.next()?;
        storage.program(at, bytes)
    }
    fn read(&mut self, storage: &mut BodyRegion, at: u32, bytes: &mut [u8]) -> Result<(), Refusal> {
        self.next()?;
        storage.read(at, bytes)
    }
}
fn drive(
    transaction: &mut Transaction,
    store: &mut Store,
    access: &mut Access,
) -> Result<(), &'static str> {
    loop {
        if transaction.writer.as_ref().unwrap().needs_chunk() {
            transaction.prepare_chunk(store)?;
        }
        let calls = access.calls;
        let done = transaction.advance(store, access)?;
        assert_eq!(
            access.calls,
            calls + 1,
            "one actual access per incremental step"
        );
        if done {
            return Ok(());
        }
    }
}
#[test]
fn parked_prefix_commit_restores_same_boot_exact_owner_and_original_admissions() {
    let (mut store, token, host, root, evidence, admissions) = fixture();
    let generation = token.generation;
    let summary = store.custody.as_ref().unwrap().summary.clone();
    let mut transaction = Transaction::prepare(&mut store, token).ok().unwrap();
    let receipt = endorsed(&transaction);
    transaction.begin(&mut store, receipt).unwrap();
    assert_eq!(store.journal.generation(), Some(generation));
    assert!(store.parked && !store.ready && store.evidence.is_none());
    assert!(transaction.finish(&mut store).is_err());
    drive(
        &mut transaction,
        &mut store,
        &mut Access {
            calls: 0,
            fail: None,
        },
    )
    .unwrap();
    assert_eq!(
        store.journal.generation(),
        Some(generation),
        "readback alone cannot update token/source"
    );
    let mut token = transaction.finish(&mut store).unwrap();
    assert_eq!(token.generation, generation + 1);
    assert!(store.parked && !store.ready && store.evidence.is_none());
    let custody = store.custody.as_ref().unwrap();
    assert!(custody.summary == summary);
    assert_eq!(custody.local_count, 0);
    assert!(custody.prefix.is_some());
    let owner = restore_store(&mut store, &mut token, &host.host_id, &host.boot_id, &root).unwrap();
    assert_eq!(owner.evidence(), &evidence);
    assert_eq!(serde_json::to_vec(&store.admissions).unwrap(), admissions);
    assert_eq!(store.journal.generation(), Some(generation + 1));
}
#[test]
fn deny_and_cancel_return_the_actual_original_owner_without_checkpoint() {
    for deny in 0..3 {
        let (mut store, token, host, root, evidence, admissions) = fixture();
        let generation = token.generation;
        let mut transaction = Transaction::prepare(&mut store, token).ok().unwrap();
        let mut receipt = endorsed(&transaction);
        match deny {
            0 => receipt.signature.0[0] ^= 1,
            1 => receipt.offer.0[0] ^= 1,
            _ => receipt.summary.count += 1,
        }
        assert!(transaction.begin(&mut store, receipt).is_err());
        let mut token = transaction.cancel(&mut store).unwrap();
        assert_eq!(token.generation, generation);
        let owner =
            restore_store(&mut store, &mut token, &host.host_id, &host.boot_id, &root).unwrap();
        assert_eq!(owner.evidence(), &evidence);
        assert_eq!(serde_json::to_vec(&store.admissions).unwrap(), admissions);
        assert_eq!(store.journal.generation(), Some(generation));
    }
}
#[test]
fn every_failed_step_keeps_the_original_parked_owner_restorable() {
    let (mut store, token, _, _, _, _) = fixture();
    let mut transaction = Transaction::prepare(&mut store, token).ok().unwrap();
    let receipt = endorsed(&transaction);
    transaction.begin(&mut store, receipt).unwrap();
    let mut all = Access {
        calls: 0,
        fail: None,
    };
    drive(&mut transaction, &mut store, &mut all).unwrap();
    for cut in 1..=all.calls {
        let (mut store, token, host, root, evidence, admissions) = fixture();
        let generation = token.generation;
        let mut transaction = Transaction::prepare(&mut store, token).ok().unwrap();
        let receipt = endorsed(&transaction);
        transaction.begin(&mut store, receipt).unwrap();
        assert!(drive(
            &mut transaction,
            &mut store,
            &mut Access {
                calls: 0,
                fail: Some(cut)
            }
        )
        .is_err());
        let mut token = transaction.cancel(&mut store).unwrap();
        assert_eq!(token.generation, generation);
        let owner =
            restore_store(&mut store, &mut token, &host.host_id, &host.boot_id, &root).unwrap();
        assert_eq!(owner.evidence(), &evidence);
        assert_eq!(serde_json::to_vec(&store.admissions).unwrap(), admissions);
        assert_eq!(store.journal.generation(), Some(generation));
    }
}

#[test]
fn changed_offer_cannot_rewrite_or_discard_the_original_owner() {
    let (mut store, token, host, root, evidence, _) = fixture();
    let generation = token.generation;
    let mut transaction = Transaction::prepare(&mut store, token).ok().unwrap();
    let receipt = endorsed(&transaction);
    transaction.offered.offer.source.0[0] ^= 1;
    assert!(transaction.begin(&mut store, receipt).is_err());
    assert!(transaction.writer.is_none());
    let mut token = transaction.cancel(&mut store).unwrap();
    let owner = restore_store(&mut store, &mut token, &host.host_id, &host.boot_id, &root).unwrap();
    assert_eq!(owner.evidence(), &evidence);
    assert_eq!(store.journal.generation(), Some(generation));
}

#[test]
fn cancellation_after_unadopted_readback_still_returns_original_source_owner() {
    let (mut store, token, host, root, evidence, _) = fixture();
    let generation = token.generation;
    let mut transaction = Transaction::prepare(&mut store, token).ok().unwrap();
    let receipt = endorsed(&transaction);
    transaction.begin(&mut store, receipt).unwrap();
    drive(
        &mut transaction,
        &mut store,
        &mut Access {
            calls: 0,
            fail: None,
        },
    )
    .unwrap();
    let mut token = transaction.cancel(&mut store).unwrap();
    assert_eq!(token.generation, generation);
    let owner = restore_store(&mut store, &mut token, &host.host_id, &host.boot_id, &root).unwrap();
    assert_eq!(owner.evidence(), &evidence);
    assert!(store.custody.as_ref().unwrap().local_count > 0);
    assert_eq!(store.journal.generation(), Some(generation));
}

#[test]
fn source_and_owner_scope_refusal_returns_the_unconsumed_token() {
    for fault in 0..5 {
        let (mut store, mut token, _, _, _, _) = fixture();
        let generation = token.generation;
        match fault {
            0 => token.generation += 1,
            1 => token.key[0] ^= 1,
            2 => token.binding[0] ^= 1,
            3 => store.ready = true,
            _ => store.parked = false,
        }
        let expected_key = token.key;
        let expected_binding = token.binding;
        let expected_generation = token.generation;
        let (returned, _) = Transaction::prepare(&mut store, token).err().unwrap();
        assert_eq!(returned.key, expected_key);
        assert_eq!(returned.binding, expected_binding);
        assert_eq!(returned.generation, expected_generation);
        assert!(returned.proposal.is_some());
        assert_eq!(store.journal.generation(), Some(generation));
    }
}

#[test]
fn bounded_export_chunks_are_authenticated_before_publication() {
    let (mut store, token, host, root, evidence, _) = fixture();
    let generation = token.generation;
    let mut transaction = Transaction::prepare(&mut store, token).ok().unwrap();
    let mut expected = Vec::new();
    export_store(&mut store, &transaction.offered, &mut |bytes| {
        expected.extend_from_slice(bytes);
        Ok(())
    })
    .unwrap();
    let mut actual = Vec::new();
    loop {
        let mut chunk = [0; 128];
        let n = transaction
            .export_chunk(&mut store, actual.len(), &mut chunk)
            .unwrap();
        if n == 0 {
            break;
        }
        actual.extend_from_slice(&chunk[..n]);
    }
    assert_eq!(actual, expected);
    let mut chunk = [42; 128];
    assert!(transaction
        .export_chunk(&mut store, expected.len() + 1, &mut chunk)
        .is_err());
    assert_eq!(chunk, [0; 128]);
    transaction.offered.offer.stream.0[0] ^= 1;
    assert!(transaction.export_chunk(&mut store, 0, &mut chunk).is_err());
    assert_eq!(chunk, [0; 128]);
    let mut token = transaction.cancel(&mut store).unwrap();
    let owner = restore_store(&mut store, &mut token, &host.host_id, &host.boot_id, &root).unwrap();
    assert_eq!(owner.evidence(), &evidence);
    assert_eq!(store.journal.generation(), Some(generation));
}
