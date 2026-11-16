//! Reselect planning boundaries from original retained File custody only.
use super::Owner;
impl Owner {
    pub(crate) fn restore_selected_file_planning(
        &mut self,
        custody: crate::durable_host::source_revision::RetainedSourcePreparation,
    ) -> Result<(), String> {
        self.require_workset_recovered()?;
        if self.host.is_playing() || self.session.realization().is_some() {
            return Err("retained File planning restore requires an idle owner".into());
        }
        let retained = custody.restore(&self.host, self.session.evidence())?;
        self.selected_file_workset = Some(retained);
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
