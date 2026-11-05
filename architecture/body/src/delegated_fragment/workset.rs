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
    if ids.windows(2).any(|pair| pair[0] == pair[1]) {
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
    let Some(first) = fragments.first().map(|f| f.metadata()) else {
        return false;
    };
    if fragments.len() != usize::from(first.local_fragment_count) {
        return false;
    }
    let mut hash = begin();
    let mut previous = None;
    for _ in 0..fragments.len() {
        // Canonical ordering with no pointer-vector or ID copies at the receiver.
        let next = fragments
            .iter()
            .map(|f| f.metadata().fragment)
            .filter(|id| previous.is_none_or(|last| *id > last))
            .min();
        let Some(next) = next else {
            return false;
        }; // Duplicate fragment.
        update(&mut hash, next);
        previous = Some(next);
    }
    let digest: [u8; 32] = hash.finalize().into();
    fragments.iter().all(|f| {
        let m = f.metadata();
        m.local_fragment_count == first.local_fragment_count && m.local_fragment_digest == digest
    })
}
