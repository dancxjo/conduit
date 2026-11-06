//! One trusted Root effect after an owner action's policy/return is authorized.
//! Incoming proposal Info, copied IDs or matching Source do not grant this call.
use super::{debug, state, Owner};
use conduit_body::{BodyBiographyEvidence, BodyState, BodyWorkset, ResidentPlot};
use std::path::Path;

impl Owner {
    pub(super) fn require_workset_recovered(&self) -> Result<(), String> {
        if self.workset_recovery_required {
            Err("owner-workset-recovery-required: resume the retained journal before further effects".into())
        } else {
            Ok(())
        }
    }

    /// Consume original finite-policy Source custody. Copied IDs and public
    /// proposal Info cannot construct this approval or reach the shared effect.
    pub(super) fn commit_authorized_clock_revision(
        &mut self,
        root: &Path,
        authorization: super::clock_interval::AuthorizedClockRevision,
    ) -> Result<(), String> {
        let (expected, (host, boot, generation), source, checked) = authorization.into_parts();
        let advertised = self.host.advertisement();
        if advertised.host_id != host
            || advertised.boot_id != boot
            || advertised.offer_generation != generation
        {
            return Err("workset approval belongs to another Host/Boot or offer generation".into());
        }
        self.commit_reviewed_workset_revision(root, &expected, &[(&source, &checked)])
    }

    /// Trusted reference-fixture entrance; borrowed metadata is not production
    /// adoption authority. Installed actions must consume policy-owned approval.
    #[cfg(test)]
    pub(super) fn commit_admitted_workset_revision(
        &mut self,
        root: &Path,
        expected: &BodyBiographyEvidence,
        sources: &[(
            &crate::plot_source::CanonicalSource,
            &conduit_plot::ExpandedAuthoringPlot,
        )],
    ) -> Result<(), String> {
        self.commit_reviewed_workset_revision(root, expected, sources)
    }

    /// Only an already-authorized Root action may invoke this continuation. Its
    /// semantic policy must constrain these actual owned Sources (as the clock
    /// action does), not merely approve a caller-supplied digest or description.
    /// No public command or ordinary Plot receives this privilege here.
    fn commit_reviewed_workset_revision(
        &mut self,
        root: &Path,
        expected: &BodyBiographyEvidence,
        sources: &[(
            &crate::plot_source::CanonicalSource,
            &conduit_plot::ExpandedAuthoringPlot,
        )],
    ) -> Result<(), String> {
        self.ensure_execution_retained(root)?;
        if self.host.is_playing()
            || self.session.realization().is_some()
            || self.session.evidence().body.state != BodyState::Lulled
        {
            return Err("workset revision requires a Lulled owner and a retired Play".into());
        }
        if self.session.evidence() != expected {
            return Err("workset revision differs from the current owner basis".into());
        }
        if sources.len() > conduit_body::MAX_BODY_PLOTS {
            return Err("workset revision exceeds Body capacity".into());
        }
        let desired = BodyWorkset::from_plots(sources.iter().map(|(_, plot)| {
            ResidentPlot::new(
                plot.expanded.source_document_id.clone(),
                plot.expanded.checked_plot_id.clone(),
            )
        }))
        .map_err(debug)?;
        if desired == expected.body.workset {
            return Err("workset revision has no change".into());
        }
        // The same ordinary planner independently rechecks every exact Source
        // and current offer. Preparation is not reservation or permission to run.
        self.prepare_workset_partitions_for(&desired, sources)?;
        let ad = self.host.advertisement();
        let mut staged = self.session.clone();
        for plot in expected
            .body
            .workset
            .plots()
            .iter()
            .filter(|plot| !desired.contains(plot))
        {
            staged
                .remove_plot(
                    staged.evidence().body.workload_revision,
                    plot,
                    &ad.host_id,
                    &ad.boot_id,
                )
                .map_err(debug)?;
        }
        for plot in desired
            .plots()
            .iter()
            .filter(|plot| !expected.body.workset.contains(plot))
        {
            staged
                .admit_plot(
                    staged.evidence().body.workload_revision,
                    plot.clone(),
                    &ad.host_id,
                    &ad.boot_id,
                )
                .map_err(debug)?;
        }
        let foreground = self
            .resident
            .as_ref()
            .filter(|plot| desired.contains(plot))
            .cloned()
            .or_else(|| desired.plots().first().cloned());
        let name = foreground
            .as_ref()
            .and_then(|resident| {
                sources.iter().find(|(_, plot)| {
                    plot.expanded.source_document_id == resident.source_document_id
                        && plot.expanded.checked_plot_id == resident.checked_plot_id
                })
            })
            .map(|(_, plot)| plot.expanded.name.clone());
        let retained = state::WorksetSources::checked(
            staged.evidence(),
            sources
                .iter()
                .map(|(source, plot)| (source.source.clone(), plot.expanded.name.clone()))
                .collect(),
        )?;
        if let Err(error) = state::retain_workset_revision(
            root,
            &mut staged,
            expected,
            &retained,
            self.last_execution.as_ref(),
            self.admissions.as_ref(),
        ) {
            // A published WAL may have partly committed. Do not overwrite it
            // with the old session or start another Play; ordinary restart
            // recovery completes its exact intent before a fresh owner resumes.
            self.workset_recovery_required = root
                .join("body/owner-transaction.json")
                .try_exists()
                .unwrap_or(true)
                || state::load(root)
                    .map(|current| current.as_ref() != Some(expected))
                    .unwrap_or(true);
            return Err(error);
        }
        self.session = staged;
        self.resident = foreground;
        self.resident_name = name;
        self.clock_interval_ms = None;
        Ok(())
    }
}

#[cfg(test)]
#[path = "workset_revision_tests.rs"]
mod tests;
