//! Checked Fore Line choices before the complete Body Wake is published.
//! Descriptive offers do not establish native possession or execution authority.
use super::{debug, Owner};
use conduit_body::{BodyPlotPlan, ResidentPlot};
use conduit_core::{BaseImplementationId, LineOffer};
use conduit_planner::{ConnectionQueueLimits, ExternalForeLineChoice, ForeBoundaryKey};
use std::collections::BTreeMap;

pub(crate) struct ForeLineSelection<'a> {
    pub choices: &'a [ExternalForeLineChoice],
    pub offers: &'a [LineOffer],
    pub allowed_bases: &'a [BaseImplementationId],
}
/// Cold facts retained by an independently owned native provider. Constructing
/// these rows grants no carrier possession, Ready sign or execution authority.
pub(crate) struct OwnedForeLineSelection {
    pub choices: Vec<ExternalForeLineChoice>,
    pub offers: Vec<LineOffer>,
    pub allowed_bases: Vec<BaseImplementationId>,
}
impl Owner {
    pub(super) fn plan_workset_with_source_boundaries_and_lines(
        &mut self,
        sources: &[(
            &crate::plot_source::CanonicalSource,
            &conduit_plot::ExpandedAuthoringPlot,
        )],
        boundaries: &BTreeMap<ResidentPlot, BTreeMap<ForeBoundaryKey, ConnectionQueueLimits>>,
        lines: &BTreeMap<ResidentPlot, ForeLineSelection<'_>>,
    ) -> Result<(), String> {
        let mut partitions = self.prepare_workset_partitions(sources, boundaries)?;
        self.select_fore_lines(&mut partitions, lines)?;
        let ad = self.host.advertisement();
        self.session
            .propose(partitions, &ad.host_id, &ad.boot_id)
            .map_err(debug)?;
        Ok(())
    }

    /// Call the native provider once with the complete checked baseline, before
    /// publishing Wake. This lets it derive ABI pins without generating another
    /// kernel or publishing a preliminary proposal. Refusal leaves Body state
    /// untouched; only the existing exact Line binder can change realization.
    pub(super) fn plan_workset_with_source_boundaries_and_provider<F>(
        &mut self,
        sources: &[(
            &crate::plot_source::CanonicalSource,
            &conduit_plot::ExpandedAuthoringPlot,
        )],
        boundaries: &BTreeMap<ResidentPlot, BTreeMap<ForeBoundaryKey, ConnectionQueueLimits>>,
        provider: &mut F,
    ) -> Result<(), String>
    where
        F: FnMut(&[BodyPlotPlan]) -> Result<BTreeMap<ResidentPlot, OwnedForeLineSelection>, String>,
    {
        let mut partitions = self.prepare_workset_partitions(sources, boundaries)?;
        let owned = provider(&partitions)?;
        let borrowed = owned
            .iter()
            .map(|(plot, selection)| {
                (
                    plot.clone(),
                    ForeLineSelection {
                        choices: &selection.choices,
                        offers: &selection.offers,
                        allowed_bases: &selection.allowed_bases,
                    },
                )
            })
            .collect();
        self.select_fore_lines(&mut partitions, &borrowed)?;
        let ad = self.host.advertisement();
        self.session
            .propose(partitions, &ad.host_id, &ad.boot_id)
            .map_err(debug)?;
        Ok(())
    }

    /// Recheck current Body member offers before sealing a Line into any Plan.
    /// The caller keeps its actual carrier; this method cannot create one.
    pub(super) fn select_fore_lines(
        &self,
        partitions: &mut [BodyPlotPlan],
        selections: &BTreeMap<ResidentPlot, ForeLineSelection<'_>>,
    ) -> Result<(), String> {
        if selections
            .keys()
            .any(|plot| !partitions.iter().any(|p| &p.plot == plot))
        {
            return Err("Fore Line choice is outside the exact retained workset".into());
        }
        let hosts = self.planning_hosts();
        for partition in partitions {
            let Some(selection) = selections.get(&partition.plot) else {
                continue;
            };
            for choice in selection.choices {
                for (host, boot) in [
                    (&choice.host_id, &choice.boot_id),
                    (&choice.peer_host_id, &choice.peer_boot_id),
                ] {
                    if hosts
                        .iter()
                        .filter(|ad| &ad.host_id == host && &ad.boot_id == boot)
                        .count()
                        != 1
                    {
                        return Err(
                            "Fore Line choice requires current Body member Host/Boot evidence"
                                .into(),
                        );
                    }
                }
            }
            partition.plan = conduit_planner::bind_external_fore_lines(
                &partition.plan,
                selection.choices,
                selection.offers,
                selection.allowed_bases,
            )
            .map_err(debug)?;
        }
        Ok(())
    }
}
