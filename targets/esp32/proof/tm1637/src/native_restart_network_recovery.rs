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
    pub(crate) fn step(&mut self, work: &mut [u8; WORK]) -> Result<bool, &'static str> {
        if self.failed {
            return Err("network-recovery-failed");
        }
        self.failed = true;
        if self.erases < 8 {
            self.storage.erase(&self.permit, self.erases)?;
            self.erases += 1;
        } else if self.readback < CAPACITY {
            self.storage.read(self.readback, work)?;
            if work.iter().any(|byte| *byte != 255) {
                return Err("network-recovery-not-blank");
            }
            self.readback += WORK as u32;
        }
        self.failed = false;
        Ok(self.erases == 8 && self.readback == CAPACITY)
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
        let Some(cursor) = store.custody.as_ref().and_then(|c| c.network.as_ref()) else {
            return Ok(None);
        };
        let key = cold_key(store)?;
        let evidence = store.evidence.as_ref().ok_or("network-recovery-evidence")?;
        let previous = store.custody.as_ref().ok_or("network-recovery-source")?;
        cursor.validate(&evidence.body_id, &key.verifying_key())?;
        if cursor.bytes()[56..88] != storage.identity() {
            return Err("network-recovery-region");
        }
        if cursor.bytes()[8] == 2 {
            return Ok(None);
        }
        u64::from_le_bytes(cursor.bytes()[16..24].try_into().unwrap())
            .checked_add(1)
            .ok_or("network-recovery-epoch")?;
        Ok(Some(RecoveryPermit {
            source: previous.source.identity(),
            binding: store.binding,
            root: key.verifying_key().to_bytes(),
            region: storage.identity(),
            pending: Sha256::digest(cursor.bytes()).into(),
        }))
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
    let pending = previous
        .network
        .as_ref()
        .ok_or("network-recovery-pending")?;
    pending.validate(&evidence.body_id, &key.verifying_key())?;
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
fn clean_cursor(
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
