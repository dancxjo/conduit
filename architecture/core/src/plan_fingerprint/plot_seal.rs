//! Existing Plot Plan identity encoding for primitive, activation-free Plans.
//! Pins and computed digests establish no current placement or Start authority.
use super::*;

#[derive(Clone, Copy, Debug)]
pub struct PlainPlotPlanSeal<'a> {
    pub source_document_id: &'a str,
    pub checked_plot_id: &'a str,
    pub expanded_plot_id: &'a str,
}
#[derive(Clone, Copy, Debug)]
pub struct PlotFragmentSeal<'a> {
    pub host_id: &'a str,
    pub fragment_id: &'a str,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlotPlanSealRefusal {
    Unsupported,
    Commitments,
}
impl<'a> PlainPlotPlanSeal<'a> {
    /// Builder-side eligibility, not a replacement for `verify_plan`.
    pub fn capture(plan: &'a crate::Plan) -> Result<Self, PlotPlanSealRefusal> {
        if !plan.realization_backs.is_empty()
            || !plan.activations.is_empty()
            || !plan.activation_preparations.is_empty()
        {
            return Err(PlotPlanSealRefusal::Unsupported);
        }
        Ok(Self {
            source_document_id: plan.source_document_id.as_str(),
            checked_plot_id: plan.checked_plot_id.as_str(),
            expanded_plot_id: plan.expanded_plot_id.as_str(),
        })
    }
    /// Hash the exact existing empty-tail Plan format with fresh fragment
    /// commitments. Input order must be the original Core commitment order.
    /// Finite codec bounds are not admission or approval of these identities.
    pub fn digest(
        self,
        fragments: &[PlotFragmentSeal<'_>],
    ) -> Result<[u8; 32], PlotPlanSealRefusal> {
        let valid_text = |s: &str| !s.is_empty() && u32::try_from(s.len()).is_ok();
        if !valid_text(self.source_document_id)
            || !valid_text(self.checked_plot_id)
            || !valid_text(self.expanded_plot_id)
        {
            return Err(PlotPlanSealRefusal::Commitments);
        }
        if fragments.is_empty()
            || fragments.len() > usize::from(u16::MAX)
            || fragments
                .iter()
                .any(|f| !valid_text(f.host_id) || !valid_text(f.fragment_id))
            || fragments
                .windows(2)
                .any(|p| (p[0].host_id, p[0].fragment_id) >= (p[1].host_id, p[1].fragment_id))
        {
            return Err(PlotPlanSealRefusal::Commitments);
        }
        let mut hash = CanonicalHash::new();
        push_identity(&mut hash, self);
        push_commitments(&mut hash, fragments.iter().copied());
        Ok(hash.finish())
    }
}
pub(super) fn push_identity(
    hash: &mut impl crate::canonical_bytes::CanonicalSink,
    identity: PlainPlotPlanSeal<'_>,
) {
    push_string(hash, identity.source_document_id);
    push_string(hash, identity.checked_plot_id);
    push_string(hash, identity.expanded_plot_id);
}
pub(super) fn push_commitments<'a>(
    hash: &mut impl crate::canonical_bytes::CanonicalSink,
    fragments: impl ExactSizeIterator<Item = PlotFragmentSeal<'a>>,
) {
    push_u32(hash, fragments.len() as u32);
    for fragment in fragments {
        push_string(hash, fragment.host_id);
        push_string(hash, fragment.fragment_id);
    }
}
