//! Explicit, bounded Body birth and first-Host admission for the Crèche.

mod abi;
mod birth_draft;
mod browser_configuration;
mod catalog_preparation;
pub(crate) use browser_configuration::BrowserConfigurationSelection;
mod durable;
mod graduation;
mod initial_plots;
mod protocol;
mod review;
mod session;
#[cfg(feature = "plot-runner")]
mod workspace;
#[cfg(feature = "plot-runner")]
pub(crate) use initial_plots::inventory_application_subjects;
#[cfg(feature = "plot-runner")]
pub(crate) use workspace::{
    handoff_workspace, plan_workspace_plots, require_workspace_plot, workspace_evidence,
    workspace_library,
};
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct JoinedLineObservation {
    pub host_id: conduit_core::HostId,
    pub boot_id: conduit_core::BootId,
    pub carrier: String,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct PlanningAuthority {
    pub browser_audio: bool,
}
mod spore;
pub(crate) use spore::{prepare_workspace_browser, prepare_workspace_physical};
mod spore_target;

/// Checked Source inventory reused by whole-Body publication, never transport labels.
pub(crate) fn body_face_inventory(
    source: &str,
) -> Result<Vec<conduit_body_make::ReviewedBodyPlot>, String> {
    let inventory = initial_plots::reviewed_inventory(source)?;
    let reviewed = inventory
        .plots
        .into_iter()
        .map(|plot| conduit_body_make::ReviewedBodyPlot {
            label: plot.title,
            source_document_id: plot.source_document_id,
            checked_plot_id: plot.checked_plot_id,
        })
        .collect::<Vec<_>>();
    conduit_body_make::validate_reviewed_body_plots(&reviewed)?;
    Ok(reviewed)
}

#[cfg(test)]
mod button_workset_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod typed_workset_tests;

#[cfg(all(test, feature = "plot-runner"))]
mod pure_expression_workset_tests;
