//! Private validation custody, distinct from physical or membership authority.
//! Parsed DTOs cannot construct these tokens. Fingerprints only invalidate an
//! existing signature proof; they never create one.
use super::*;
use crate::native_body_checkpoint::{RewriteCompletion, SourceIdentity, SourceSnapshot};
use core::cell::Cell;
use sha2::{Digest, Sha256};

pub(crate) struct ValidatedReceipts {
    body: [u8; 32],
    root: Option<[u8; 32]>,
    fingerprint: [u8; 32],
}
pub(crate) struct AuthenticatedReceipts {
    root: Option<[u8; 32]>,
    fingerprint: [u8; 32],
    source: SourceIdentity,
}
fn fingerprint(
    prefix: Option<&wire::Receipt>,
    layouts: &layout::Layouts,
) -> Result<[u8; 32], &'static str> {
    let mut hash = Sha256::new();
    hash.update(b"conduit.environment/private-rewrite-receipts@1\0");
    json::stream(&(prefix, layouts), &mut |bytes: &[u8]| {
        hash.update(bytes);
        Ok::<_, json::CodecError>(())
    })
    .map_err(|_| "restart-receipt-fingerprint")?;
    Ok(hash.finalize().into())
}
fn binding(body: &conduit_body::BodyId) -> [u8; 32] {
    Sha256::digest(body.as_str().as_bytes()).into()
}
impl ValidatedReceipts {
    /// The sole constructor performs all signatures; raw JSON and digests are
    /// insufficient. Caller still owes full physical source authentication.
    pub(super) fn verify(
        body: &conduit_body::BodyId,
        key: Option<&ed25519_dalek::VerifyingKey>,
        prefix: Option<&wire::Receipt>,
        layouts: &layout::Layouts,
    ) -> Result<Self, &'static str> {
        #[cfg(test)]
        VERIFICATION_PASSES.with(|n| n.set(n.get() + 1));
        if let Some(key) = key {
            if let Some(prefix) = prefix {
                prefix
                    .verify(body, key)
                    .map_err(|_| "restart-prefix-signature")?;
            }
            layouts.validate(body, key)?;
        } else if prefix.is_some() || !layouts.is_empty() {
            return Err("receipts require pinned Root");
        }
        Ok(Self {
            body: binding(body),
            root: key.map(|k| k.to_bytes()),
            fingerprint: fingerprint(prefix, layouts)?,
        })
    }
    pub(super) fn bind(
        self,
        source: &SourceSnapshot,
    ) -> Result<AuthenticatedReceipts, &'static str> {
        if source.binding() != &self.body {
            return Err("restart-receipt-source-body");
        }
        Ok(AuthenticatedReceipts {
            root: self.root,
            fingerprint: self.fingerprint,
            source: source.identity(),
        })
    }
}
impl AuthenticatedReceipts {
    fn check(
        &self,
        source: &SourceSnapshot,
        body: &conduit_body::BodyId,
        root: &[u8; 32],
        prefix: Option<&wire::Receipt>,
        layouts: &layout::Layouts,
    ) -> Result<(), &'static str> {
        if self.source != source.identity()
            || source.binding() != &binding(body)
            || self.root != Some(*root)
            || self.fingerprint != fingerprint(prefix, layouts)?
        {
            return Err("restart-receipt-custody-changed");
        }
        Ok(())
    }
}
/// Immutable references freeze every deterministic-pass input. No mutation of
/// the manager, evidence, pending segments or receipts can coexist with this.
pub(crate) struct PreparedRewrite<'a> {
    emitted: Cell<Option<(usize, [u8; 32])>>,
    evidence: &'a BodyBiographyEvidence,
    pending: &'a [BodyBiographyArchiveSegment],
    admissions: &'a AdmissionManager,
    previous: &'a archive::Summary,
    span: archive::Span,
    prefix: Option<&'a wire::Receipt>,
    local_count: u64,
    layouts: &'a layout::Layouts,
    summary: archive::Summary,
    checked: &'a AuthenticatedReceipts,
}
impl<'a> PreparedRewrite<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare(
        checked: &'a AuthenticatedReceipts,
        source: &SourceSnapshot,
        evidence: &'a BodyBiographyEvidence,
        pending: &'a [BodyBiographyArchiveSegment],
        admissions: &'a AdmissionManager,
        previous: &'a archive::Summary,
        span: archive::Span,
        prefix: Option<&'a wire::Receipt>,
        local_count: u64,
        root: &[u8; 32],
        layouts: &'a layout::Layouts,
    ) -> Result<Self, &'static str> {
        checked.check(source, &evidence.body_id, root, prefix, layouts)?;
        let summary = validate_rewrite_state::<()>(
            evidence,
            pending,
            admissions,
            previous,
            prefix,
            local_count,
        )
        .map_err(|_| "restart-rewrite-state")?;
        Ok(Self {
            emitted: Cell::new(None),
            evidence,
            pending,
            admissions,
            previous,
            span,
            prefix,
            local_count,
            layouts,
            summary,
            checked,
        })
    }
    pub(crate) fn emit<E>(
        &self,
        read: impl FnMut(&mut [u8; 128]) -> Result<usize, ()>,
        sink: &mut dyn FnMut(&[u8]) -> Result<(), E>,
    ) -> Result<(archive::Summary, archive::Span), StreamRefusal<E>> {
        let mut hash = Sha256::new();
        let mut length = 0usize;
        let result = emit_rewrite(
            self.evidence,
            self.pending,
            self.admissions,
            self.previous,
            self.span,
            self.prefix,
            self.local_count,
            self.layouts,
            self.summary.clone(),
            read,
            &mut |bytes: &[u8]| {
                sink(bytes)?;
                length += bytes.len();
                hash.update(bytes);
                Ok(())
            },
        )?;
        let transcript = (length, <[u8; 32]>::from(hash.finalize()));
        if self.emitted.get().is_some_and(|prior| prior != transcript) {
            return Err(StreamRefusal::Invalid);
        }
        self.emitted.set(Some(transcript));
        Ok(result)
    }

    /// Only an actual successful rewrite/readback snapshot can advance custody.
    /// This cannot change receipts, Body or Root, and is not an erase entrance.
    pub(crate) fn committed(
        &self,
        completion: RewriteCompletion,
    ) -> Result<(SourceSnapshot, AuthenticatedReceipts), &'static str> {
        let (length, digest) = self.emitted.get().ok_or("restart-rewrite-not-emitted")?;
        if !completion.retains_seed()
            || !completion.encoded_matches(length, &digest)
            || !completion.continues_source(self.checked.source)
        {
            return Err("restart-rewrite-commit-source");
        }
        let source = completion.into_source();
        let proof = AuthenticatedReceipts {
            root: self.checked.root,
            fingerprint: self.checked.fingerprint,
            source: source.identity(),
        };
        Ok((source, proof))
    }
}
#[cfg(test)]
std::thread_local! {static VERIFICATION_PASSES:core::cell::Cell<usize>=const{core::cell::Cell::new(0)};}
#[cfg(test)]
pub(super) fn verification_passes() -> usize {
    VERIFICATION_PASSES.with(|n| n.get())
}
