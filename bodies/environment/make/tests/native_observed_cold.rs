//! Opt-in CPU orchestration of the actual native cold epoch service and Root
//! restart signer/checkpoint. Only physical media and archive global locks are
//! substituted; canonical admission and signed record/ACK checks stay real.
//!
//! Explicit entrance (the default suite intentionally selects zero tests):
//! ```text
//! cargo +stable rustc --locked --manifest-path bodies/environment/make/Cargo.toml --test native_observed_cold -- --cfg 'feature="observation-store"'
//! target/host-proof/debug/deps/native_observed_cold-<hash> actual_cold_service --nocapture
//! ```
//! Set CARGO_TARGET_DIR to the chosen existing cache for the command above. The
//! final-crate cfg enables the actual native Root observed module; no dependency
//! feature graph, target admission, SDK or physical-device claim is added.
//!
//! Archive/work locks below instrument synchronous ownership scopes, not actual
//! ESP interrupt state. Every physical mock I/O asserts outside those scopes.
#![cfg(feature = "observation-store")]
#![allow(dead_code, unused_imports, unexpected_cfgs)]
extern crate alloc;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_flash_jobs.rs"]
mod native_flash_jobs;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_flash_window.rs"]
mod native_flash_window;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_observed_reclaim_job.rs"]
mod native_observed_reclaim_job;

#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use ed25519_dalek::Signer;
use sha2::{Digest, Sha256};
use std::{
    cell::{Cell, RefCell},
    path::Path,
    sync::{Arc, Mutex as StdMutex},
    time::Duration,
};
thread_local! {static LOCK_DEPTH:Cell<u32>=const{Cell::new(0)};}
static ARM_COMMIT_READ_FAULT: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
static COMMIT_READ_FAULT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
fn outside_lock() {
    LOCK_DEPTH.with(|n| assert_eq!(n.get(), 0, "physical I/O under archive/work critical scope"));
}
struct Lock<T>(StdMutex<T>);
impl<T> Lock<T> {
    const fn new(value: T) -> Self {
        Self(StdMutex::new(value))
    }
    fn lock<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        let guard = self.0.lock().unwrap();
        LOCK_DEPTH.with(|n| n.set(n.get() + 1));
        struct End;
        impl Drop for End {
            fn drop(&mut self) {
                LOCK_DEPTH.with(|n| n.set(n.get() - 1));
            }
        }
        let _end = End;
        f(&guard)
    }
}
#[path = "../../../../targets/esp32/proof/tm1637/src/native_body_checkpoint.rs"]
mod native_body_checkpoint;
// Per-thread read faults affect only explicit receipt-proof minting tests.
std::thread_local! {
    static CHECKPOINT_DECODE_FAULT: std::cell::Cell<(u8,usize,usize)> = const { std::cell::Cell::new((0,0,0)) };
}
fn checkpoint_decode_fault(mode: u8) {
    CHECKPOINT_DECODE_FAULT.with(|f| f.set((mode, 0, 0)));
}
mod native_evidence_region {
    pub fn test_empty() -> BodyRegion {
        BodyRegion {
            media: std::sync::Arc::new(std::sync::Mutex::new((vec![255; 65536], true))),
        }
    }
    use super::*;
    pub struct BodyRegion {
        pub media: Arc<StdMutex<(Vec<u8>, bool)>>,
    }
    impl native_body_checkpoint::Storage for BodyRegion {
        fn capacity(&self) -> u32 {
            65536
        }
        fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), native_body_checkpoint::Refusal> {
            outside_lock();
            if COMMIT_READ_FAULT.load(std::sync::atomic::Ordering::SeqCst) {
                return Err(native_body_checkpoint::Refusal::Storage);
            }
            let m = self.media.lock().unwrap();
            out.copy_from_slice(
                m.0.get(at as usize..at as usize + out.len())
                    .ok_or(native_body_checkpoint::Refusal::Storage)?,
            );
            CHECKPOINT_DECODE_FAULT.with(|fault| {
                use sha2::Digest;
                let (mode, mut headers, mut payloads) = fault.get();
                if mode == 0 {
                    return;
                }
                if at.is_multiple_of(32768) && out.len() == 160 {
                    headers += 1;
                    payloads = 0;
                    if mode == 1 && headers == 2 {
                        // A completely authenticated second source with a
                        // different private seed must still fail the root pin.
                        out[56..88].fill(62);
                        let length = u32::from_le_bytes(out[16..20].try_into().unwrap()) as usize;
                        let mut hash = sha2::Sha256::new();
                        hash.update(&out[..120]);
                        hash.update(&m.0[at as usize + 160..at as usize + 160 + length]);
                        out[120..152].copy_from_slice(&hash.finalize());
                    }
                } else if mode == 2 && headers == 2 && at % 32768 >= 160 {
                    payloads += 1;
                    let base = (at / 32768 * 32768) as usize;
                    let length =
                        u32::from_le_bytes(m.0[base + 16..base + 20].try_into().unwrap()) as usize;
                    if payloads == length.div_ceil(128) + 1 && !out.is_empty() {
                        out[0] ^= 1;
                    }
                }
                fault.set((mode, headers, payloads));
            });
            Ok(())
        }
        fn program(
            &mut self,
            at: u32,
            bytes: &[u8],
        ) -> Result<(), native_body_checkpoint::Refusal> {
            outside_lock();
            let mut m = self.media.lock().unwrap();
            if !m.1 {
                return Err(native_body_checkpoint::Refusal::Storage);
            }
            for (a, b) in m.0[at as usize..at as usize + bytes.len()]
                .iter_mut()
                .zip(bytes)
            {
                assert_eq!(*a & b, *b);
                *a &= b;
            }
            if bytes == b"done"
                && ARM_COMMIT_READ_FAULT.swap(false, std::sync::atomic::Ordering::SeqCst)
            {
                COMMIT_READ_FAULT.store(true, std::sync::atomic::Ordering::SeqCst)
            }
            Ok(())
        }
        fn erase_bank(&mut self, bank: u8) -> Result<(), native_body_checkpoint::Refusal> {
            outside_lock();
            let mut m = self.media.lock().unwrap();
            if !m.1 {
                return Err(native_body_checkpoint::Refusal::Storage);
            }
            m.0[usize::from(bank) * 32768..(usize::from(bank) + 1) * 32768].fill(255);
            Ok(())
        }
    }
}
#[path = "../../../../targets/esp32/proof/tm1637/src/native_joined_host_wire.rs"]
mod joined;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_restart.rs"]
mod native_restart;
fn canonical_admission(
    directory: &Path,
) -> (
    ed25519_dalek::SigningKey,
    joined::JoinedHostReceipt,
    conduit_body::SpawnInvitationClaim,
) {
    use conduit_body::{AdmissionManager, SpawnAdmissionProof, SpawnInvitationSecret};
    use ed25519_dalek::Signer;
    use std::{io::Write, os::unix::fs::OpenOptionsExt};
    let key = ed25519_dalek::SigningKey::from_bytes(&[41; 32]);
    let root = conduit_core::HostAdvertisement {
        protocol_version: conduit_core::PROTOCOL_VERSION,
        host_id: "producer/root".into(),
        boot_id: "producer/boot-one".into(),
        offer_generation: conduit_core::OfferGeneration(1),
        profile: "fixture/root".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let sign = conduit_core::bind_sign(&root.host_id, &root.boot_id, None, 0);
    let image =
        conduit_environment_make::EnvironmentBodyImage::prepare_mask_from_sign(&sign).unwrap();
    let mut owner = conduit_environment_runtime::body_image::open_installed_image_sealed(
        image.body.evidence().clone(),
        &root,
    )
    .unwrap();
    let mut manager = AdmissionManager::new(owner.evidence().body_id.clone()).unwrap();
    let secret = SpawnInvitationSecret::from_csprng_bytes([22; 32]).unwrap();
    let claim = owner
        .issue_invitation(
            &mut manager,
            secret.clone(),
            [3; 32],
            10,
            999,
            &root.host_id,
            &root.boot_id,
        )
        .unwrap();
    let observer = conduit_core::HostAdvertisement {
        host_id: "observer/host".into(),
        boot_id: "observer/boot".into(),
        ..root.clone()
    };
    let proof = SpawnAdmissionProof {
        invitation_id: claim.invitation_id.clone(),
        body_id: claim.body_id.clone(),
        host_id: observer.host_id.clone(),
        boot_id: observer.boot_id.clone(),
        nonce: claim.nonce,
        signature: secret.sign(&claim.signing_transcript(
            &observer.host_id,
            &observer.boot_id,
            observer.offer_generation,
        )),
    };
    let credential = owner
        .admit_invited_host(
            &mut manager,
            &observer,
            &proof,
            11,
            &root.host_id,
            &root.boot_id,
        )
        .unwrap();
    assert!(owner
        .admit_invited_host(
            &mut manager,
            &observer,
            &proof,
            12,
            &root.host_id,
            &root.boot_id
        )
        .is_err());
    // This CPU owner writes its complete actual membership+replay manager before
    // signing admission. It is not the native flash checkpoint implementation.
    let checkpoint = directory.join("canonical-body-checkpoint.json");
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&checkpoint)
        .unwrap();
    serde_json::to_writer(&mut file, &(owner.evidence(), &manager)).unwrap();
    file.flush().unwrap();
    file.sync_all().unwrap();
    std::fs::File::open(directory).unwrap().sync_all().unwrap();
    let (evidence, recovered): (conduit_body::BodyBiographyEvidence, AdmissionManager) =
        serde_json::from_slice(&std::fs::read(checkpoint).unwrap()).unwrap();
    evidence.validate().unwrap();
    assert_eq!(evidence, *owner.evidence());
    assert_eq!(recovered, manager);
    recovered
        .verify_admitted_message(
            &evidence.membership,
            &credential.part_id,
            &observer.host_id,
            &observer.boot_id,
            &[42; 32],
            &secret.sign(&[42; 32]),
        )
        .unwrap();
    let mut receipt = joined::JoinedHostReceipt {
        schema: joined::PROFILE.into(),
        credential,
        membership_revision: evidence.membership.revision.0,
        producer_host: root.host_id,
        producer_boot: root.boot_id,
        producer_key: key.verifying_key().to_bytes(),
        invitation_id: claim.invitation_id.clone(),
        offer_generation: observer.offer_generation,
        signature: vec![],
    };
    receipt.signature = key
        .sign(&joined::transcript(&receipt.scope()))
        .to_bytes()
        .to_vec();
    (key, receipt, claim)
}

#[path = "native_observed_cold/archive.rs"]
mod native_observed_archive;
#[test]
fn actual_cold_service_keeps_admitted_durable_erase_authority_and_error_custody() {
    use std::os::unix::fs::DirBuilderExt;
    let directory =
        std::env::temp_dir().join(format!("conduit-actual-cold-{}", std::process::id()));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&directory)
        .unwrap();
    let (key, receipt, _) = canonical_admission(&directory);
    let (evidence, manager): (
        conduit_body::BodyBiographyEvidence,
        conduit_body::AdmissionManager,
    ) = serde_json::from_slice(
        &std::fs::read(directory.join("canonical-body-checkpoint.json")).unwrap(),
    )
    .unwrap();
    // Reproduce a real producer restart: canonical resume_here detaches the
    // observer's ephemeral presence while preserving its admitted relationship
    // and completed continuity credential. Persist this exact offline state.
    let recovered = conduit_body::SealedBodyLifecycleSession::resume_here(
        evidence,
        &receipt.producer_host,
        &"producer/boot-after-restart".into(),
    )
    .unwrap();
    let evidence = recovered.evidence().clone();
    let observer = evidence
        .membership
        .parts
        .iter()
        .find(|part| part.part_id == receipt.credential.part_id)
        .unwrap();
    assert!(observer.current.is_none());
    let transcript = [97; 32];
    let secret = conduit_body::SpawnInvitationSecret::from_csprng_bytes([22; 32]).unwrap();
    let signature = secret.sign(&transcript);
    assert!(manager
        .verify_admitted_message(
            &evidence.membership,
            &receipt.credential.part_id,
            &receipt.credential.host_id,
            &receipt.credential.boot_id,
            &transcript,
            &signature
        )
        .is_err());
    manager
        .verify_retained_custody_message(
            &evidence.membership,
            &receipt.credential,
            &transcript,
            &signature,
        )
        .unwrap();
    let media = Arc::new(StdMutex::new((vec![255; 65536], true)));
    let mut journal =
        native_body_checkpoint::Checkpoint::open(native_evidence_region::BodyRegion {
            media: media.clone(),
        })
        .unwrap();
    let binding = Sha256::digest(evidence.body_id.as_str().as_bytes()).into();
    let mut plain = Vec::new();
    native_restart::payload::stream_with_admissions(
        &evidence,
        &[],
        &[],
        Some(&manager),
        &mut |bytes| {
            plain.extend_from_slice(bytes);
            Ok::<_, ()>(())
        },
    )
    .unwrap();
    journal.save(&binding, &[41; 32], &plain).unwrap();
    drop(journal);
    native_restart::initialize(
        native_evidence_region::BodyRegion {
            media: media.clone(),
        },
        evidence.clone(),
    )
    .unwrap();
    native_observed_archive::prove(&evidence.body_id, &key, &receipt.credential, &media);
    std::fs::remove_dir_all(directory).unwrap();
}

#[path = "native_observed_cold/inspection.rs"]
mod inspection;
