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
    frames: Option<(frames::Range, u64)>,
    root: Option<[u8; 32]>,
    fingerprint: [u8; 32],
    source: SourceIdentity,
}
fn fingerprint(
    prefix: Option<&wire::Receipt>,
    layouts: &layout::Layouts,
    network: Option<&network::Cursor>,
) -> Result<[u8; 32], &'static str> {
    let mut hash = Sha256::new();
    hash.update(b"conduit.environment/private-rewrite-receipts@1\0");
    json::stream(&(prefix, layouts, network), &mut |bytes: &[u8]| {
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
        Self::verify_network(body, key, prefix, layouts, None)
    }
    pub(super) fn verify_network(
        body: &conduit_body::BodyId,
        key: Option<&ed25519_dalek::VerifyingKey>,
        prefix: Option<&wire::Receipt>,
        layouts: &layout::Layouts,
        network: Option<&network::Cursor>,
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
            if let Some(network) = network {
                network.validate(body, key)?;
            }
        } else if prefix.is_some() || !layouts.is_empty() || network.is_some() {
            return Err("receipts require pinned Root");
        }
        Ok(Self {
            body: binding(body),
            root: key.map(|k| k.to_bytes()),
            fingerprint: fingerprint(prefix, layouts, network)?,
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
            frames: None,
            root: self.root,
            fingerprint: self.fingerprint,
            source: source.identity(),
        })
    }
    pub(super) fn bind_framing(
        self,
        source: &SourceSnapshot,
        framing: Option<frames::Validated>,
        count: u64,
    ) -> Result<AuthenticatedReceipts, &'static str> {
        let mut proof = self.bind(source)?;
        proof.frames = framing.map(|f| (f.into_range(), count));
        Ok(proof)
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
        network: Option<&network::Cursor>,
    ) -> Result<(), &'static str> {
        if self.source != source.identity()
            || source.binding() != &binding(body)
            || self.root != Some(*root)
            || self.fingerprint != fingerprint(prefix, layouts, network)?
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
    transition_checked: bool,
    framed: bool,
    reuse: Option<frames::Range>,
    emitted_frames: Cell<Option<frames::Range>>,
    evidence: &'a BodyBiographyEvidence,
    pending: &'a [BodyBiographyArchiveSegment],
    admissions: &'a AdmissionManager,
    previous: &'a archive::Summary,
    span: archive::Span,
    prefix: Option<&'a wire::Receipt>,
    local_count: u64,
    layouts: &'a layout::Layouts,
    network: Option<&'a network::Cursor>,
    summary: archive::Summary,
    checked: &'a AuthenticatedReceipts,
    output_fingerprint: [u8; 32],
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
        Self::prepare_network(
            checked,
            source,
            evidence,
            pending,
            admissions,
            previous,
            span,
            prefix,
            local_count,
            root,
            layouts,
            None,
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare_network(
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
        network: Option<&'a network::Cursor>,
    ) -> Result<Self, &'static str> {
        checked.check(source, &evidence.body_id, root, prefix, layouts, network)?;
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
            transition_checked: false,
            framed: false,
            reuse: None,
            emitted_frames: Cell::new(None),
            evidence,
            pending,
            admissions,
            previous,
            span,
            prefix,
            local_count,
            layouts,
            network,
            summary,
            checked,
            output_fingerprint: checked.fingerprint,
        })
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare_framed(
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
        Self::prepare_framed_network(
            checked,
            source,
            evidence,
            pending,
            admissions,
            previous,
            span,
            prefix,
            local_count,
            root,
            layouts,
            None,
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare_framed_network(
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
        network: Option<&'a network::Cursor>,
    ) -> Result<Self, &'static str> {
        let mut result = Self::prepare_network(
            checked,
            source,
            evidence,
            pending,
            admissions,
            previous,
            span,
            prefix,
            local_count,
            root,
            layouts,
            network,
        )?;
        result.reuse = checked
            .frames
            .map(|(range, count)| {
                if range.plain != span || count != local_count {
                    return Err("restart-frame-source");
                }
                Ok(range)
            })
            .transpose()?;
        result.framed = true;
        Ok(result)
    }
    /// Explicit receipt transition: authenticate the old source with its old
    /// fingerprint, then separately authenticate the new Pending and its chain.
    /// Neither signature checking nor this prepared writer grants erase custody.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare_pending_network(
        checked: &'a AuthenticatedReceipts,
        source: &SourceSnapshot,
        evidence: &'a BodyBiographyEvidence,
        admissions: &'a AdmissionManager,
        previous: &'a archive::Summary,
        span: archive::Span,
        prefix: Option<&'a wire::Receipt>,
        local_count: u64,
        root: &[u8; 32],
        layouts: &'a layout::Layouts,
        current: Option<&'a network::Cursor>,
        next: &'a network::Cursor,
    ) -> Result<Self, &'static str> {
        let mut result = Self::prepare_framed_network(
            checked,
            source,
            evidence,
            &[],
            admissions,
            previous,
            span,
            prefix,
            local_count,
            root,
            layouts,
            current,
        )?;
        let key =
            ed25519_dalek::VerifyingKey::from_bytes(root).map_err(|_| "network-pending-root")?;
        next.validate(&evidence.body_id, &key)?;
        let (epoch, predecessor) = match current {
            Some(cursor) => cursor.offer_chain(&evidence.body_id, &key)?,
            None => (0, [0; 32]),
        };
        if next.bytes()[8] != 1
            || next.bytes()[16..24] != epoch.to_le_bytes()
            || next.bytes()[360..392] != predecessor
            || epoch == u64::MAX
        {
            return Err("network-pending-chain");
        }
        result.output_fingerprint = fingerprint(prefix, layouts, Some(next))?;
        result.network = Some(next);
        result.transition_checked = true;
        Ok(result)
    }
    pub(crate) fn emit<E>(
        &self,
        read: impl FnMut(&mut [u8; 128]) -> Result<usize, ()>,
        sink: &mut dyn FnMut(&[u8]) -> Result<(), E>,
    ) -> Result<(archive::Summary, archive::Span), StreamRefusal<E>> {
        let mut hash = Sha256::new();
        let mut length = 0usize;
        let mut output = |bytes: &[u8]| {
            sink(bytes)?;
            length += bytes.len();
            hash.update(bytes);
            Ok(())
        };
        let result = if self.framed {
            let (summary, span, range) = framed_rewrite::emit(
                self.evidence,
                self.pending,
                self.admissions,
                self.span,
                self.prefix,
                self.local_count,
                self.layouts,
                self.network,
                self.summary.clone(),
                self.reuse,
                read,
                &mut output,
            )?;
            if self.emitted_frames.get().is_some_and(|old| old != range) {
                return Err(StreamRefusal::Invalid);
            }
            self.emitted_frames.set(Some(range));
            (summary, span)
        } else {
            emit_rewrite(
                self.evidence,
                self.pending,
                self.admissions,
                self.previous,
                self.span,
                self.prefix,
                self.local_count,
                self.layouts,
                self.network,
                self.summary.clone(),
                read,
                &mut output,
            )?
        };
        let transcript = (length, <[u8; 32]>::from(hash.finalize()));
        if self.emitted.get().is_some_and(|prior| prior != transcript) {
            return Err(StreamRefusal::Invalid);
        }
        self.emitted.set(Some(transcript));
        Ok(result)
    }

    /// Only an actual successful rewrite/readback snapshot can advance custody.
    /// The output fingerprint was authenticated during preparation. This
    /// cannot change Body or Root and is not an erase entrance.
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
        let frames = if self.framed {
            Some((
                self.emitted_frames
                    .get()
                    .ok_or("restart-frame-not-emitted")?,
                self.local_count
                    .checked_add(self.pending.len() as u64)
                    .ok_or("restart-frame-count")?,
            ))
        } else {
            None
        };
        let proof = AuthenticatedReceipts {
            frames,
            root: self.checked.root,
            fingerprint: self.output_fingerprint,
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

#[cfg(all(
    test,
    any(feature = "observation-store", feature = "local-evidence-store")
))]
#[path = "native_restart_frames_tests.rs"]
mod frame_tests;

#[path = "native_restart_rewrite_network.rs"]
mod network_transition;
pub(crate) use network_transition::AuthenticatedNetworkTransition;
