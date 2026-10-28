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
        Ok(result)
    }
}
