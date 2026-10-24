//! Host-owned verified capsules, fenced by the exact checked expansion.
use conduit_core::{PlanFragment, PlannedGear};
use conduit_plot::{
    CheckedGear, ExpandedCanonicalPlot, LawExpressionContext, LawExpressionMaterial,
    VerifiedLawExpression,
};
use std::sync::Arc;

#[derive(Default)]
pub(crate) struct LawPreparationCache {
    entries: Vec<Arc<Entry>>,
}
pub(crate) struct Entry {
    source: conduit_core::SourceDocumentId,
    checked: conduit_core::CheckedPlotId,
    expanded: conduit_core::ExpandedPlotId,
    gear: CheckedGear,
    verified: VerifiedLawExpression,
}
impl Entry {
    pub(crate) fn prepare(
        plot: &ExpandedCanonicalPlot,
        gear: &CheckedGear,
        encoded: &str,
        context: &LawExpressionContext,
    ) -> Result<Arc<Self>, String> {
        let material = LawExpressionMaterial::from_canonical_hex(encoded)
            .map_err(|why| format!("law capsule encoding: {why:?}"))?;
        let bytes = material
            .canonical_bytes()
            .map_err(|why| format!("law capsule: {why:?}"))?;
        let verified = VerifiedLawExpression::verify_with_context(&bytes, context)
            .map_err(|why| format!("law capsule Source custody: {why:?}"))?;
        // Full finite evaluator admission precedes the transactional offer commit.
        verified
            .prepare_evaluator()
            .map_err(|why| format!("law evaluator: {why:?}"))?;
        Ok(Arc::new(Self {
            source: plot.source_document_id.clone(),
            checked: plot.checked_plot_id.clone(),
            expanded: plot.expanded_plot_id.clone(),
            gear: gear.clone(),
            verified,
        }))
    }
    fn matches_source(&self, plot: &ExpandedCanonicalPlot, gear: &CheckedGear) -> bool {
        self.source == plot.source_document_id
            && self.checked == plot.checked_plot_id
            && self.expanded == plot.expanded_plot_id
            && self.gear == *gear
    }
    pub(crate) fn verified(&self) -> &VerifiedLawExpression {
        &self.verified
    }
}
impl LawPreparationCache {
    pub(crate) fn source(
        &self,
        plot: &ExpandedCanonicalPlot,
        gear: &CheckedGear,
    ) -> Option<Arc<Entry>> {
        self.entries
            .iter()
            .find(|entry| entry.matches_source(plot, gear))
            .cloned()
    }
    pub(crate) fn matches(&self, entries: &[Arc<Entry>]) -> bool {
        self.entries.len() == entries.len()
            && self.entries.iter().zip(entries).all(|(left, right)| {
                left.source == right.source
                    && left.checked == right.checked
                    && left.expanded == right.expanded
                    && left.gear == right.gear
            })
    }
    pub(crate) fn replace(&mut self, entries: Vec<Arc<Entry>>) {
        self.entries = entries;
    }
    pub(crate) fn planned(
        &self,
        fragment: &PlanFragment,
        placement: &PlannedGear,
    ) -> Result<&VerifiedLawExpression, String> {
        let entry = self
            .entries
            .iter()
            .find(|entry| {
                entry.source == fragment.source_document_id
                    && entry.checked == fragment.checked_plot_id
                    && entry.expanded == fragment.expanded_plot_id
                    && entry.gear.gear_id == placement.gear_id
                    && entry.gear.kind_id == placement.kind_id
                    && entry.gear.kind_contract_revision == placement.kind_contract_revision
                    && entry.gear.configuration == placement.configuration
                    && entry.gear.inputs == placement.inputs
                    && entry.gear.outputs == placement.outputs
            })
            .ok_or("planned law expression has no exact retained Source custody")?;
        Ok(&entry.verified)
    }
}
