//! Exclusive opening of one canonical Form or authored-environment workspace.

use crate::{environment_resource, form_interaction, resource};
use patchbay_model::{AuthoredEnvironment, FormEditor, PatchbayGraph, PatchbayLayout};
use std::path::PathBuf;

pub(super) struct OpenedWorkspace {
    pub(super) form_editor: Option<FormEditor>,
    pub(super) environment: Option<AuthoredEnvironment>,
    pub(super) environment_path: Option<PathBuf>,
    pub(super) graphical_form: Option<PatchbayGraph>,
    pub(super) layout: PatchbayLayout,
}

pub(super) fn open_workspace(
    form_path: Option<PathBuf>,
    environment_path: Option<PathBuf>,
    allow_combined: bool,
) -> Result<OpenedWorkspace, String> {
    if form_path.is_some() && environment_path.is_some() && !allow_combined {
        return Err("--form and --environment are distinct workspaces".into());
    }
    let environment = environment_path
        .as_ref()
        .map(environment_resource::open_environment_resource)
        .transpose()?;
    let form_editor = form_path.map(resource::open_form_resource).transpose()?;
    let graphical_form = form_editor
        .as_ref()
        .map(form_interaction::graphical_form_for_editor)
        .transpose()?
        .flatten();
    let mut layout = form_editor
        .as_ref()
        .map(resource::open_layout_resource)
        .transpose()?
        .unwrap_or_default();
    if let Some(graph) = &graphical_form {
        layout.reconcile(graph);
    }
    Ok(OpenedWorkspace {
        form_editor,
        environment,
        environment_path,
        graphical_form,
        layout,
    })
}
