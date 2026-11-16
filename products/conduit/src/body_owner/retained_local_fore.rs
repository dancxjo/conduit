//! Reselect planning boundaries from original retained File custody only.
use super::Owner;
impl Owner {
    pub(crate) fn retain_selected_source_read_memory(
        &self,
    ) -> Result<conduit_std_host::body_execution::ImageMemoryRetention, String> {
        let original = self
            .selected_file_workset
            .as_ref()
            .ok_or("original retained File workset absent")?;
        original.retain_read_memory(&self.host)
    }

    pub(crate) fn execute_selected_source(
        &mut self,
        state: &std::path::Path,
    ) -> Result<
        Option<crate::durable_host::source_revision::source_execution::RetainedExecution>,
        String,
    > {
        use crate::durable_host::source_revision::source_execution;
        let original = self
            .selected_file_workset
            .as_ref()
            .ok_or("original retained File workset absent")?;
        let basis = source_execution::verify_retained(state, self, original)?;
        // The opaque witness is consumed synchronously while the ORIGINAL field
        // remains installed. No await, cached witness or disk-Source fallback.
        source_execution::claim_and_execute(state, self, basis)
    }
    /// Recheck the original retained File and caller guards before a local
    /// execution request can open any physical carrier.
    pub(crate) fn verify_selected_source_execution_basis(
        &self,
        basis: &crate::durable_host::source_revision::source_execution::VerifiedExecutionBasis,
    ) -> Result<(), String> {
        self.require_source_effects_unheld()?;
        self.require_workset_recovered()?;
        if self.host.is_playing() || self.session.realization().is_some() {
            return Err("fresh Source execution requires an idle owner".into());
        }
        let original = self
            .selected_file_workset
            .as_ref()
            .ok_or("original retained File workset absent")?;
        basis.verify_original(self, original)
    }
    pub(crate) fn restore_selected_file_planning(
        &mut self,
        custody: crate::durable_host::source_revision::RetainedSourcePreparation,
    ) -> Result<(), String> {
        self.require_workset_recovered()?;
        if self.host.is_playing() || self.session.realization().is_some() {
            return Err("retained File planning restore requires an idle owner".into());
        }
        let retained = custody.restore(&self.host, self.session.evidence())?;
        let mut readiness = super::wake_readiness::WakeIngressRequirements::default();
        for plot in &retained.checked.plots {
            readiness.observe(&self.session.evidence().body, plot)?;
        }
        self.selected_file_workset = Some(retained);
        self.wake_ingress_requirements = readiness;
        // Missing targets are ordinary planning unavailability, never failure of
        // the recovered Body or of a separately committed membership transition.
        self.refresh_retained_local_fore();
        Ok(())
    }
    pub(super) fn refresh_retained_local_fore(&mut self) {
        let Some(retained) = &self.selected_file_workset else {
            self.local_fore_adapters = None;
            return;
        };
        if !retained.requires_local_fore_selection() {
            self.local_fore_adapters = None;
            return;
        }
        if let Err(error) = retained.verify_planning(&self.host, self.session.evidence()) {
            self.local_fore_adapters = None;
            eprintln!("retained-local-fore-unavailable: {error:.256}");
            return;
        }
        if self.host.is_playing() || self.session.realization().is_some_and(|r| r.play.is_some()) {
            // Existing Play custody is untouched. Cached planning evidence must
            // nevertheless stop claiming a changed Root/member generation.
            if self.local_fore_adapters.as_ref().is_some_and(|set| {
                set.verify(self.host.advertisement(), &self.planning_hosts())
                    .is_err()
            }) {
                self.local_fore_adapters = None;
            }
            return;
        }
        self.local_fore_adapters = match retained.select_current_local_fore(
            &self.host,
            self.session.evidence(),
            &self.planning_hosts(),
        ) {
            Ok(selected) => Some(selected),
            Err(error) => {
                eprintln!("retained-local-fore-unavailable: {error:.256}");
                None
            }
        };
    }
    pub(super) fn require_retained_local_fore(&self) -> Result<(), String> {
        if let Some(retained) = &self.selected_file_workset {
            retained.verify_planning(&self.host, self.session.evidence())?;
        }
        if self
            .selected_file_workset
            .as_ref()
            .is_some_and(|retained| retained.requires_local_fore_selection())
        {
            self.local_fore_adapters
                .as_ref()
                .ok_or("retained-local-fore-unavailable")?
                .verify(self.host.advertisement(), &self.planning_hosts())?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "retained_local_fore_tests.rs"]
mod tests;
