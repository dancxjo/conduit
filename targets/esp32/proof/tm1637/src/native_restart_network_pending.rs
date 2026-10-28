//! Dedicated live Pending transition. No cursor setter, erase API or transport.
//! Physical readback is required before a committed result can leave this module.
use super::*;
use crate::native_body_checkpoint::Storage;
use conduit_core::BootId;

/// Describes an accepted checkpoint, never permission to erase a region.
/// The caller still owns the actual pinned snapshot and must use the restart
/// recovery protocol before any network evidence can be changed.
pub(crate) struct CommittedCustody {
    generation: u64,
    offer: [u8; 32],
}
impl CommittedCustody {
    /// Current checkpoint carrying this receipt, including later rewrites.
    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }
    pub(crate) fn offer(&self) -> &[u8; 32] {
        &self.offer
    }
}
fn live_key(
    store: &Store,
    owner: &SealedBodyLifecycleSession,
    root: &[u8; 32],
) -> Result<ed25519_dalek::SigningKey, &'static str> {
    if store.failed || store.parked || !store.ready {
        return Err("network-pending-requires-live-custody");
    }
    let body = owner.evidence();
    if <[u8; 32]>::from(Sha256::digest(body.body_id.as_str().as_bytes())) != store.binding
        || body.body.body_id != body.body_id
        || body.membership.body_id != body.body_id
        || !owner.pending_archives().is_empty()
        || !store.archives.is_empty()
    {
        return Err("network-pending-live-body");
    }
    let previous = store.custody.as_ref().ok_or("network-pending-source")?;
    if previous.source.binding() != &store.binding
        || Some(previous.source.generation()) != store.journal.generation()
    {
        return Err("network-pending-source");
    }
    let key =
        ed25519_dalek::SigningKey::from_bytes(store.seed.as_ref().ok_or("network-pending-key")?);
    if key.verifying_key().as_bytes() != root {
        return Err("network-pending-root");
    }
    Ok(key)
}
/// Actual Root entrance. Canonical member checks, current chain validation and
/// Root signing occur under the same exclusive Store lease as the write.
#[cfg(all(
    feature = "local-appliance",
    feature = "body-persistence",
    target_os = "none"
))]
pub(crate) fn commit_pending(
    owner: &SealedBodyLifecycleSession,
    boot: &BootId,
    root: &[u8; 32],
    draft: &crate::native_play::journal::snapshot::Draft,
    request: &crate::native_play::journal::snapshot::ack::Request<'_>,
) -> Result<CommittedCustody, &'static str> {
    use crate::native_play::journal::snapshot::{ack, Scope};
    use ed25519_dalek::Signer;
    with_store(|store| {
        let key = live_key(store, owner, root)?;
        if draft.root() != root {
            return Err("network-pending-snapshot-root");
        }
        let current = store.custody.as_ref().and_then(|c| c.network.as_ref());
        let cursor = if let Some(current) = current.filter(|c| c.bytes()[8] == 1) {
            current.validate(&owner.evidence().body_id, &key.verifying_key())?;
            // Exact lost-ACK retry is a read of previously committed custody.
            // It grants no new admission and never rewrites the checkpoint.
            if !request.matches_pending(draft, &owner.evidence().body_id, current.bytes()) {
                return Err("network-pending-already-committed");
            }
            current.try_clone()?
        } else {
            let (epoch, predecessor) = match current {
                Some(cursor) => {
                    cursor.offer_chain(&owner.evidence().body_id, &key.verifying_key())?
                }
                None => (0, [0; 32]),
            };
            if !draft.matches_scope(Scope {
                body: &owner.evidence().body_id,
                boot,
                epoch,
                predecessor,
            }) {
                return Err("network-pending-stale-snapshot");
            }
            let candidate = ack::verify(
                draft,
                store
                    .admissions
                    .as_ref()
                    .ok_or("network-pending-admissions")?,
                &owner.evidence().membership,
                request,
            )?;
            if candidate.membership_revision() != owner.evidence().membership.revision.0
                || candidate.root() != root
            {
                return Err("network-pending-stale-membership");
            }
            let signature = key.sign(&candidate.transcript()).to_bytes();
            let signed = candidate.signed(signature);
            payload::network::Cursor::decode(*signed.bytes())?
        };
        persist_authenticated(store, owner, boot, root, cursor)
    })
}
/// Replays an exact authenticated historical custody receipt without requiring
/// a new physical snapshot or the producer's old Boot to be live. A later member
/// return/revocation cannot undo an already committed recording.
#[cfg(all(
    feature = "local-appliance",
    feature = "body-persistence",
    target_os = "none"
))]
pub(crate) fn replay_committed(
    owner: &SealedBodyLifecycleSession,
    root: &[u8; 32],
    request: &crate::native_play::journal::snapshot::ack::Request<'_>,
) -> Result<Option<CommittedCustody>, &'static str> {
    with_store(|store| {
        read_committed(store, owner, root, |bytes| {
            request.matches_receipt(&owner.evidence().body_id, bytes)
        })
    })
}
fn read_committed(
    store: &Store,
    owner: &SealedBodyLifecycleSession,
    root: &[u8; 32],
    matches: impl FnOnce(&[u8; 480]) -> bool,
) -> Result<Option<CommittedCustody>, &'static str> {
    let key = live_key(store, owner, root)?;
    let custody = store.custody.as_ref().ok_or("network-pending-source")?;
    let Some(cursor) = custody.network.as_ref() else {
        return Ok(None);
    };
    cursor.validate(&owner.evidence().body_id, &key.verifying_key())?;
    Ok(matches(cursor.bytes()).then(|| CommittedCustody {
        generation: custody.source.generation(),
        offer: cursor.bytes()[168..200].try_into().unwrap(),
    }))
}
/// Private writer core; the production entrance above supplies actual snapshot
/// and admitted-member provenance. CPU writer tests use signed protocol fixtures.
fn persist_authenticated(
    store: &mut Store,
    owner: &SealedBodyLifecycleSession,
    boot: &BootId,
    root: &[u8; 32],
    cursor: payload::network::Cursor,
) -> Result<CommittedCustody, &'static str> {
    let key = live_key(store, owner, root)?;
    cursor.validate(&owner.evidence().body_id, &key.verifying_key())?;
    if cursor.bytes()[8] != 1 {
        return Err("network-pending-phase");
    }
    let previous = store.custody.as_ref().ok_or("network-pending-source")?;
    let result = CommittedCustody {
        generation: previous.source.generation(),
        offer: cursor.bytes()[168..200].try_into().unwrap(),
    };
    if previous.network.as_ref() == Some(&cursor) {
        return Ok(result);
    }
    if cursor.bytes()[96..128] != Sha256::digest(boot.as_str().as_bytes())[..] {
        return Err("network-pending-producer");
    }
    // All candidate storage is reserved and authenticated before mutation.
    // A failed write is ambiguous: fence this Store until reboot authenticates
    // the surviving bank. Do not return a receipt or permit a fresh live retry.
    let prepared = prepare(previous, owner, store.admissions.as_ref(), root, &cursor)?;
    store.ready = false;
    let next = write_prepared(
        &mut store.journal,
        &store.binding,
        store.seed.as_ref().ok_or("network-pending-key")?,
        previous,
        &prepared,
    )
    .inspect_err(|_| {
        store.failed = true;
    })?;
    drop(prepared);
    let generation = next.source.generation();
    // Carry immutable layout/prefix custody in place; no duplicate DTO or
    // post-commit allocation is needed to publish the authenticated successor.
    let current = store.custody.as_mut().ok_or("network-pending-source")?;
    current.source = next.source;
    current.proof = Some(next.proof);
    current.summary = next.summary;
    current.span = next.span;
    current.network = Some(cursor);
    store.ready = true;
    Ok(CommittedCustody {
        generation,
        ..result
    })
}
fn prepare<'a>(
    previous: &'a ArchiveCustody,
    owner: &'a SealedBodyLifecycleSession,
    admissions: Option<&'a AdmissionManager>,
    root: &[u8; 32],
    cursor: &'a payload::network::Cursor,
) -> Result<payload::rewrite::PreparedRewrite<'a>, &'static str> {
    payload::rewrite::PreparedRewrite::prepare_pending_network(
        previous
            .proof
            .as_ref()
            .ok_or("network-pending-source-proof")?,
        &previous.source,
        owner.evidence(),
        admissions.ok_or("network-pending-admissions")?,
        &previous.summary,
        previous.span,
        previous.prefix.as_ref(),
        previous.local_count,
        root,
        &previous.layouts,
        previous.network.as_ref(),
        cursor,
    )
}
struct CommittedRewrite {
    source: crate::native_body_checkpoint::SourceSnapshot,
    proof: payload::rewrite::AuthenticatedReceipts,
    summary: payload::archive::Summary,
    span: payload::archive::Span,
}
/// Shared with fault-injected CPU media. The emitted transcript and new
/// fingerprint are bound to the actual two-pass writer's full physical readback.
fn write_prepared<S: Storage>(
    journal: &mut Checkpoint<S>,
    binding: &[u8; 32],
    seed: &[u8; 32],
    previous: &ArchiveCustody,
    prepared: &payload::rewrite::PreparedRewrite<'_>,
) -> Result<CommittedRewrite, &'static str> {
    let mut metadata = None;
    let completion = journal
        .rewrite_streamed_attested(&previous.source, binding, seed, |io| {
            let mut read_refusal = None;
            let next = prepared
                .emit(
                    |out| {
                        io.read_source(out).map_err(|e| {
                            read_refusal = Some(e);
                        })
                    },
                    &mut |bytes| io.write(bytes),
                )
                .map_err(|e| read_refusal.unwrap_or_else(|| stream_refusal(e)))?;
            if next.0 != previous.summary || metadata.as_ref().is_some_and(|prior| prior != &next) {
                return Err(Refusal::Corrupt);
            }
            metadata = Some(next);
            Ok(())
        })
        .map_err(checkpoint_commit_refusal)?;
    let (source, proof) = prepared.committed(completion)?;
    let (summary, span) = metadata.ok_or("network-pending-metadata")?;
    Ok(CommittedRewrite {
        source,
        proof,
        summary,
        span,
    })
}
#[cfg(test)]
#[path = "native_restart_network_pending_tests.rs"]
mod tests;
