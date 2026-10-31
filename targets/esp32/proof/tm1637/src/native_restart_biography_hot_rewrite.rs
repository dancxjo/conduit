//! Receipt-authenticated projection of one unchanged private source. No Body
//! DTO, admission manager, archive vector or output document is materialized.
use super::*;
use crate::native_body_checkpoint::{RewriteCompletion, SourceIdentity, SourceSnapshot};
use core::cell::Cell;
use sha2::Digest;
#[path = "native_restart_biography_hot_json.rs"]
mod projection;

pub(crate) struct Recipe {
    source: SourceIdentity,
    old_span: archive::Span,
    old_prefix: bool,
    receipt: wire::Receipt,
    validation: Option<rewrite::ValidatedReceipts>,
    emitted: Cell<Option<(usize, [u8; 32], archive::Span)>>,
}
impl Recipe {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare(
        source: &SourceSnapshot,
        body: &conduit_body::BodyId,
        root: &[u8; 32],
        span: archive::Span,
        old_prefix: bool,
        receipt: wire::Receipt,
        layouts: &layout::Layouts,
        network: Option<&network::Cursor>,
    ) -> Result<Self, &'static str> {
        if source.binding() != &<[u8; 32]>::from(sha2::Sha256::digest(body.as_str().as_bytes())) {
            return Err("biography-hot-source-binding");
        }
        let key =
            ed25519_dalek::VerifyingKey::from_bytes(root).map_err(|_| "biography-hot-root")?;
        let validation = rewrite::ValidatedReceipts::verify_network(
            body,
            Some(&key),
            Some(&receipt),
            layouts,
            network,
        )?;
        Ok(Self {
            source: source.identity(),
            old_span: span,
            old_prefix,
            receipt,
            validation: Some(validation),
            emitted: Cell::new(None),
        })
    }
    pub(crate) fn receipt(&self) -> &wire::Receipt {
        &self.receipt
    }
    pub(crate) fn into_receipt(self) -> wire::Receipt {
        self.receipt
    }
    pub(crate) fn planned_bytes(&self) -> Option<usize> {
        self.emitted.get().map(|v| v.0)
    }
    pub(crate) fn emit<E>(
        &self,
        read: impl FnMut(&mut [u8; 128]) -> Result<usize, ()>,
        sink: &mut dyn FnMut(&[u8]) -> Result<(), E>,
    ) -> Result<archive::Span, StreamRefusal<E>> {
        let mut failure = None;
        let mut length = 0usize;
        let mut hash = sha2::Sha256::new();
        let mut output = |bytes: &[u8]| {
            sink(bytes).map_err(|e| {
                failure = Some(e);
            })?;
            length = length
                .checked_add(bytes.len())
                .filter(|n| *n <= MAX_BYTES)
                .ok_or(())?;
            hash.update(bytes);
            Ok(())
        };
        let mut index = compress::MatchIndex::new();
        let mut encoder =
            compress::Encoder::with_index(&mut output, MAX_PLAIN_BYTES, MAX_BYTES, &mut index);
        let projected = with_plain(read, |plain| {
            projection::rewrite(
                plain,
                &self.receipt,
                self.old_span,
                self.old_prefix,
                &mut |bytes| encoder.write(bytes).map_err(|_| ()),
            )
            .map_err(|_| "biography-hot-projection")
        });
        let finished = if projected.is_ok() {
            encoder.finish().map(|_| ())
        } else {
            Err(compress::Error::Invalid)
        };
        drop(encoder);
        if let Some(error) = failure {
            return Err(StreamRefusal::Sink(error));
        }
        let span = projected.map_err(|_| StreamRefusal::Invalid)?;
        finished.map_err(|_| StreamRefusal::Capacity)?;
        let transcript = (length, hash.finalize().into(), span);
        if self
            .emitted
            .get()
            .is_some_and(|previous| previous != transcript)
        {
            return Err(StreamRefusal::Invalid);
        }
        self.emitted.set(Some(transcript));
        Ok(span)
    }
    pub(crate) fn committed(
        &mut self,
        completion: RewriteCompletion,
    ) -> Result<
        (
            SourceSnapshot,
            rewrite::AuthenticatedReceipts,
            archive::Span,
        ),
        &'static str,
    > {
        let (length, digest, span) = self.emitted.get().ok_or("biography-hot-not-emitted")?;
        if !completion.retains_seed()
            || !completion.continues_source(self.source)
            || !completion.encoded_matches(length, &digest)
        {
            return Err("biography-hot-readback-source");
        }
        let source = completion.into_source();
        let proof = self
            .validation
            .take()
            .ok_or("biography-hot-already-committed")?
            .bind(&source)?;
        Ok((source, proof, span))
    }
}
