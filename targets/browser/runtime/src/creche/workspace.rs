//! Crèche-to-workspace handoff and shared exact inventory planning.
use conduit_body::{BodyBiographyEvidence, BodyFormPlan};
use conduit_core::{BootId, HostId};

pub(crate) fn workspace_evidence() -> Result<BodyBiographyEvidence, String> {
    super::session::biography().ok_or_else(|| "Birth a Body before arriving".into())
}

pub(crate) fn handoff_workspace() {
    super::session::forget_local();
}

pub(crate) fn require_workspace_form(
    source: &str,
    form: &conduit_body::ResidentForm,
) -> Result<(), String> {
    let inventory = super::initial_forms::reviewed_inventory(source)?;
    if !inventory.forms.iter().any(|entry| {
        entry.source_document_id == form.source_document_id.as_str()
            && entry.checked_form_id == form.checked_form_id.as_str()
    }) {
        return Err("Form has a stale or missing reviewed identity".into());
    }
    Ok(())
}

pub(crate) fn workspace_library(
    source: &str,
) -> Result<conduit_workspace_model::library::FormLibrary, String> {
    use conduit_workspace_model::library::{FormLibrary, LibraryEntry};
    let inventory = super::initial_forms::reviewed_inventory(source)?;
    FormLibrary::new(
        inventory
            .forms
            .into_iter()
            .map(|entry| LibraryEntry {
                form: conduit_body::ResidentForm::new(
                    entry.source_document_id.into(),
                    entry.checked_form_id.into(),
                ),
                title: entry.title,
                search_text: format!("{} {}", entry.name, entry.required_kinds.join(" ")),
            })
            .collect(),
    )
    .map_err(|error| format!("Form library refused: {error:?}"))
}

pub(crate) fn plan_workspace_forms(
    evidence: &BodyBiographyEvidence,
    source: &str,
    host: &HostId,
    boot: &BootId,
) -> Result<Vec<BodyFormPlan>, String> {
    let inventory = super::initial_forms::check_inventory(source)?;
    let (startup, profile) = crate::installed_browser::catalogs()?;
    let backs = crate::installed_browser::backs(&startup, &profile)?;
    let hosts = [crate::installed_browser::advertisement(
        host.clone(),
        boot.clone(),
    )];
    let bases = crate::installed_browser::local_bases();
    let mut plans = Vec::with_capacity(evidence.body.workset.len());
    for resident in evidence.body.workset.forms() {
        let (document, form) = inventory
            .iter()
            .find_map(|entry| {
                if entry.checked.source_document_id != resident.source_document_id {
                    return None;
                }
                entry
                    .checked
                    .forms
                    .iter()
                    .find(|form| form.checked_form_id == resident.checked_form_id)
                    .map(|form| (&entry.checked, form))
            })
            .ok_or("Resident Form has a stale or missing checked identity")?;
        let expanded =
            conduit_form::expand_canonical_form_with_backs(document, &form.name, &profile, &backs)
                .map_err(|error| format!("Workspace expansion refused: {error:?}"))?;
        let placements = conduit_planner::default_expanded_placements(&expanded, &hosts)
            .map_err(|error| error.to_string())?;
        let plan = super::review::queue_plan::plan(&expanded, &hosts, &placements, &bases)?;
        plans.push(BodyFormPlan {
            form: resident.clone(),
            plan,
        });
    }
    Ok(plans)
}
