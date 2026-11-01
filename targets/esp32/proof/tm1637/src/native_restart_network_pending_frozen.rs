//! Reuse the canonical frozen codec for Pending/Clean, with legacy preeffects
//! allocation fallback. Bundled preparation has no new timing qualification.
use super::*;
use crate::native_body_checkpoint::{Checkpoint, SourceSnapshot};
pub(super) fn evidence_digest(
    owner: &SealedBodyLifecycleSession,
) -> Result<[u8; 32], &'static str> {
    let mut digest = Sha256::new();
    digest.update(b"conduit.environment/network-pending-frozen-owner@1\0");
    payload::json::stream(owner.evidence(), &mut |bytes: &[u8]| {
        digest.update(bytes);
        Ok::<_, payload::json::CodecError>(())
    })
    .map_err(|_| "network-pending-owner-digest")?;
    Ok(digest.finalize().into())
}
#[allow(clippy::type_complexity)]
pub(super) fn prepare_writer(
    prepared: &payload::rewrite::PreparedRewrite<'_>,
    journal: &mut Checkpoint<BodyRegion>,
    source: &SourceSnapshot,
    binding: &[u8; 32],
    seed: &[u8; 32],
    summary: &payload::archive::Summary,
) -> Result<
    (
        IncrementalRewrite,
        Option<payload::rewrite::FrozenRewrite>,
        payload::archive::Summary,
        payload::archive::Span,
    ),
    &'static str,
> {
    if let Some(measured) = prepared.measure_frozen(journal, source)? {
        let metadata = measured.metadata();
        if &metadata.0 != summary {
            return Err("network-pending-metadata");
        }
        if let Some((frozen, writer)) =
            prepared.capture_frozen(journal, source, binding, seed, &measured)?
        {
            return Ok((writer, Some(frozen), metadata.0, metadata.1));
        }
    }
    // Only fallible cache/index allocation refusal (or unframed source) arrives
    // here. Source/input corruption is terminal, never a legacy fallback.
    let mut metadata = None;
    let writer = IncrementalRewrite::prepare(journal, source, binding, seed, |reader, sink| {
        let next = emit(prepared, reader, sink)?;
        if &next.0 != summary {
            return Err(Refusal::Corrupt);
        }
        metadata = Some(next);
        Ok(())
    })
    .map_err(checkpoint_commit_refusal)?;
    let (summary, span) = metadata.ok_or("network-pending-metadata")?;
    Ok((writer, None, summary, span))
}
