//! Extend the retained Body with the ordinary Crèche Source and plan all members.
use super::{debug, retained_sources, Owner};
use conduit_body::{BodyState, ResidentPlot};
use std::path::Path;
const SOURCE: &str = include_str!("../../../../plots/creche/main.conduit");
fn shipped() -> Result<ResidentPlot, String> {
    let checked = crate::plot_source::parse(SOURCE)?.expand_entry_for_authoring()?;
    Ok(ResidentPlot::new(
        checked.expanded.source_document_id,
        checked.expanded.checked_plot_id,
    ))
}
impl Owner {
    #[cfg(feature = "native-creche-sdl")]
    pub(crate) fn workload_revision(&self) -> u64 {
        self.session.evidence().body.workload_revision
    }
    /// A repeated encounter reuses its member and Source; it never repeats birth
    /// or rewrites the primary Source, selected resident or membership.
    #[allow(dead_code)] // Native application composition supplies the owner lock.
    pub(crate) fn admit_shipped_creche(
        &mut self,
        root: &Path,
        expected_revision: u64,
    ) -> Result<ResidentPlot, String> {
        if self.host.is_playing()
            || self.session.realization().is_some()
            || self.session.evidence().body.state != BodyState::Lulled
        {
            return Err("Crèche Source admission requires the retained idle Lulled Body".into());
        }
        if self.session.evidence().body.workload_revision != expected_revision {
            return Err("stale Body workload revision".into());
        }
        self.ensure_execution_retained(root)?;
        self.persist(root)?;
        let resident = shipped()?;
        let mut next = self.session.clone();
        if !next.evidence().body.workset.contains(&resident) {
            let host = self.host.advertisement();
            next.admit_plot(
                expected_revision,
                resident.clone(),
                &host.host_id,
                &host.boot_id,
            )
            .map_err(debug)?;
        }
        retained_sources::retain(root, &resident, SOURCE)?;
        // Keep the actual accepted lifecycle even if persistence fails. The
        // common replay barrier must retain this exact session before any Play.
        self.session = next;
        self.persist(root)?;
        Ok(resident)
    }
    /// All current members are required. Unsupported activation/input owners
    /// refuse before proposal instead of replacing the workset with Crèche.
    #[allow(dead_code)]
    pub(crate) fn plan_creche_workset(&mut self, root: &Path) -> Result<(), String> {
        if self.host.is_playing() || self.session.realization().is_some() {
            return Err("Body already has a current proposal or Play".into());
        }
        self.ensure_execution_retained(root)?;
        let creche = shipped()?;
        if !self.session.evidence().body.workset.contains(&creche) {
            return Err("Crèche is not admitted to this Body".into());
        }
        let mut partitions = Vec::with_capacity(self.session.evidence().body.workset.len());
        for resident in self.session.evidence().body.workset.plots() {
            let (source, checked) = retained_sources::load(
                root,
                resident,
                self.resident.as_ref().map(|r| &r.source_document_id),
            )?;
            if !checked.expanded.activations.is_empty() {
                return Err(
                    "Crèche workset lacks an admitted owner for another Plot's activation".into(),
                );
            }
            let partition = self.plan_partition_with_source(
                &source,
                &checked,
                resident,
                self.host.advertisement(),
            )?;
            if resident != &creche
                && partition
                    .plan
                    .fragments
                    .iter()
                    .flat_map(|f| &f.fore_ports)
                    .any(|p| p.direction == conduit_core::PortDirection::Input)
            {
                return Err(
                    "Crèche workset lacks an admitted owner for another Plot's input".into(),
                );
            }
            partitions.push(partition);
        }
        let host = self.host.advertisement();
        self.session
            .propose(partitions, &host.host_id, &host.boot_id)
            .map_err(debug)?;
        self.persist(root)
    }
}
#[cfg(test)]
#[path = "creche_workset/tests.rs"]
mod tests;
