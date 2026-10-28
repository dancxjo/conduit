//! Cold network custody recovery, before Journal::open and before local Play.
//! Actual authenticated Pending owns the region until Clean physical readback.
use super::*;
use ed25519_dalek::Signer;
const CAPACITY: u32 = 32768;
const SECTOR: u32 = 4096;
pub(crate) const WORK: usize = 512;
/// Narrow physical adapter. Erasure always requires the affine Pending permit;
/// implementing this trait alone does not expose an unguarded erase entrance.
pub(crate) trait Storage {
    fn identity(&self) -> [u8; 32];
    fn capacity(&self) -> u32;
    fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), &'static str>;
    fn erase(&mut self, permit: &RecoveryPermit, sector: u8) -> Result<(), &'static str>;
}
/// Not Clone/Deserialize. Minted only from actual authenticated cold Store custody.
pub(crate) struct RecoveryPermit {
    source: crate::native_body_checkpoint::SourceIdentity,
    binding: [u8; 32],
    root: [u8; 32],
    region: [u8; 32],
    pending: [u8; 32],
    receipt: [u8; 32],
    epoch: u64,
}
impl RecoveryPermit {
    pub(crate) fn permits(&self, region: [u8; 32], capacity: u32, relative: u32) -> bool {
        region == self.region
            && capacity == CAPACITY
            && relative.is_multiple_of(SECTOR)
            && relative < CAPACITY
    }
}
pub(crate) enum Opened<S> {
    Ready(S),
    Recovering(Recovery<S>),
}
pub(crate) struct Recovery<S> {
    storage: S,
    permit: RecoveryPermit,
    erases: u8,
    readback: u32,
    failed: bool,
}
/// Full blank readback, still owning the exact region and Pending permit.
/// No raw region or permit escapes before authenticated Clean readback.
pub(crate) struct Blank<S> {
    storage: S,
    permit: RecoveryPermit,
}
impl<S: Storage> Recovery<S> {
    /// One sector erase or one <=512B read. Interrupted erasure is resumed from
    /// durable Pending; rechecking the original digest would reject lawful cuts.
    pub(crate) fn next_step(&self) -> Option<Step> {
        if self.failed {
            None
        } else if self.erases < 8 {
            Some(Step::Erase(self.erases))
        } else if self.readback < CAPACITY {
            Some(Step::Read(self.readback))
        } else {
            None
        }
    }
    pub(crate) fn complete(&self) -> bool {
        !self.failed && self.erases == 8 && self.readback == CAPACITY
    }
    pub(crate) fn step_with(
        &mut self,
        work: &mut [u8; WORK],
        run: impl FnOnce(&mut S, &RecoveryPermit, Step, &mut [u8; WORK]) -> Result<(), &'static str>,
    ) -> Result<bool, &'static str> {
        let step = self.next_step().ok_or("network-recovery-phase")?;
        self.failed = true;
        if matches!(step, Step::Read(_)) {
            work.fill(0);
        }
        run(&mut self.storage, &self.permit, step, work)?;
        match step {
            Step::Erase(_) => self.erases += 1,
            Step::Read(_) => {
                if work.iter().any(|b| *b != 255) {
                    return Err("network-recovery-not-blank");
                }
                self.readback += WORK as u32;
            }
        }
        self.failed = false;
        Ok(self.complete())
    }
    pub(crate) fn step(&mut self, work: &mut [u8; WORK]) -> Result<bool, &'static str> {
        if self.complete() {
            return Ok(true);
        }
        self.step_with(work, |storage, permit, step, work| match step {
            Step::Erase(sector) => storage.erase(permit, sector),
            Step::Read(at) => storage.read(at, work),
        })
    }
    pub(crate) fn finish(self) -> Result<Blank<S>, Self> {
        if self.failed || self.erases != 8 || self.readback != CAPACITY {
            return Err(self);
        }
        Ok(Blank {
            storage: self.storage,
            permit: self.permit,
        })
    }
}
fn cold_key(store: &Store) -> Result<ed25519_dalek::SigningKey, &'static str> {
    if store.failed || store.ready || store.parked {
        return Err("network-recovery-requires-cold-custody");
    }
    let evidence = store.evidence.as_ref().ok_or("network-recovery-evidence")?;
    let key =
        ed25519_dalek::SigningKey::from_bytes(store.seed.as_ref().ok_or("network-recovery-key")?);
    let previous = store.custody.as_ref().ok_or("network-recovery-source")?;
    if previous.source.binding() != &store.binding
        || previous.source.binding()
            != &<[u8; 32]>::from(Sha256::digest(evidence.body_id.as_str().as_bytes()))
        || Some(previous.source.generation()) != store.journal.generation()
    {
        return Err("network-recovery-source");
    }
    Ok(key)
}
fn begin_store<S: Storage>(store: &Store, storage: S) -> Result<Opened<S>, (S, &'static str)> {
    let check = || -> Result<Option<RecoveryPermit>, &'static str> {
        if store.failed || store.ready || store.parked {
            return Err("network-recovery-requires-cold-custody");
        }
        if store.evidence.is_none()
            || (store.custody.is_none() && store.journal.generation().is_some())
        {
            return Err("network-recovery-cold-evidence");
        }
        if storage.capacity() != CAPACITY {
            return Err("network-recovery-geometry");
        }
        let Some(_) = store.custody.as_ref().and_then(|c| c.network.as_ref()) else {
            return Ok(None);
        };
        let key = cold_key(store)?;
        let evidence = store.evidence.as_ref().ok_or("network-recovery-evidence")?;
        mint_permit(store, &evidence.body_id, &key, &storage)
    };
    match check() {
        Ok(None) => Ok(Opened::Ready(storage)),
        Ok(Some(permit)) => Ok(Opened::Recovering(Recovery {
            storage,
            permit,
            erases: 0,
            readback: 0,
            failed: false,
        })),
        Err(e) => Err((storage, e)),
    }
}
#[derive(Clone, Copy)]
pub(crate) enum Step {
    Erase(u8),
    Read(u32),
}
fn mint_permit<S: Storage>(
    store: &Store,
    body: &conduit_body::BodyId,
    key: &ed25519_dalek::SigningKey,
    storage: &S,
) -> Result<Option<RecoveryPermit>, &'static str> {
    if storage.capacity() != CAPACITY {
        return Err("network-recovery-geometry");
    }
    let previous = store.custody.as_ref().ok_or("network-recovery-source")?;
    if previous.source.binding() != &store.binding
        || Some(previous.source.generation()) != store.journal.generation()
        || store.binding != <[u8; 32]>::from(Sha256::digest(body.as_str().as_bytes()))
    {
        return Err("network-recovery-source");
    }
    let cursor = previous
        .network
        .as_ref()
        .ok_or("network-recovery-pending")?;
    cursor.validate(body, &key.verifying_key())?;
    if cursor.bytes()[56..88] != storage.identity() {
        return Err("network-recovery-region");
    }
    if cursor.bytes()[8] == 2 {
        return Ok(None);
    }
    let epoch = u64::from_le_bytes(cursor.bytes()[16..24].try_into().unwrap());
    epoch.checked_add(1).ok_or("network-recovery-epoch")?;
    Ok(Some(RecoveryPermit {
        source: previous.source.identity(),
        binding: store.binding,
        root: key.verifying_key().to_bytes(),
        region: storage.identity(),
        pending: Sha256::digest(cursor.bytes()).into(),
        receipt: Sha256::digest(&cursor.bytes()[24..408]).into(),
        epoch,
    }))
}
pub(super) fn begin_live<S: Storage>(
    store: &Store,
    owner: &SealedBodyLifecycleSession,
    root: &[u8; 32],
    storage: S,
) -> Result<Recovery<S>, (S, &'static str)> {
    let checked = (|| {
        let key = super::network_pending::live_key(store, owner, root)?;
        mint_permit(store, &owner.evidence().body_id, &key, &storage)?
            .ok_or("network-live-requires-pending")
    })();
    match checked {
        Ok(permit) => Ok(Recovery {
            storage,
            permit,
            erases: 0,
            readback: 0,
            failed: false,
        }),
        Err(e) => Err((storage, e)),
    }
}
fn checked_blank<S: Storage>(
    store: &Store,
    body: &conduit_body::BodyId,
    key: &ed25519_dalek::SigningKey,
    blank: &Blank<S>,
) -> Result<(), &'static str> {
    let previous = store.custody.as_ref().ok_or("network-recovery-source")?;
    let pending = previous
        .network
        .as_ref()
        .ok_or("network-recovery-pending")?;
    pending.validate(body, &key.verifying_key())?;
    if pending.bytes()[8] != 1
        || previous.source.identity() != blank.permit.source
        || store.binding != blank.permit.binding
        || key.verifying_key().to_bytes() != blank.permit.root
        || <[u8; 32]>::from(Sha256::digest(pending.bytes())) != blank.permit.pending
        || blank.storage.identity() != blank.permit.region
        || blank.storage.capacity() != CAPACITY
    {
        return Err("network-recovery-stale-blank");
    }
    Ok(())
}
pub(super) fn check_live_blank<S: Storage>(
    store: &Store,
    owner: &SealedBodyLifecycleSession,
    root: &[u8; 32],
    blank: &Blank<S>,
) -> Result<(), &'static str> {
    let key = super::network_pending::live_key(store, owner, root)?;
    checked_blank(store, &owner.evidence().body_id, &key, blank).map(|_| ())
}
/// Affine proof that full blank readback and its exact Clean successor were
/// adopted. Only this token can reset the native journal's capacity metadata.
pub(crate) struct CleanBlank<S>(S);
impl<S> CleanBlank<S> {
    pub(crate) fn into_storage(self) -> S {
        self.0
    }
}
pub(super) fn finish_live<S: Storage>(
    store: &Store,
    owner: &SealedBodyLifecycleSession,
    root: &[u8; 32],
    blank: Blank<S>,
) -> Result<CleanBlank<S>, (Blank<S>, &'static str)> {
    let checked = (|| {
        let key = super::network_pending::live_key(store, owner, root)?;
        let current = store.custody.as_ref().ok_or("network-recovery-source")?;
        let cursor = current.network.as_ref().ok_or("network-recovery-pending")?;
        cursor.validate(&owner.evidence().body_id, &key.verifying_key())?;
        if cursor.bytes()[8] != 2
            || !current.source.succeeds(blank.permit.source)
            || Some(current.source.generation()) != store.journal.generation()
            || store.binding != blank.permit.binding
            || key.verifying_key().to_bytes() != blank.permit.root
            || blank.storage.identity() != blank.permit.region
            || blank.storage.capacity() != CAPACITY
            || cursor.bytes()[16..24]
                != blank
                    .permit
                    .epoch
                    .checked_add(1)
                    .ok_or("network-recovery-epoch")?
                    .to_le_bytes()
            || <[u8; 32]>::from(Sha256::digest(&cursor.bytes()[24..408])) != blank.permit.receipt
        {
            return Err("network-live-clean-scope");
        }
        Ok(())
    })();
    match checked {
        Ok(()) => Ok(CleanBlank(blank.storage)),
        Err(e) => Err((blank, e)),
    }
}
/// Called before the live network journal can append. Store borrowing fences
/// an in-flight checkpoint; failure fences an ambiguous committed successor.
/// Cold/parked layout alone is not refusal: retiring a lawful network control
/// can overlap ordinary preparation while the cached cursor remains Clean.
pub(crate) fn ensure_append_allowed() -> Result<(), &'static str> {
    with_store(append_allowed)
}
fn append_allowed(store: &mut Store) -> Result<(), &'static str> {
    if store.failed {
        return Err("network-evidence-restart-failed");
    }
    if let Some(custody) = store.custody.as_ref() {
        if custody.source.binding() != &store.binding
            || Some(custody.source.generation()) != store.journal.generation()
        {
            return Err("network-evidence-stale-checkpoint");
        }
        if custody
            .network
            .as_ref()
            .is_some_and(|cursor| cursor.bytes()[8] == 1)
        {
            return Err("network-evidence-pending-reclaim");
        }
    } else if store.journal.generation().is_some() {
        return Err("network-evidence-missing-custody");
    }
    Ok(())
}
/// Cold startup only. No region is returned while Pending survives. This is
/// finite bootstrap work: <=8 sector erases plus64 readback chunks, before any
/// sensor/radio/Mask Play. It adds no permanent async-task workspace.
#[inline(never)]
pub(crate) fn recover<S: Storage>(storage: S) -> Result<S, &'static str> {
    // Keep region ownership in this stack frame even when Store access refuses.
    let mut storage = Some(storage);
    let opened = with_store(|store| {
        begin_store(store, storage.take().unwrap()).map_err(|(region, e)| {
            storage = Some(region);
            e
        })
    })?;
    match opened {
        Opened::Ready(region) => Ok(region),
        Opened::Recovering(job) => recover_pending(job),
    }
}
#[inline(never)]
fn recover_pending<S: Storage>(mut job: Recovery<S>) -> Result<S, &'static str> {
    let mut work = [0; WORK];
    while !job.step(&mut work)? {}
    let blank = job.finish().map_err(|_| "network-recovery-incomplete")?;
    finish(blank).map_err(|(_blank, e)| e)
}
pub(crate) fn finish<S: Storage>(blank: Blank<S>) -> Result<S, (Blank<S>, &'static str)> {
    match with_store(|store| clean_store(store, &blank)) {
        Ok(()) => Ok(blank.storage),
        Err(e) => Err((blank, e)),
    }
}
fn clean_store<S: Storage>(store: &mut Store, blank: &Blank<S>) -> Result<(), &'static str> {
    let key = cold_key(store)?;
    let evidence = store.evidence.as_ref().ok_or("network-recovery-evidence")?;
    let previous = store.custody.as_ref().ok_or("network-recovery-source")?;
    checked_blank(store, &evidence.body_id, &key, blank)?;
    let pending = previous
        .network
        .as_ref()
        .ok_or("network-recovery-pending")?;
    // Allocate/sign/validate before any checkpoint effect. The original
    // member custody stays byte-for-byte intact; only signed phase/epoch change.
    let cursor = clean_cursor(pending, &key)?;
    let prepared = payload::rewrite::PreparedRewrite::prepare_clean_network(
        previous.proof.as_ref().ok_or("network-recovery-proof")?,
        &previous.source,
        evidence,
        store
            .admissions
            .as_ref()
            .ok_or("network-recovery-admissions")?,
        &previous.summary,
        previous.span,
        previous.prefix.as_ref(),
        previous.local_count,
        key.verifying_key().as_bytes(),
        &previous.layouts,
        pending,
        &cursor,
    )?;
    let next = super::network_transition::write_prepared(
        &mut store.journal,
        &store.binding,
        store.seed.as_ref().ok_or("network-recovery-key")?,
        previous,
        &prepared,
    )
    .inspect_err(|_| {
        store.failed = true;
    })?;
    drop(prepared);
    let current = store.custody.as_mut().ok_or("network-recovery-source")?;
    current.source = next.source;
    current.proof = Some(next.proof);
    current.summary = next.summary;
    current.span = next.span;
    current.network = Some(cursor);
    Ok(())
}
pub(super) fn clean_cursor(
    pending: &payload::network::Cursor,
    key: &ed25519_dalek::SigningKey,
) -> Result<payload::network::Cursor, &'static str> {
    let mut bytes = *pending.bytes();
    let epoch = u64::from_le_bytes(bytes[16..24].try_into().unwrap())
        .checked_add(1)
        .ok_or("network-recovery-epoch")?;
    bytes[8] = 2;
    bytes[16..24].copy_from_slice(&epoch.to_le_bytes());
    let mut hash = Sha256::new();
    hash.update(b"conduit.environment/network-retirement-receipt@1\0");
    hash.update(&bytes[..408]);
    bytes[408..472].copy_from_slice(&key.sign(&hash.finalize()).to_bytes());
    payload::network::Cursor::decode(bytes)
}
#[cfg(test)]
#[path = "native_restart_network_recovery_tests.rs"]
mod tests;
