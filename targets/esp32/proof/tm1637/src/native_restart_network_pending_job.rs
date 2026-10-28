//! Incremental live Pending transition. Owns the actual Store lease throughout;
//! production wrapper owns the frozen Draft until finish or failure. No HTTP or erase authority.
use super::*;
use crate::native_body_checkpoint::{
    incremental::{IncrementalRewrite, Operation, SectorStorage},
    PayloadReader,
};
use alloc::boxed::Box;
use network_pending::{live_key, prepare, CommittedCustody};
#[cfg(all(
    feature = "local-appliance",
    feature = "body-persistence",
    target_os = "none"
))]
pub(crate) enum Start {
    Committed(CommittedCustody),
    Writing(Box<PendingJob>),
}
struct CheckpointJob {
    lease: StoreLease,
    cursor: Option<payload::network::Cursor>,
    writer: Option<IncrementalRewrite>,
    summary: payload::archive::Summary,
    span: payload::archive::Span,
    root: [u8; 32],
    effects: bool,
    adopted: bool,
    poisoned: bool,
}
/// Fallible exact allocation, before acquiring any actual Store or snapshot.
fn reserve_with<T>(
    allocate: impl FnOnce(core::alloc::Layout) -> *mut u8,
) -> Result<Box<core::mem::MaybeUninit<T>>, &'static str> {
    let raw = allocate(core::alloc::Layout::new::<T>()).cast::<core::mem::MaybeUninit<T>>();
    if raw.is_null() {
        return Err("network-pending-job-capacity");
    }
    // SAFETY: allocator supplied the exact nonzero PendingJob layout; unique
    // ownership is initialized only by Box::write after complete preparation.
    Ok(unsafe { Box::from_raw(raw) })
}
#[cfg(all(
    feature = "local-appliance",
    feature = "body-persistence",
    target_os = "none"
))]
pub(crate) fn start(
    owner: &SealedBodyLifecycleSession,
    boot: &conduit_core::BootId,
    root: &[u8; 32],
    draft: crate::native_play::journal::snapshot::Draft,
    request: &crate::native_play::journal::snapshot::ack::Request<'_>,
) -> Result<Start, (crate::native_play::journal::snapshot::Draft, &'static str)> {
    enum Prepared {
        Committed(CommittedCustody),
        Writing(Box<core::mem::MaybeUninit<PendingJob>>, CheckpointJob),
    }
    // Borrow the actual pin throughout fallible preparation. Only the success
    // branch moves it; every preeffects refusal returns the identical owner.
    let prepared = (|| {
        let backing = reserve_with::<PendingJob>(|layout| unsafe { alloc::alloc::alloc(layout) })?;
        let lease = take_store()?;
        let store = lease.0.as_ref().ok_or("restart-not-ready")?;
        let cursor =
            network_pending::authenticated_cursor(store, owner, boot, root, &draft, request)?;
        if store.custody.as_ref().and_then(|c| c.network.as_ref()) == Some(&cursor) {
            return Ok(Prepared::Committed(CommittedCustody {
                generation: store.journal.generation().ok_or("network-pending-source")?,
                offer: cursor.bytes()[168..200].try_into().unwrap(),
            }));
        }
        let core = CheckpointJob::initialize(lease, owner, boot, root, cursor)?;
        Ok(Prepared::Writing(backing, core))
    })();
    match prepared {
        Err(reason) => Err((draft, reason)),
        Ok(Prepared::Committed(receipt)) => Ok(Start::Committed(receipt)),
        Ok(Prepared::Writing(backing, core)) => Ok(Start::Writing(Box::write(
            backing,
            PendingJob {
                core,
                _snapshot: draft,
            },
        ))),
    }
}

fn emit<S: crate::native_body_checkpoint::Storage>(
    prepared: &payload::rewrite::PreparedRewrite<'_>,
    reader: &mut PayloadReader<'_, S>,
    sink: &mut dyn FnMut(&[u8]) -> Result<(), Refusal>,
) -> Result<(payload::archive::Summary, payload::archive::Span), Refusal> {
    let mut refused = None;
    prepared
        .emit(
            |out| {
                reader.read(out).map_err(|e| {
                    refused = Some(e);
                })
            },
            sink,
        )
        .map_err(|e| refused.unwrap_or_else(|| stream_refusal(e)))
}
impl CheckpointJob {
    fn initialize(
        mut lease: StoreLease,
        owner: &SealedBodyLifecycleSession,
        boot: &conduit_core::BootId,
        root: &[u8; 32],
        cursor: payload::network::Cursor,
    ) -> Result<Self, &'static str> {
        let store = lease.0.as_mut().ok_or("restart-not-ready")?;
        let key = live_key(store, owner, root)?;
        cursor.validate(&owner.evidence().body_id, &key.verifying_key())?;
        if cursor.bytes()[8] != 1
            || cursor.bytes()[96..128] != Sha256::digest(boot.as_str().as_bytes())[..]
        {
            return Err("network-pending-producer");
        }
        let previous = store.custody.as_ref().ok_or("network-pending-source")?;
        let prepared = prepare(previous, owner, store.admissions.as_ref(), root, &cursor)?;
        let mut metadata = None;
        let writer = IncrementalRewrite::prepare(
            &mut store.journal,
            &previous.source,
            &store.binding,
            store.seed.as_ref().ok_or("network-pending-key")?,
            |reader, sink| {
                let next = emit(&prepared, reader, sink)?;
                if next.0 != previous.summary {
                    return Err(Refusal::Corrupt);
                }
                metadata = Some(next);
                Ok(())
            },
        )
        .map_err(checkpoint_commit_refusal)?;
        drop(prepared);
        let (summary, span) = metadata.ok_or("network-pending-metadata")?;
        store.ready = false;
        Ok(Self {
            lease,
            cursor: Some(cursor),
            writer: Some(writer),
            summary,
            span,
            root: *root,
            effects: false,
            adopted: false,
            poisoned: false,
        })
    }
    pub(crate) fn operation(&self) -> Option<Operation> {
        if self.poisoned {
            None
        } else {
            self.writer.as_ref().and_then(|w| w.operation())
        }
    }
    pub(crate) fn step(&self) -> u32 {
        self.writer.as_ref().map_or(u32::MAX, |w| w.step())
    }
    pub(crate) fn needs_chunk(&self) -> bool {
        !self.poisoned && self.writer.as_ref().is_some_and(|w| w.needs_chunk())
    }
    pub(crate) fn is_complete(&self) -> bool {
        !self.poisoned && self.writer.as_ref().is_some_and(|w| w.is_complete())
    }
    pub(crate) fn prepare_chunk(
        &mut self,
        owner: &SealedBodyLifecycleSession,
    ) -> Result<(), &'static str> {
        if self.poisoned {
            return Err("network-pending-job-failed");
        }
        let result = (|| {
            if !owner.pending_archives().is_empty() {
                return Err("network-pending-live-body");
            }
            let store = self.lease.0.as_mut().ok_or("restart-not-ready")?;
            let previous = store.custody.as_ref().ok_or("network-pending-source")?;
            let prepared = prepare(
                previous,
                owner,
                store.admissions.as_ref(),
                &self.root,
                self.cursor.as_ref().ok_or("network-pending-phase")?,
            )?;
            self.writer
                .as_mut()
                .ok_or("network-pending-phase")?
                .prepare_chunk(&mut store.journal, |reader, sink| {
                    let next = emit(&prepared, reader, sink)?;
                    if next.0 != self.summary || next.1 != self.span {
                        return Err(Refusal::Corrupt);
                    }
                    Ok(())
                })
                .map_err(checkpoint_commit_refusal)
        })();
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    pub(crate) fn advance(
        &mut self,
        access: &mut impl SectorStorage<BodyRegion>,
    ) -> Result<bool, &'static str> {
        if self.poisoned {
            return Err("network-pending-job-failed");
        }
        let writer = self.writer.as_mut().ok_or("network-pending-phase")?;
        if writer.operation().is_none() {
            return Err("network-pending-operation");
        }
        // A failed hardware call may already have effects; fence even if it
        // reports failure before advancing the checked ordinal.
        self.effects = true;
        let result = writer
            .advance(
                &mut self.lease.0.as_mut().ok_or("restart-not-ready")?.journal,
                access,
            )
            .map_err(checkpoint_commit_refusal);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    pub(crate) fn finish(
        &mut self,
        owner: &SealedBodyLifecycleSession,
    ) -> Result<CommittedCustody, &'static str> {
        if !self.is_complete() || !owner.pending_archives().is_empty() {
            return Err("network-pending-incomplete");
        }
        let store = self.lease.0.as_mut().ok_or("restart-not-ready")?;
        let previous = store.custody.as_ref().ok_or("network-pending-source")?;
        let cursor = self.cursor.as_ref().ok_or("network-pending-phase")?;
        let prepared = prepare(
            previous,
            owner,
            store.admissions.as_ref(),
            &self.root,
            cursor,
        )?;
        // Reconstitute the prepared emitter's immutable witness and frame range
        // without programming or copying any retained Body DTO.
        let next = store
            .journal
            .read_snapshot(&previous.source, |reader| {
                emit(&prepared, reader, &mut |_| Ok(()))
            })
            .map_err(checkpoint_commit_refusal)?;
        if next.0 != self.summary || next.1 != self.span {
            return Err("network-pending-metadata");
        }
        let completion = self
            .writer
            .take()
            .ok_or("network-pending-phase")?
            .finish(&mut store.journal)
            .map_err(checkpoint_commit_refusal)?;
        let (source, proof) = prepared.committed(completion)?;
        drop(prepared);
        let generation = source.generation();
        let offer = cursor.bytes()[168..200].try_into().unwrap();
        let current = store.custody.as_mut().ok_or("network-pending-source")?;
        current.source = source;
        current.proof = Some(proof);
        current.summary = self.summary.clone();
        current.span = self.span;
        current.network = self.cursor.take();
        store.ready = true;
        self.adopted = true;
        Ok(CommittedCustody { generation, offer })
    }
    fn fence(&mut self) {
        if !self.adopted {
            if let Some(store) = self.lease.0.as_mut() {
                if self.effects {
                    store.failed = true;
                    store.ready = false;
                } else {
                    store.ready = true;
                }
            }
        }
    }
}
impl Drop for CheckpointJob {
    fn drop(&mut self) {
        self.fence();
    }
}
/// Nonoptional physical snapshot custody: safe Rust cannot release the frozen
/// region while a production checkpoint job remains live.
#[cfg(all(
    feature = "local-appliance",
    feature = "body-persistence",
    target_os = "none"
))]
pub(crate) struct PendingJob {
    core: CheckpointJob,
    _snapshot: crate::native_play::journal::snapshot::Draft,
}
#[cfg(all(
    feature = "local-appliance",
    feature = "body-persistence",
    target_os = "none"
))]
impl PendingJob {
    pub(crate) fn operation(&self) -> Option<Operation> {
        self.core.operation()
    }
    pub(crate) fn step(&self) -> u32 {
        self.core.step()
    }
    pub(crate) fn needs_chunk(&self) -> bool {
        self.core.needs_chunk()
    }
    pub(crate) fn is_complete(&self) -> bool {
        self.core.is_complete()
    }
    pub(crate) fn prepare_chunk(
        &mut self,
        owner: &SealedBodyLifecycleSession,
    ) -> Result<(), &'static str> {
        self.core.prepare_chunk(owner)
    }
    pub(crate) fn advance(
        &mut self,
        access: &mut impl SectorStorage<BodyRegion>,
    ) -> Result<bool, &'static str> {
        self.core.advance(access)
    }
    pub(crate) fn finish(
        mut self: Box<Self>,
        owner: &SealedBodyLifecycleSession,
    ) -> Result<CommittedCustody, &'static str> {
        self.core.finish(owner)
    }
}
#[cfg(test)]
#[path = "native_restart_network_pending_job_tests.rs"]
mod tests;
