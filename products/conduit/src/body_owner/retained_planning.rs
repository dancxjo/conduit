//! Retained Source custody through complete local or explicitly Line-bound planning.
use super::{continuing::ImageIngress, Owner};
use crate::body_owner::ForeLineSelection;
use conduit_body::ResidentPlot;
use std::{collections::BTreeMap, path::Path};
impl Owner {
    /// Plan the complete retained workset for the owner's existing finite
    /// receipt adapter. Output budgets confer no input or Host Call authority.
    pub(crate) fn plan_retained(&mut self, root: &Path) -> Result<(), String> {
        self.plan_retained_with_fore_lines(root, &BTreeMap::new())
    }

    pub(crate) fn plan_retained_with_ingress(
        &mut self,
        root: &Path,
        ingress: Option<&ImageIngress>,
    ) -> Result<(), String> {
        self.plan_retained_with_ingress_and_lines(root, ingress, &BTreeMap::new())
    }

    /// Line facts are supplied by the owning native provider, never a Plot or
    /// incoming preparation document. They remain planning evidence only.
    pub(crate) fn plan_retained_with_fore_lines(
        &mut self,
        root: &Path,
        lines: &BTreeMap<ResidentPlot, ForeLineSelection<'_>>,
    ) -> Result<(), String> {
        self.plan_retained_with_ingress_and_lines(root, None, lines)
    }

    fn plan_retained_with_ingress_and_lines(
        &mut self,
        root: &Path,
        ingress: Option<&ImageIngress>,
        lines: &BTreeMap<ResidentPlot, ForeLineSelection<'_>>,
    ) -> Result<(), String> {
        self.ensure_execution_retained(root)?;
        if let Some(ingress) = ingress {
            self.host.verify_selected_image_fore_input(&ingress.input)?;
        }
        #[cfg(target_os = "linux")]
        if let Some(retained) = self.selected_file_workset.take() {
            let result = (|| {
                retained.verify(&self.host, &self.session.evidence().body.workset)?;
                let boundaries = super::continuing::boundaries_for_plots_with_local(
                    retained.checked.plots.iter(),
                    ingress,
                    self.local_fore_adapters.as_ref(),
                )?;
                let borrowed = retained
                    .checked
                    .plots
                    .iter()
                    .map(|plot| (&retained.checked.source, plot))
                    .collect::<Vec<_>>();
                self.plan_workset_with_source_boundaries_and_lines(&borrowed, &boundaries, lines)
            })();
            self.selected_file_workset = Some(retained);
            return result;
        }
        let sources = super::super::retained_source::workset(root)?
            .ok_or("retained Source workset is unavailable for Body Plan")?;
        let boundaries = super::continuing::boundaries_for_plots_with_local(
            sources.iter().map(|source| &source.plot),
            ingress,
            self.local_fore_adapters.as_ref(),
        )?;
        let borrowed: Vec<_> = sources
            .iter()
            .map(|source| (&source.source, &source.plot))
            .collect();
        self.plan_workset_with_source_boundaries_and_lines(&borrowed, &boundaries, lines)
    }
}
