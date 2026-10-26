//! Exclusive Root restart custody. Initialize before selecting or publishing a
//! signing key. No corrupt/nonempty journal is treated as a fresh installation.
use crate::{
    native_body_checkpoint::{Checkpoint, Refusal},
    native_evidence_region::BodyRegion,
};
use alloc::vec::Vec;
use conduit_body::{
    AdmissionManager, BodyBiographyArchiveSegment, BodyBiographyEvidence,
    SealedBodyLifecycleSession,
};
use core::cell::RefCell;
use embassy_sync::blocking_mutex::{raw::CriticalSectionRawMutex, Mutex};
use sha2::{Digest, Sha256};
#[path = "native_restart_payload.rs"]
mod payload;
static STORE: Mutex<CriticalSectionRawMutex, RefCell<Option<Store>>> =
    Mutex::new(RefCell::new(None));
struct Store {
    journal: Checkpoint<BodyRegion>,
    binding: [u8; 32],
    seed: Option<[u8; 32]>,
    evidence: Option<BodyBiographyEvidence>,
    archives: Vec<BodyBiographyArchiveSegment>,
    admissions: AdmissionManager,
    pristine: bool,
    failed: bool,
    ready: bool,
}
pub struct RetainedBody {
    pub evidence: BodyBiographyEvidence,
    pub pristine: bool,
}
pub fn initialize(region: BodyRegion, compiled: BodyBiographyEvidence) -> Result<(), &'static str> {
    compiled
        .validate()
        .map_err(|_| "restart-compiled-biography")?;
    let binding: [u8; 32] = Sha256::digest(compiled.body_id.as_str().as_bytes()).into();
    let mut journal = Checkpoint::open(region).map_err(|_| "restart-journal")?;
    let (evidence, archives, seed, pristine, admissions) = match journal
        .payload_length(&binding)
        .map_err(|_| "restart-body-binding")?
    {
        None => {
            let admissions = AdmissionManager::new(compiled.body_id.clone())
                .map_err(|_| "restart-admissions")?;
            (compiled, Vec::new(), None, true, admissions)
        }
        Some(length) => {
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(length)
                .map_err(|_| "restart-memory")?;
            bytes.resize(length, 0);
            let mut seed = [0; 32];
            let n = journal
                .load(&binding, &mut seed, &mut bytes)
                .map_err(|_| "restart-load")?
                .ok_or("restart-missing")?;
            if n != length {
                seed.fill(0);
                return Err("restart-length");
            }
            let retained = match payload::decode(&bytes, &compiled.body_id) {
                Ok(value) => value,
                Err(error) => {
                    seed.fill(0);
                    return Err(error);
                }
            };
            let pristine = retained.evidence == compiled && retained.archives.is_empty();
            let mut admissions = match retained.admissions {
                Some(manager) => manager,
                None if retained.evidence.membership.parts.len() <= 1 => {
                    AdmissionManager::new(compiled.body_id.clone())
                        .map_err(|_| "restart-admissions")?
                }
                None => return Err("restart-missing-admissions"),
            };
            admissions.retire_unfinished_challenges();
            (
                retained.evidence,
                retained.archives,
                Some(seed),
                pristine,
                admissions,
            )
        }
    };
    STORE.lock(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_some() {
            return Err("restart-already-owned");
        }
        *slot = Some(Store {
            journal,
            binding,
            seed,
            evidence: Some(evidence),
            archives,
            admissions,
            pristine,
            failed: false,
            ready: false,
        });
        Ok(())
    })
}
/// New entropy is used only when the whole journal is verified erased. Persist
/// the exact initial biography and key before returning a publishable signer.
pub fn select_signer_seed(mut entropy: [u8; 32]) -> Result<[u8; 32], &'static str> {
    let result = STORE.lock(|slot| {
        let mut slot = slot.borrow_mut();
        let store = slot.as_mut().ok_or("restart-not-initialized")?;
        if store.failed {
            return Err("restart-failed");
        }
        if let Some(seed) = store.seed {
            return Ok(seed);
        }
        if entropy == [0; 32] {
            return Err("restart-entropy");
        }
        let evidence = store.evidence.as_ref().ok_or("restart-body-missing")?;
        store
            .journal
            .save_streamed(&store.binding, &entropy, |sink| {
                payload::stream_with_admissions(
                    evidence,
                    &store.archives,
                    &[],
                    Some(&store.admissions),
                    sink,
                )
                .map_err(stream_refusal)
            })
            .map_err(|_| {
                store.failed = true;
                "restart-initial-commit"
            })?;
        store.seed = Some(entropy);
        Ok(entropy)
    });
    entropy.fill(0);
    result
}
pub fn take_body() -> Result<RetainedBody, &'static str> {
    STORE.lock(|slot| {
        let mut slot = slot.borrow_mut();
        let store = slot.as_mut().ok_or("restart-not-initialized")?;
        if store.failed || store.seed.is_none() {
            return Err("restart-not-ready");
        }
        Ok(RetainedBody {
            evidence: store.evidence.take().ok_or("restart-body-already-owned")?,
            pristine: store.pristine,
        })
    })
}
/// Persist all mutable evidence and every archive before acknowledging custody.
/// Call after each proposal/start/recovery/membership mutation and before any
/// corresponding effect release or externally visible acknowledgement.
pub fn checkpoint(
    owner: &mut SealedBodyLifecycleSession,
    verifying_key: &[u8; 32],
) -> Result<(), &'static str> {
    STORE.lock(|slot| {
        let mut slot = slot.borrow_mut();
        let store = slot.as_mut().ok_or("restart-not-initialized")?;
        checkpoint_store(store, owner, verifying_key)
    })
}
fn checkpoint_store(
    store: &mut Store,
    owner: &mut SealedBodyLifecycleSession,
    verifying_key: &[u8; 32],
) -> Result<(), &'static str> {
    store.ready = false;
    if store.failed {
        return Err("restart-failed");
    }
    let seed = store.seed.as_ref().ok_or("restart-key-missing")?;
    if ed25519_dalek::SigningKey::from_bytes(seed)
        .verifying_key()
        .as_bytes()
        != verifying_key
    {
        return Err("restart-signer-mismatch");
    }
    let body_binding: [u8; 32] =
        Sha256::digest(owner.evidence().body_id.as_str().as_bytes()).into();
    if body_binding != store.binding {
        return Err("restart-body-binding");
    }
    store
        .journal
        .save_streamed(&store.binding, seed, |sink| {
            payload::stream_with_admissions(
                owner.evidence(),
                &store.archives,
                owner.pending_archives(),
                Some(&store.admissions),
                sink,
            )
            .map_err(stream_refusal)
        })
        .map_err(|_| {
            store.failed = true;
            "restart-checkpoint-commit"
        })?;
    // save hashes physical readback and commits the marker last. Only those
    // exact, validated archive bytes can now discharge the owner's pending head.
    let head = owner
        .pending_archives()
        .last()
        .map(|segment| segment.digest);
    store.archives.extend_from_slice(owner.pending_archives());
    if let Some(head) = head {
        owner.acknowledge_archives(head).map_err(|_| {
            store.failed = true;
            "restart-archive-ack"
        })?;
    }
    store.ready = true;
    Ok(())
}
/// Read retained canonical custody without exporting its private keys.
pub(crate) fn with_admissions<T>(
    read: impl FnOnce(&AdmissionManager) -> Result<T, &'static str>,
) -> Result<T, &'static str> {
    STORE.lock(|slot| {
        let slot = slot.borrow();
        let store = slot.as_ref().ok_or("restart-not-initialized")?;
        if store.failed || !store.ready {
            return Err("restart-not-ready");
        }
        read(&store.admissions)
    })
}
/// Root-only transaction: no result leaves before complete durable readback.
pub(crate) fn admissions_transaction<T>(
    owner: &mut SealedBodyLifecycleSession,
    verifying_key: &[u8; 32],
    mutate: impl FnOnce(
        &mut SealedBodyLifecycleSession,
        &mut AdmissionManager,
    ) -> Result<T, &'static str>,
) -> Result<T, &'static str> {
    STORE.lock(|slot| {
        let mut slot = slot.borrow_mut();
        let store = slot.as_mut().ok_or("restart-not-initialized")?;
        if store.failed || !store.ready {
            return Err("restart-not-ready");
        }
        let result = mutate(owner, &mut store.admissions)?;
        checkpoint_store(store, owner, verifying_key)?;
        Ok(result)
    })
}

pub fn publication_ready() -> bool {
    STORE.lock(|slot| {
        slot.borrow()
            .as_ref()
            .is_some_and(|store| store.ready && !store.failed)
    })
}
pub fn generation() -> Option<u64> {
    STORE.lock(|slot| {
        slot.borrow()
            .as_ref()
            .and_then(|store| store.journal.generation())
    })
}

fn stream_refusal(error: payload::StreamRefusal<Refusal>) -> Refusal {
    match error {
        payload::StreamRefusal::Sink(error) => error,
        payload::StreamRefusal::Invalid => Refusal::Corrupt,
    }
}
