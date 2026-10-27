//! Exclusive Root restart custody. Initialize before selecting or publishing a
//! signing key. No corrupt/nonempty journal is treated as a fresh installation.
use crate::{
    native_body_checkpoint::{Checkpoint, Refusal},
    native_evidence_region::BodyRegion,
};
use alloc::{boxed::Box, vec::Vec};
use conduit_body::{
    AdmissionManager, BodyBiographyArchiveSegment, BodyBiographyEvidence,
    SealedBodyLifecycleSession,
};
use core::cell::RefCell;
use embassy_sync::blocking_mutex::{raw::CriticalSectionRawMutex, Mutex};
use sha2::{Digest, Sha256};
#[path = "native_restart_payload.rs"]
pub(crate) mod payload;
// Awake CPU-only custody: no DMA or deep-sleep persistence is claimed.
#[allow(unexpected_cfgs)] // Native profile is absent from the CPU proof crate.
#[cfg_attr(feature = "local-appliance", esp_hal::ram(unstable(rtc_slow)))]
static STORE: Mutex<CriticalSectionRawMutex, RefCell<Slot>> =
    Mutex::new(RefCell::new(Slot::Uninitialized));
// Inline static custody preserves the existing heap/stack reserve; boxing
// would introduce a new fallible startup allocation.
#[allow(clippy::large_enum_variant)]
enum Slot {
    Uninitialized,
    Available(Store),
    Borrowed,
}
// Move exclusive ownership under a short critical section. Serialization and
// flash I/O run after interrupts are restored. Readers fail closed while the
// Store is borrowed; no second initializer or transaction can enter.
struct StoreLease(Option<Store>);
impl Drop for StoreLease {
    fn drop(&mut self) {
        if let Some(store) = self.0.take() {
            STORE.lock(|slot| {
                let mut slot = slot.borrow_mut();
                debug_assert!(matches!(*slot, Slot::Borrowed));
                *slot = Slot::Available(store);
            });
        }
    }
}
fn with_store<T>(
    action: impl FnOnce(&mut Store) -> Result<T, &'static str>,
) -> Result<T, &'static str> {
    let store = STORE.lock(|slot| {
        let mut slot = slot.borrow_mut();
        match core::mem::replace(&mut *slot, Slot::Borrowed) {
            Slot::Available(store) => Ok(store),
            prior => {
                *slot = prior;
                Err("restart-not-ready")
            }
        }
    })?;
    let mut lease = StoreLease(Some(store));
    action(lease.0.as_mut().ok_or("restart-not-ready")?)
}
struct Store {
    journal: Checkpoint<BodyRegion>,
    binding: [u8; 32],
    seed: Option<[u8; 32]>,
    evidence: Option<BodyBiographyEvidence>,
    archives: Vec<BodyBiographyArchiveSegment>,
    admissions: Option<AdmissionManager>,
    parked: bool,
    pristine: bool,
    failed: bool,
    ready: bool,
}
pub struct RetainedBody {
    pub evidence: BodyBiographyEvidence,
    pub pristine: bool,
}
/// Cold, unpublished custody only. No Clone/Deserialize or caller construction.
pub struct ParkedBody {
    proposal: Option<conduit_body::VerifiedBodyProposalContext>,
    generation: u64,
    body_id: conduit_body::BodyId,
    foreground: Option<conduit_body::ResidentPlot>,
    binding: [u8; 32],
    key: [u8; 32],
    host: conduit_core::HostId,
    boot: conduit_core::BootId,
}
impl ParkedBody {
    pub fn prepare_verified_proposal(
        &mut self,
        plots: &[conduit_body::VerifiedBodyPlotPlan<'_>],
        masks: &[conduit_body::VerifiedBodyMaskTopology<'_>],
    ) -> Result<conduit_body::PreparedVerifiedBodyProposal, &'static str> {
        self.proposal
            .take()
            .ok_or("restart-proposal-already-prepared")?
            .prepare_verified(plots, masks, None)
            .map_err(|_| "restart-proposal")
    }
}
/// Commit complete custody before releasing any RAM. Every refusal leaves the
/// caller's actual owner installed; an executing/realized Body cannot be parked.
pub fn park_box(
    owner: &mut Option<Box<SealedBodyLifecycleSession>>,
    key: &[u8; 32],
    host: &conduit_core::HostId,
    boot: &conduit_core::BootId,
) -> Result<ParkedBody, &'static str> {
    with_store(|store| park_store(store, owner, key, host, boot))
}
fn park_store(
    store: &mut Store,
    owner: &mut Option<Box<SealedBodyLifecycleSession>>,
    key: &[u8; 32],
    host: &conduit_core::HostId,
    boot: &conduit_core::BootId,
) -> Result<ParkedBody, &'static str> {
    let body = owner.as_mut().ok_or("restart-body-missing")?;
    if store.parked
        || store.failed
        || !store.ready
        || body.evidence().body.state != conduit_body::BodyState::Lulled
        || body.realization().is_some()
    {
        return Err("restart-park-state");
    }
    // Validate the exact current admitted Root without cloning biography.
    body.evidence()
        .membership
        .parts
        .iter()
        .find(|part| {
            part.state == conduit_body::MembershipState::Admitted
                && part
                    .current
                    .as_ref()
                    .is_some_and(|current| &current.host_id == host && &current.boot_id == boot)
        })
        .ok_or("restart-park-scope")?;
    checkpoint_store(store, body, key)?;
    if !body.pending_archives().is_empty() {
        return Err("restart-park-pending");
    }
    let token = ParkedBody {
        proposal: Some(
            body.prepare_verified_proposal_context(host, boot)
                .map_err(|_| "restart-proposal-context")?,
        ),
        generation: store.journal.generation().ok_or("restart-generation")?,
        body_id: body.evidence().body_id.clone(),
        foreground: body.foreground().cloned(),
        binding: store.binding,
        key: *key,
        host: host.clone(),
        boot: boot.clone(),
    };
    store.parked = true;
    store.ready = false;
    store.evidence = None;
    store.archives = Vec::new();
    store.admissions = None;
    drop(owner.take());
    Ok(token)
}

struct PrivateBytes(Vec<u8>);
impl Drop for PrivateBytes {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}
struct PrivateSeed([u8; 32]);
impl Drop for PrivateSeed {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}
/// Reopen the already admitted SAME Boot; no interrupted-Wake reconciliation,
/// membership mutation, challenge retirement, new key or journal write occurs.
pub fn restore_parked(
    token: ParkedBody,
    host: &conduit_core::HostId,
    boot: &conduit_core::BootId,
    key: &[u8; 32],
) -> Result<Box<SealedBodyLifecycleSession>, &'static str> {
    with_store(|store| restore_store(store, token, host, boot, key))
}
fn restore_store(
    store: &mut Store,
    token: ParkedBody,
    host: &conduit_core::HostId,
    boot: &conduit_core::BootId,
    key: &[u8; 32],
) -> Result<Box<SealedBodyLifecycleSession>, &'static str> {
    if !store.parked
        || store.failed
        || store.ready
        || token.generation != store.journal.generation().ok_or("restart-generation")?
        || token.binding != store.binding
        || &token.key != key
        || &token.host != host
        || &token.boot != boot
    {
        return Err("restart-restore-scope");
    }
    let length = store
        .journal
        .payload_length(&store.binding)
        .map_err(|_| "restart-restore-binding")?
        .ok_or("restart-missing")?;
    #[cfg(all(target_os = "none", feature = "local-appliance"))]
    crate::native_root::memory_check::mark("cold-body-restore-payload", length);
    let mut bytes = PrivateBytes(Vec::new());
    bytes
        .0
        .try_reserve_exact(length)
        .map_err(|_| "restart-memory")?;
    bytes.0.resize(length, 0);
    let mut seed = PrivateSeed([0; 32]);
    let loaded = store
        .journal
        .load(&store.binding, &mut seed.0, &mut bytes.0)
        .map_err(|_| "restart-restore-load")?
        .ok_or("restart-missing")?;
    if loaded != length
        || store.seed.as_ref() != Some(&seed.0)
        || ed25519_dalek::SigningKey::from_bytes(&seed.0)
            .verifying_key()
            .as_bytes()
            != key
    {
        return Err("restart-restore-key");
    }
    let retained = payload::decode(&bytes.0, &token.body_id)?;
    // The authenticated decoder owns every field. Retire private JSON and the
    // duplicate signing seed before canonical opening/Box allocation peaks.
    drop(bytes);
    drop(seed);
    #[cfg(all(target_os = "none", feature = "local-appliance"))]
    crate::native_root::memory_check::mark("cold-body-restore-decoded", length);
    if <[u8; 32]>::from(Sha256::digest(
        retained.evidence.body_id.as_str().as_bytes(),
    )) != store.binding
    {
        return Err("restart-restore-binding");
    }
    let admissions = retained.admissions.ok_or("restart-admissions-missing")?;
    let mut body = SealedBodyLifecycleSession::open_admitted(retained.evidence, host, boot)
        .map_err(|_| "restart-restore-admitted")?;
    if let Some(plot) = token.foreground.as_ref() {
        body.select_plot(plot)
            .map_err(|_| "restart-restore-foreground")?;
    }
    if body.evidence().body.state != conduit_body::BodyState::Lulled || body.realization().is_some()
    {
        return Err("restart-restore-state");
    }
    let body = Box::new(body);
    store.archives = retained.archives;
    store.admissions = Some(admissions);
    store.parked = false;
    store.ready = true;
    Ok(body)
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
        if !matches!(*slot, Slot::Uninitialized) {
            return Err("restart-already-owned");
        }
        *slot = Slot::Available(Store {
            journal,
            binding,
            seed,
            evidence: Some(evidence),
            archives,
            admissions: Some(admissions),
            parked: false,
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
    let result = with_store(|store| {
        if store.failed || store.parked {
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
                    Some(store.admissions.as_ref().ok_or(Refusal::Corrupt)?),
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
    with_store(|store| {
        if store.failed || store.parked || store.seed.is_none() {
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
    with_store(|store| checkpoint_store(store, owner, verifying_key))
}
fn checkpoint_store(
    store: &mut Store,
    owner: &mut SealedBodyLifecycleSession,
    verifying_key: &[u8; 32],
) -> Result<(), &'static str> {
    store.ready = false;
    if store.failed || store.parked {
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
                Some(store.admissions.as_ref().ok_or(Refusal::Corrupt)?),
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
    with_store(|store| {
        if store.failed || store.parked || !store.ready {
            return Err("restart-not-ready");
        }
        read(
            store
                .admissions
                .as_ref()
                .ok_or("restart-admissions-missing")?,
        )
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
    with_store(|store| {
        if store.failed || store.parked || !store.ready {
            return Err("restart-not-ready");
        }
        let result = mutate(
            owner,
            store
                .admissions
                .as_mut()
                .ok_or("restart-admissions-missing")?,
        )?;
        checkpoint_store(store, owner, verifying_key)?;
        Ok(result)
    })
}

pub fn publication_ready() -> bool {
    STORE.lock(
        |slot| matches!(&*slot.borrow(), Slot::Available(store) if store.ready && !store.failed && !store.parked),
    )
}
pub fn generation() -> Option<u64> {
    STORE.lock(|slot| match &*slot.borrow() {
        Slot::Available(store) => store.journal.generation(),
        _ => None,
    })
}

fn stream_refusal(error: payload::StreamRefusal<Refusal>) -> Refusal {
    match error {
        payload::StreamRefusal::Sink(error) => error,
        payload::StreamRefusal::Invalid => Refusal::Corrupt,
    }
}
#[cfg(test)]
#[path = "native_restart_park_tests.rs"]
mod park_tests;
