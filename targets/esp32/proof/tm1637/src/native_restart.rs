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
#[path = "native_restart_restore.rs"]
mod restore;
#[cfg_attr(
    test,
    allow(
        unused_imports,
        reason = "CPU Store proofs use local custody instead of the global entrance"
    )
)]
pub use restore::{restore_parked, restore_parked_retained};
#[cfg(test)]
use restore::{restore_store, restore_store_retained};
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
    custody: Option<ArchiveCustody>,
    admissions: Option<AdmissionManager>,
    parked: bool,
    pristine: bool,
    failed: bool,
    ready: bool,
}
struct ArchiveCustody {
    source: crate::native_body_checkpoint::SourceSnapshot,
    summary: payload::archive::Summary,
    span: payload::archive::Span,
}
pub struct RetainedBody {
    pub evidence: BodyBiographyEvidence,
    pub pristine: bool,
}
/// Cold, unpublished custody only. No Clone/Deserialize or caller construction.
pub struct ParkedBody {
    #[cfg(not(target_os = "none"))]
    workspace: Option<PrivateBytes>,
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
    pub fn prepare_verified_plot(
        &self,
        plot: &conduit_body::VerifiedBodyPlotPlan<'_>,
    ) -> Result<conduit_body::PreparedVerifiedBodyPlotPlan, &'static str> {
        self.proposal
            .as_ref()
            .ok_or("restart-proposal-already-prepared")?
            .prepare_plot(plot)
            .map_err(|_| "restart-plot-proposal")
    }
    /// Reserve the actual complete checkpoint input before later cold owners
    /// fragment the heap. It stays private, bounded by the committed length,
    /// and is consumed directly by restore rather than copied or reallocated.
    #[cfg(not(target_os = "none"))]
    #[inline(never)]
    pub fn reserve_restore_workspace(&mut self) -> Result<usize, &'static str> {
        let length = with_store(|store| {
            if !store.parked
                || store.failed
                || store.ready
                || self.generation != store.journal.generation().ok_or("restart-generation")?
                || self.binding != store.binding
                || ed25519_dalek::SigningKey::from_bytes(
                    store.seed.as_ref().ok_or("restart-key-missing")?,
                )
                .verifying_key()
                .as_bytes()
                    != &self.key
            {
                return Err("restart-workspace-scope");
            }
            store
                .journal
                .payload_length(&store.binding)
                .map_err(|_| "restart-workspace-binding")?
                .ok_or("restart-missing")
        })?;
        if let Some(bytes) = self.workspace.as_ref() {
            return if bytes.0.len() == length {
                Ok(length)
            } else {
                Err("restart-workspace-length")
            };
        }
        let mut bytes = PrivateBytes(Vec::new());
        bytes
            .0
            .try_reserve_exact(length)
            .map_err(|_| "restart-memory")?;
        bytes.0.resize(length, 0);
        self.workspace = Some(bytes);
        Ok(length)
    }
    pub fn prepare_verified_proposal(
        &mut self,
        plots: &[conduit_body::VerifiedBodyPlotPlan<'_>],
        masks: &[conduit_body::VerifiedBodyMaskTopology<'_>],
    ) -> Result<conduit_body::PreparedVerifiedBodyProposal, &'static str> {
        self.prepare_verified_proposal_with_prepared(&[], plots, masks)
    }
    pub fn prepare_verified_proposal_with_prepared(
        &mut self,
        prepared: &[conduit_body::PreparedVerifiedBodyPlotPlan],
        plots: &[conduit_body::VerifiedBodyPlotPlan<'_>],
        masks: &[conduit_body::VerifiedBodyMaskTopology<'_>],
    ) -> Result<conduit_body::PreparedVerifiedBodyProposal, &'static str> {
        let context = self
            .proposal
            .take()
            .ok_or("restart-proposal-already-prepared")?;
        match context.prepare_verified_with_prepared_retained(prepared, plots, masks, None) {
            Ok(proposal) => Ok(proposal),
            Err((context, _)) => {
                self.proposal = Some(context);
                Err("restart-proposal")
            }
        }
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
        #[cfg(not(target_os = "none"))]
        workspace: None,
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
    // Archive bytes remain durably owned by the affine source snapshot.
    store.archives = Vec::new();
    store.admissions = None;
    drop(owner.take());
    Ok(token)
}

#[cfg(not(target_os = "none"))]
struct PrivateBytes(Vec<u8>);
#[cfg(not(target_os = "none"))]
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
#[inline(never)]
fn decode_checkpoint(
    journal: &mut Checkpoint<BodyRegion>,
    binding: &[u8; 32],
    body_id: &conduit_body::BodyId,
    seed: &mut [u8; 32],
    custody: Option<&ArchiveCustody>,
) -> Result<
    (
        payload::archive::Resident,
        crate::native_body_checkpoint::SourceSnapshot,
    ),
    &'static str,
> {
    seed.fill(0);
    // Authenticate and count the complete plaintext before allocating a DTO.
    // Compression increases durable capacity, never the available execution RAM.
    let generation = journal.generation();
    if let Some(custody) = custody {
        if custody.source.generation() != generation.ok_or("restart-generation")?
            || custody.source.binding() != binding
            || journal
                .payload_length(binding)
                .map_err(|_| "restart-custody-scope")?
                != Some(custody.source.payload_length())
        {
            return Err("restart-custody-scope");
        }
    }
    let mut private_seed = PrivateSeed([0; 32]);
    let plain = journal
        .load_streamed(binding, &mut private_seed.0, |reader| {
            payload::resident_extent(|chunk| reader.read(chunk).map_err(|_| ()))
                .map_err(|_| Refusal::Corrupt)
        })
        .map_err(|_| "restart-preflight")?
        .ok_or("restart-missing")?;
    private_seed.0.fill(0);
    let plain = if let Some(custody) = custody {
        if plain.span != custody.span || plain.segments != custody.summary.count {
            return Err("restart-custody-span");
        }
        plain.active
    } else {
        plain
            .active
            .checked_add(plain.largest)
            .ok_or("restart-capacity")?
    };
    #[cfg(target_os = "none")]
    {
        let memory = esp_alloc::HEAP.stats();
        let available = memory.size.saturating_sub(memory.current_usage);
        // Measured complete biographies retain about 1.25 times their JSON
        // extent. Keep another 4 KiB for validation and subsequent custody.
        let needed = plain.saturating_mul(5).div_ceil(4).saturating_add(4096);
        if available < needed {
            return Err("restart-memory-capacity");
        }
    }
    #[cfg(not(target_os = "none"))]
    let _ = plain;
    if journal.generation() != generation {
        return Err("restart-generation");
    }
    let mut refusal = None;
    journal
        .load_streamed_with_snapshot(binding, seed, |reader| {
            payload::decode_resident(
                |chunk| reader.read(chunk).map_err(|_| ()),
                body_id,
                custody.map(|c| &c.summary),
            )
            .and_then(|resident| {
                if custody.is_some_and(|c| c.span != resident.span) {
                    return Err("restart-custody-span");
                }
                Ok(resident)
            })
            .map_err(|error| {
                refusal = Some(error);
                Refusal::Corrupt
            })
        })
        .map_err(|_| refusal.unwrap_or("restart-load"))?
        .ok_or("restart-missing")
}
pub fn initialize(region: BodyRegion, compiled: BodyBiographyEvidence) -> Result<(), &'static str> {
    compiled
        .validate()
        .map_err(|_| "restart-compiled-biography")?;
    let binding: [u8; 32] = Sha256::digest(compiled.body_id.as_str().as_bytes()).into();
    let mut journal = Checkpoint::open(region).map_err(|_| "restart-journal")?;
    let (evidence, archives, custody, seed, pristine, admissions) = match journal
        .payload_length(&binding)
        .map_err(|_| "restart-body-binding")?
    {
        None => {
            let admissions = AdmissionManager::new(compiled.body_id.clone())
                .map_err(|_| "restart-admissions")?;
            (compiled, Vec::new(), None, None, true, admissions)
        }
        Some(_length) => {
            let mut seed = PrivateSeed([0; 32]);
            let (resident, source) =
                decode_checkpoint(&mut journal, &binding, &compiled.body_id, &mut seed.0, None)?;
            let retained = resident.retained;
            let pristine = retained.evidence == compiled && resident.summary.count == 0;
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
                Vec::new(),
                Some(ArchiveCustody {
                    source,
                    summary: resident.summary,
                    span: resident.span,
                }),
                Some(seed.0),
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
            custody,
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
                payload::stream_compressed_with_admissions(
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
        let mut checked_seed = PrivateSeed([0; 32]);
        let body_id = evidence.body_id.clone();
        let (resident, source) = decode_checkpoint(
            &mut store.journal,
            &store.binding,
            &body_id,
            &mut checked_seed.0,
            None,
        )?;
        if checked_seed.0 != entropy {
            return Err("restart-initial-key");
        }
        store.custody = Some(ArchiveCustody {
            source,
            summary: resident.summary,
            span: resident.span,
        });
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
    let next_custody = if let Some(previous) = store.custody.as_ref() {
        let mut metadata = None;
        let source = store
            .journal
            .rewrite_streamed(&previous.source, &store.binding, seed, |io| {
                let next = payload::stream_rewrite(
                    owner.evidence(),
                    owner.pending_archives(),
                    store.admissions.as_ref().ok_or(Refusal::Corrupt)?,
                    &previous.summary,
                    previous.span,
                    |out| io.read_source(out).map_err(|_| ()),
                    &mut |bytes| io.write(bytes),
                )
                .map_err(stream_refusal)?;
                if metadata.as_ref().is_some_and(|prior| prior != &next) {
                    return Err(Refusal::Corrupt);
                }
                metadata = Some(next);
                Ok(())
            })
            .map_err(|_| {
                store.failed = true;
                "restart-checkpoint-commit"
            })?;
        let (summary, span) = metadata.ok_or("restart-checkpoint-metadata")?;
        ArchiveCustody {
            source,
            summary,
            span,
        }
    } else {
        // Only a genuinely new journal (or the CPU storage fixture) starts
        // without a previously validated source. Existing media never falls
        // back to an empty archive chain.
        if store.journal.generation().is_some() {
            return Err("restart-custody-missing");
        }
        store
            .journal
            .save_streamed(&store.binding, seed, |sink| {
                payload::stream_compressed_with_admissions(
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
        let mut checked_seed = PrivateSeed([0; 32]);
        let (resident, source) = decode_checkpoint(
            &mut store.journal,
            &store.binding,
            &owner.evidence().body_id,
            &mut checked_seed.0,
            None,
        )?;
        if checked_seed.0 != *seed {
            return Err("restart-checkpoint-key");
        }
        ArchiveCustody {
            source,
            summary: resident.summary,
            span: resident.span,
        }
    };
    store.custody = Some(next_custody);
    // save hashes physical readback and commits the marker last. Only those
    // exact, validated archive bytes can now discharge the owner's pending head.
    let head = owner
        .pending_archives()
        .last()
        .map(|segment| segment.digest);
    // Complete old and newly sealed archives now reside in the authenticated
    // committed source. No full archive Vec remains resident after ACK.
    store.archives = Vec::new();
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
        // Mutation is now private pending custody; a staging failure must fence
        // publication just like a failed checkpoint, rather than expose it.
        store.ready = false;
        // Membership replay suffixes may move while the exact local Play stays
        // current. Archive custody remains pending until complete durable ACK.
        if owner
            .realization()
            .and_then(|current| current.play.as_ref())
            .is_some()
        {
            owner
                .seal_started_membership_history_for_checkpoint()
                .map_err(|_| "restart-active-membership-seal")?;
        }
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
