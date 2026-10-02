//! Atomic checked workset edits; admission failure never mutates the live session.
use super::{debug, BodyLifecycleSession, BootId, HostId, Refusal, ResidentPlot};
use serde::Deserialize;

#[derive(Deserialize)]
pub(super) enum WorksetEdit {
    Install,
    Remove,
    Replace { previous: ResidentPlot },
}

pub(super) fn change(
    current: &BodyLifecycleSession,
    host: &HostId,
    boot: &BootId,
    expected_revision: u64,
    plot: ResidentPlot,
    source: &str,
    edit: WorksetEdit,
) -> Result<BodyLifecycleSession, Refusal> {
    crate::plot_runner::workspace::require_empty()?;
    if !matches!(edit, WorksetEdit::Remove) {
        crate::creche::require_workspace_plot(source, &plot)?;
    }
    let mut candidate = current.clone();
    match edit {
        WorksetEdit::Install => {
            candidate
                .admit_plot(expected_revision, plot.clone(), host, boot)
                .map_err(debug)?;
            candidate.select_plot(&plot).map_err(debug)?;
        }
        WorksetEdit::Remove => candidate
            .remove_plot(expected_revision, &plot, host, boot)
            .map_err(debug)?,
        WorksetEdit::Replace { previous } => {
            candidate
                .remove_plot(expected_revision, &previous, host, boot)
                .map_err(debug)?;
            let revision = candidate.evidence().body.workload_revision;
            candidate
                .admit_plot(revision, plot.clone(), host, boot)
                .map_err(debug)?;
            candidate.select_plot(&plot).map_err(debug)?;
        }
    }
    Ok(candidate)
}

#[cfg(test)]
#[path = "workspace_workset_tests.rs"]
mod tests;
