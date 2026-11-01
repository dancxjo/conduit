//! Explicit Pending-to-Clean receipt transition, preserving exact member custody.
use super::*;
impl<'a> PreparedRewrite<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare_clean_network(
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
        current: &'a network::Cursor,
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
            Some(current),
        )?;
        let key =
            ed25519_dalek::VerifyingKey::from_bytes(root).map_err(|_| "network-clean-root")?;
        current.validate(&evidence.body_id, &key)?;
        next.validate(&evidence.body_id, &key)?;
        let epoch = u64::from_le_bytes(current.bytes()[16..24].try_into().unwrap())
            .checked_add(1)
            .ok_or("network-clean-epoch")?;
        if current.bytes()[8] != 1
            || next.bytes()[8] != 2
            || next.bytes()[16..24] != epoch.to_le_bytes()
            || next.bytes()[24..408] != current.bytes()[24..408]
        {
            return Err("network-clean-transition");
        }
        result.output_fingerprint = fingerprint(prefix, layouts, Some(next))?;
        result.network = Some(next);
        result.transition_checked = true;
        Ok(result)
    }
}

/// Private validation custody for one exact source and authenticated receipt
/// transition. A hash can invalidate this witness, but cannot construct one.
/// It supplies neither physical storage access nor commit/erase authority.
pub(crate) struct AuthenticatedNetworkTransition {
    source: SourceIdentity,
    binding: [u8; 32],
}
fn transition_binding(prepared: &PreparedRewrite<'_>, output: &[u8; 32]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"conduit.environment/private-network-transition@1\0");
    hash.update(prepared.checked.root.unwrap_or([0; 32]));
    hash.update(binding(&prepared.evidence.body_id));
    hash.update(prepared.checked.fingerprint);
    hash.update(output);
    hash.finalize().into()
}
impl<'a> PreparedRewrite<'a> {
    /// Only a successful signature-checked Pending/Clean preparation can issue
    /// this owned witness. Raw source, fingerprints and DTOs cannot issue it.
    pub(crate) fn authenticated_network_transition(
        &self,
    ) -> Result<AuthenticatedNetworkTransition, &'static str> {
        if !self.transition_checked {
            return Err("network-transition-not-authenticated");
        }
        Ok(AuthenticatedNetworkTransition {
            source: self.checked.source,
            binding: transition_binding(self, &self.output_fingerprint),
        })
    }
    /// Reconstitute a deterministic recipe under existing receipt/source
    /// custody, reusing only this exact transition's earlier signature checks.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare_authenticated_network_transition(
        transition: &AuthenticatedNetworkTransition,
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
        let output = fingerprint(prefix, layouts, Some(next))?;
        if transition.source != source.identity()
            || transition.binding != transition_binding(&result, &output)
        {
            return Err("network-transition-custody-changed");
        }
        result.output_fingerprint = output;
        result.network = Some(next);
        result.transition_checked = true;
        Ok(result)
    }
}
