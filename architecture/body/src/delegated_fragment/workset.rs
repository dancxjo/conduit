//! Approval commits to the complete local fragment set, without another catalog.
use super::*;

fn update(hash: &mut Sha256, id: &str) {
    hash.update((id.len() as u32).to_le_bytes());
    hash.update(id.as_bytes());
}
fn begin() -> Sha256 {
    let mut hash = Sha256::new();
    hash.update(b"conduit.body/local-assigned-fragments@1");
    hash
}
pub(super) fn local_fragment_scope(
    scope: PreparedBodyFragment<'_>,
) -> Result<(u16, [u8; 32]), BodyFragmentDocumentRefusal> {
    let selected = scope.fragment();
    let plan = scope.plan();
    let plans = plan.plots.iter().map(|p| &p.plan).chain(
        plan.mask_topologies
            .iter()
            .flat_map(|t| t.chains.iter().map(|c| &c.plan)),
    );
    let mut ids: Vec<_> = plans
        .flat_map(|p| &p.fragments)
        .filter(|f| {
            f.host_id == selected.host_id
                && f.boot_id == selected.boot_id
                && f.offer_generation == selected.offer_generation
        })
        .map(|f| f.fragment_id.as_str())
        .collect();
    ids.sort_unstable();
    fragment_set_commitment(&ids)
}

pub(super) fn fragment_set_commitment(
    ids: &[&str],
) -> Result<(u16, [u8; 32]), BodyFragmentDocumentRefusal> {
    if ids.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(BodyFragmentDocumentRefusal::Metadata);
    }
    let count = u16::try_from(ids.len()).map_err(|_| BodyFragmentDocumentRefusal::Bounds)?;
    let mut hash = begin();
    for id in ids {
        update(&mut hash, id);
    }
    Ok((count, hash.finalize().into()))
}

#[cfg(feature = "authenticated-admission")]
pub(super) fn matches_local_fragment_set(fragments: &[&DelegatedBodyFragment<'_>]) -> bool {
    matches_local_metadata(fragments.len(), |i| *fragments[i].metadata())
}

pub(super) fn matches_local_metadata<'a>(
    count: usize,
    at: impl Fn(usize) -> BodyFragmentApprovalMetadata<'a>,
) -> bool {
    if count == 0 {
        return false;
    }
    let first = at(0);
    if count != usize::from(first.local_fragment_count) {
        return false;
    }
    let scope = |m: BodyFragmentApprovalMetadata<'a>| {
        (
            m.body,
            m.wake,
            m.body_plan,
            m.body_play,
            m.play_sequence,
            m.workload_revision,
            m.host,
            m.boot,
            m.offer_generation,
        )
    };
    if (0..count).any(|i| {
        let m = at(i);
        scope(m) != scope(first)
            || m.local_fragment_count != first.local_fragment_count
            || m.local_fragment_digest != first.local_fragment_digest
    }) {
        return false;
    }
    let mut hash = begin();
    let mut previous = None;
    for _ in 0..count {
        let Some(next) = (0..count)
            .map(|i| at(i).fragment)
            .filter(|id| previous.is_none_or(|last| *id > last))
            .min()
        else {
            return false;
        };
        update(&mut hash, next);
        previous = Some(next);
    }
    let digest: [u8; 32] = hash.finalize().into();
    first.local_fragment_digest == digest
}
