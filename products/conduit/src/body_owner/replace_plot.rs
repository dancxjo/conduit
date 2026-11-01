//! Explicit replacement of one resident Plot in the SAME lulled Body.
use super::*;
use conduit_body::{BodyState, ResidentPlot};
use std::path::Path;
impl Owner {
    pub(crate) fn replace_plot(
        &mut self,
        root: &Path,
        source: &str,
        expected_revision: u64,
        expected_checked: &str,
    ) -> Result<serde_json::Value, String> {
        if source.len() as u64 > super::super::MAXIMUM_SOURCE {
            return Err("replacement Source exceeds its finite bound".into());
        }
        if self.host.is_playing()
            || self.pending_browser.is_some()
            || self.session.realization().is_some()
            || self.session.evidence().body.state != BodyState::Lulled
        {
            return Err(
                "Plot replacement requires a lulled Body and no current proposal or Play".into(),
            );
        }
        // Journal replay may have completed a prior unknown outcome. Never
        // publish a new mutation from the pre-recovery in-memory session.
        let committed = state::load(root)?.ok_or("Body has no committed biography")?;
        if &committed != self.session.evidence() {
            return Err(
                "Body owner is stale after transaction recovery; restart before replacement".into(),
            );
        }
        let evidence = self.session.evidence();
        let [old] = evidence.body.workset.plots() else {
            return Err("Plot replacement currently supports exactly one resident".into());
        };
        if evidence.body.workload_revision != expected_revision
            || old.checked_plot_id.as_str() != expected_checked
            || self.resident.as_ref() != Some(old)
        {
            return Err(
                "Plot replacement expected workload revision or checked Plot is stale".into(),
            );
        }
        let installation = super::super::super::read_installation(&root.join("installation.json"))?;
        let host = self.host.advertisement();
        if installation.host_id != host.host_id.as_str()
            || installation.joined_body_state.is_some()
            || installation
                .body_state
                .as_ref()
                .is_none_or(|binding| binding.body_id != evidence.body_id.as_str())
        {
            return Err("Plot replacement belongs to another installed Host or Body".into());
        }
        let retained =
            super::super::checked_retained_source(root)?.ok_or("resident Source is unavailable")?;
        if ResidentPlot::new(
            retained.expanded.source_document_id,
            retained.expanded.checked_plot_id,
        ) != *old
        {
            return Err("retained Source differs from exact resident Plot".into());
        }
        let checked = crate::plot_source::parse(source)?.expand_entry_for_authoring()?;
        let next = ResidentPlot::new(
            checked.expanded.source_document_id.clone(),
            checked.expanded.checked_plot_id.clone(),
        );
        if &next == old {
            return Err("replacement is already the resident Plot".into());
        }
        let old = old.clone();
        let mut staged = self.session.clone();
        staged
            .remove_plot(expected_revision, &old, &host.host_id, &host.boot_id)
            .map_err(debug)?;
        staged
            .admit_plot(
                expected_revision
                    .checked_add(1)
                    .ok_or("workload revision exhausted")?,
                next.clone(),
                &host.host_id,
                &host.boot_id,
            )
            .map_err(debug)?;
        // No feasibility claim or implicit Start. Exact native preparation and
        // grants remain the responsibility of the later ordinary start command.
        state::retain_session(
            root,
            &mut staged,
            self.last_execution.as_ref(),
            self.admissions.as_ref(),
            Some(source.as_bytes()),
            None,
        ).map_err(|error| format!("plot-replacement-outcome-unknown: {error}; restart the owner and inspect recovered retained state before retrying"))?;
        self.session = staged;
        self.resident = Some(next.clone());
        self.resident_name = Some(checked.expanded.name);
        self.clock_interval_ms = clock_interval::recognized_interval(&next);
        self.todo_live = None;
        self.todo_verified = None;
        self.thermostat_live = None;
        self.notebook = None;
        self.face_wardrobe = None;
        self.attached_terminal_route = None;
        self.direct_spoken_route = None;
        self.direct_spoken_template = None;
        self.llm_spoken_route = None;
        self.pending_native_mask = None;
        Ok(
            serde_json::json!({"body_id":self.session.evidence().body_id,"host_id":host.host_id,"boot_id":host.boot_id,
            "previous_plot":old,"resident_plot":next,"workload_revision":self.session.evidence().body.workload_revision,
            "custody":"retained-source-and-canonical-workset-not-started"}),
        )
    }
}
