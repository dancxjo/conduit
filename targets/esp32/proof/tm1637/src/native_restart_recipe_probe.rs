//! CPU-only private probe of the real authenticated source, never target admission.
use super::super::framed_rewrite::{
    emit_pieces,
    recipe::{CaptureRefusal, ChangedPart, EncodedPiece, Limits, Recipe},
};
use super::*;
/// prefix/append/suffix changed bytes, reused bytes, original source bytes, output bytes.
pub(crate) fn probe(
    store: &mut crate::native_restart::Store,
    owner: &conduit_body::SealedBodyLifecycleSession,
    root: &[u8; 32],
) -> Result<[usize; 6], &'static str> {
    let old = store.custody.as_ref().ok_or("probe-custody")?;
    let prepared = PreparedRewrite::prepare_framed_network(
        old.proof.as_ref().ok_or("probe-proof")?,
        &old.source,
        owner.evidence(),
        owner.pending_archives(),
        store.admissions.as_ref().ok_or("probe-admissions")?,
        &old.summary,
        old.span,
        old.prefix.as_ref(),
        old.local_count,
        root,
        &old.layouts,
        old.network.as_ref(),
    )?;
    let mut dimensions = [0usize; 3];
    let mut expected = Sha256::new();
    let mut length = 0usize;
    let encode =
        |journal: &mut crate::native_body_checkpoint::Checkpoint<
            crate::native_evidence_region::BodyRegion,
        >,
         sink: &mut dyn FnMut(EncodedPiece<'_>) -> Result<(), CaptureRefusal>| {
            journal.read_snapshot(&old.source, |reader| {
                emit_pieces(
                    prepared.evidence,
                    prepared.pending,
                    prepared.admissions,
                    prepared.span,
                    prepared.prefix,
                    prepared.local_count,
                    prepared.layouts,
                    prepared.network,
                    prepared.summary.clone(),
                    prepared.reuse,
                    |out| reader.read(out).map_err(|_| ()),
                    sink,
                )
                .map_err(|_| crate::native_body_checkpoint::Refusal::Corrupt)
            })
        };
    encode(&mut store.journal, &mut |piece| {
        if let EncodedPiece::Changed { part, bytes } = &piece {
            dimensions[match part {
                ChangedPart::Prefix => 0,
                ChangedPart::Append => 1,
                ChangedPart::Suffix => 2,
            }] += bytes.len();
        }
        expected.update(piece.bytes());
        length += piece.bytes().len();
        Ok(())
    })
    .map_err(|_| "probe-measure")?;
    let (recipe, _) = Recipe::capture(
        Limits {
            prefix: dimensions[0],
            append: dimensions[1],
            suffix: dimensions[2],
        },
        |sink| encode(&mut store.journal, sink).map_err(|_| StreamRefusal::Invalid),
    )
    .map_err(|_| "probe-capture")?;
    let mut ordinary = Sha256::new();
    store
        .journal
        .read_snapshot(&old.source, |reader| {
            prepared
                .emit(|out| reader.read(out).map_err(|_| ()), &mut |bytes| {
                    ordinary.update(bytes);
                    Ok::<_, ()>(())
                })
                .map_err(|_| crate::native_body_checkpoint::Refusal::Corrupt)
        })
        .map_err(|_| "probe-ordinary")?;
    let parts = recipe.changed_parts();
    let mut actual = Sha256::new();
    actual.update(parts[0]);
    if let Some(span) = recipe.source_range() {
        store
            .journal
            .read_snapshot(&old.source, |reader| {
                let mut at = 0;
                let mut bytes = [0; 128];
                loop {
                    let n = reader.read(&mut bytes)?;
                    if n == 0 {
                        break;
                    }
                    let lo = at.max(span.start);
                    let hi = (at + n).min(span.end);
                    if lo < hi {
                        actual.update(&bytes[lo - at..hi - at]);
                    }
                    at += n;
                }
                Ok(())
            })
            .map_err(|_| "probe-reuse")?;
    }
    actual.update(parts[1]);
    actual.update(parts[2]);
    let expected: [u8; 32] = expected.finalize().into();
    if <[u8; 32]>::from(actual.finalize()) != expected
        || <[u8; 32]>::from(ordinary.finalize()) != expected
    {
        return Err("probe-recipe-bytes");
    }
    let reused = recipe.source_range().map_or(0, |s| s.end - s.start);
    Ok([
        dimensions[0],
        dimensions[1],
        dimensions[2],
        reused,
        old.source.payload_length(),
        length,
    ])
}
