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
    use conduit_workspace_model::library::{
        FormLibrary, LibraryAvailability, LibraryEntry, MAX_LIBRARY_FORMS,
    };
    #[derive(serde::Deserialize)]
    struct Catalog {
        schema: String,
        maximum_forms: usize,
        forms: Vec<CatalogForm>,
    }
    #[derive(serde::Deserialize)]
    struct CatalogForm {
        title: String,
        entry: String,
        source_document_id: String,
        checked_form_id: String,
        required_kinds: Vec<String>,
        availability: CatalogAvailability,
        graceful_fallback: Option<CatalogFallback>,
    }
    #[derive(serde::Deserialize)]
    struct CatalogAvailability {
        disposition: String,
        reason: String,
    }
    #[derive(serde::Deserialize)]
    struct CatalogFallback {
        slug: String,
        title: String,
        disposition: String,
        reason: String,
    }
    let catalog: Catalog = serde_json::from_str(source)
        .map_err(|_| "reviewed Workspace Form catalog is malformed".to_string())?;
    if catalog.schema != "conduit.workspace/reviewed-form-catalog@1"
        || catalog.maximum_forms == 0
        || catalog.maximum_forms > MAX_LIBRARY_FORMS
        || catalog.forms.is_empty()
        || catalog.forms.len() > catalog.maximum_forms
    {
        return Err("reviewed Workspace Form catalog violates its bound".into());
    }
    FormLibrary::new(
        catalog
            .forms
            .into_iter()
            .map(|entry| {
                let availability = match entry.availability.disposition.as_str() {
                    "available" if !entry.availability.reason.is_empty() => {
                        LibraryAvailability::Available
                    }
                    "needs-capability" if !entry.availability.reason.is_empty() => {
                        LibraryAvailability::NeedsCapability(entry.availability.reason)
                    }
                    _ => return Err("reviewed Workspace Form availability is malformed".into()),
                };
                Ok(LibraryEntry {
                    form: conduit_body::ResidentForm::new(
                        entry.source_document_id.into(),
                        entry.checked_form_id.into(),
                    ),
                    title: entry.title,
                    search_text: format!("{} {}", entry.entry, entry.required_kinds.join(" ")),
                    availability,
                    graceful_fallback: entry
                        .graceful_fallback
                        .map(|fallback| -> Result<_, String> {
                            if fallback.slug.is_empty() || fallback.title.is_empty() {
                                return Err("reviewed Workspace graceful fallback is malformed".into());
                            }
                            let availability = match fallback.disposition.as_str() {
                                "available" if !fallback.reason.is_empty() => {
                                    LibraryAvailability::Available
                                }
                                "needs-capability" if !fallback.reason.is_empty() => {
                                    LibraryAvailability::NeedsCapability(fallback.reason)
                                }
                                _ => {
                                    return Err(
                                        "reviewed Workspace graceful fallback availability is malformed"
                                            .into(),
                                    )
                                }
                            };
                            Ok(conduit_workspace_model::library::LibraryFallback {
                                title: fallback.title,
                                availability,
                            })
                        })
                        .transpose()?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?,
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
    let hosts = [super::initial_forms::reviewed_browser_host(
        source,
        host.clone(),
        boot.clone(),
    )?];
    let bases = crate::installed_browser::local_bases();
    let mut plans = Vec::with_capacity(evidence.body.workset.len());
    for resident in evidence.body.workset.forms() {
        let (document, form, presentation) = inventory
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
                    .map(|form| (&entry.checked, form, entry.presentation))
            })
            .ok_or("Resident Form has a stale or missing checked identity")?;
        let (startup, mut profile) =
            crate::installed_browser::catalogs_for_presentation(presentation)?;
        crate::installed_browser::catalogs::install_checked_structured_selectors(
            document,
            &mut profile,
        )?;
        let backs = crate::installed_browser::backs(&startup, &profile)?;
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
