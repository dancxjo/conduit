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
    workspace_library, WorkspaceIssuerCustody,
};
#[cfg(feature = "plot-runner")]
pub(crate) fn check_stem_configuration(
    selection: BrowserConfigurationSelection,
) -> Result<conduit_host_make::CheckedHostConfiguration, String> {
    browser_configuration::review(selection).map(|(_, checked, _)| checked)
}

#[cfg(feature = "plot-runner")]
pub(crate) fn stem_configuration_review(
    configuration: &conduit_host_make::CheckedHostConfiguration,
) -> Result<serde_json::Value, String> {
    let mut implementations = std::collections::BTreeSet::new();
    for base in &configuration.configuration().bases {
        if let Some(id) = &base.implementation {
            implementations.insert(id.clone());
        }
        implementations.extend(base.implementations.iter().cloned());
    }
    let (review, checked, _) = browser_configuration::review(BrowserConfigurationSelection {
        catalog_generation: 1,
        implementations: implementations.into_iter().collect(),
    })?;
    if checked.configuration_id() != configuration.configuration_id() {
        return Err("Stem configuration review changed its canonical identity".into());
    }
    serde_json::to_value(review).map_err(|error| error.to_string())
}

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
