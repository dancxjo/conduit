//! Shared finite two-pass checkpoint writer; no admission or erase authority.
use super::*;
use crate::native_body_checkpoint::Storage;
pub(super) struct CommittedRewrite {
    pub(super) source: crate::native_body_checkpoint::SourceSnapshot,
    pub(super) proof: payload::rewrite::AuthenticatedReceipts,
    pub(super) summary: payload::archive::Summary,
    pub(super) span: payload::archive::Span,
}
/// Shared with fault-injected CPU media. The emitted transcript and new
/// fingerprint are bound to the actual two-pass writer's full physical readback.
pub(super) fn write_prepared<S: Storage>(
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
