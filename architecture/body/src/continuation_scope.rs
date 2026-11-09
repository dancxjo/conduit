//! Owner-authorized local replacement inside an unchanged full Wake workset.
//! This is policy commitment data, not global Playing or a new lifecycle object.
use crate::{
    body_plan_commitment_digest, BodyPlotSeal, MAX_BODY_PLOTS, MAX_BODY_PLOT_IDENTITY_BYTES,
    MAX_LIFECYCLE_ID_BYTES,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContinuationAuthority {
    LocalWorksetReplacement,
}
#[derive(Clone, Copy, Debug)]
pub struct ContinuationPlotPin<'a> {
    pub source_document: &'a str,
    pub checked_plot: &'a str,
    /// The exact originally checked Plot Plan, retained unchanged for nonlocal work.
    pub plan_id: &'a str,
    pub local: bool,
}
pub struct ContinuationFullScope<'a> {
    pub wake_id: &'a str,
    pub wake_sequence: u64,
    pub workload_revision: u64,
    pub base_body_plan: &'a str,
    /// Existing ResidentPlot order (SourceDocumentId, then CheckedPlotId).
    /// Initial profile has no separate Mask topology or Body-time tail.
    pub plots: &'a [ContinuationPlotPin<'a>],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContinuationFullScopeRefusal {
    Bounds,
    Order,
    Wake,
    BasePlan,
}
/// Commit the retained FULL workset and exactly which subset may be freshly
/// realized locally. No remote commitment may be changed by this permission.
pub fn continuation_scope_digest(
    body: &str,
    scope: &ContinuationFullScope<'_>,
) -> Result<[u8; 32], ContinuationFullScopeRefusal> {
    use ContinuationFullScopeRefusal as E;
    let valid = |s: &str| !s.is_empty() && s.len() <= MAX_LIFECYCLE_ID_BYTES;
    if !valid(body)
        || !valid(scope.wake_id)
        || scope.plots.is_empty()
        || scope.plots.len() > MAX_BODY_PLOTS
        || !scope.plots.iter().any(|p| p.local)
        || scope
            .plots
            .iter()
            .any(|p| !valid(p.source_document) || !valid(p.checked_plot) || !valid(p.plan_id))
        || scope
            .plots
            .iter()
            .try_fold(0usize, |n, p| {
                n.checked_add(p.source_document.len())?
                    .checked_add(p.checked_plot.len())
            })
            .is_none_or(|n| n > MAX_BODY_PLOT_IDENTITY_BYTES)
    {
        return Err(E::Bounds);
    }
    if scope.plots.windows(2).any(|p| {
        (p[0].source_document, p[0].checked_plot) >= (p[1].source_document, p[1].checked_plot)
    }) {
        return Err(E::Order);
    }
    if scope.wake_id.as_bytes()
        != crate::identity::bound_identity_bytes("wake", &[body], scope.wake_sequence)
    {
        return Err(E::Wake);
    }
    let empty = BodyPlotSeal {
        source_document_id: "",
        checked_plot_id: "",
        plan_id: "",
    };
    let mut pins = [empty; MAX_BODY_PLOTS];
    for (out, p) in pins.iter_mut().zip(scope.plots) {
        *out = BodyPlotSeal {
            source_document_id: p.source_document,
            checked_plot_id: p.checked_plot,
            plan_id: p.plan_id,
        };
    }
    let digest = body_plan_commitment_digest(
        body,
        scope.wake_id,
        scope.workload_revision,
        &pins[..scope.plots.len()],
        &[],
        None,
    )
    .map_err(|_| E::BasePlan)?;
    if !digest_matches(scope.base_body_plan, "body-plan/sha256:", digest) {
        return Err(E::BasePlan);
    }
    let mut hash = Sha256::new();
    hash.update(b"conduit.body/local-continuation-scope@1\0");
    for s in [body, scope.wake_id, scope.base_body_plan] {
        push(&mut hash, s)
    }
    hash.update(scope.wake_sequence.to_le_bytes());
    hash.update(scope.workload_revision.to_le_bytes());
    hash.update((scope.plots.len() as u16).to_le_bytes());
    for p in scope.plots {
        for s in [p.source_document, p.checked_plot, p.plan_id] {
            push(&mut hash, s)
        }
        hash.update([u8::from(p.local)]);
    }
    Ok(hash.finalize().into())
}
fn push(hash: &mut Sha256, s: &str) {
    hash.update((s.len() as u32).to_le_bytes());
    hash.update(s.as_bytes())
}
pub(crate) fn digest_matches(id: &str, prefix: &str, digest: [u8; 32]) -> bool {
    let Some(hex) = id.strip_prefix(prefix) else {
        return false;
    };
    if hex.len() != 64 {
        return false;
    }
    const HEX: &[u8; 16] = b"0123456789abcdef";
    digest
        .iter()
        .zip(hex.as_bytes().as_chunks::<2>().0)
        .all(|(b, p)| p[0] == HEX[usize::from(b >> 4)] && p[1] == HEX[usize::from(b & 15)])
}
