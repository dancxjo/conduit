//! Explicit, bounded Body birth and first-Host admission for the Crèche.

mod abi;
mod birth_draft;
mod browser_configuration;
mod durable;
mod graduation;
mod graduation_presentation;
mod initial_forms;
mod protocol;
mod review;
mod session;
#[cfg(feature = "form-runner")]
mod workspace;
#[cfg(feature = "form-runner")]
pub(crate) use initial_forms::{expanded_inventory_form, inventory_form_title};
#[cfg(feature = "form-runner")]
pub(crate) use workspace::{
    handoff_workspace, plan_workspace_forms, require_workspace_form, workspace_evidence,
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
mod spore_target;

#[cfg(test)]
mod button_workset_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod typed_workset_tests;
