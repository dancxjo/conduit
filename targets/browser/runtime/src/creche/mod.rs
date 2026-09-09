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
pub(crate) use workspace::{
    handoff_workspace, plan_workspace_forms, require_workspace_form, workspace_evidence,
    workspace_library,
};
mod spore;
mod spore_target;

#[cfg(test)]
mod button_workset_tests;
#[cfg(test)]
mod tests;
