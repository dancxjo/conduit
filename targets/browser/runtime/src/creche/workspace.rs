//! Crèche-to-workspace handoff and shared exact inventory planning.
use super::{JoinedLineObservation, PlanningAuthority};
use conduit_body::{BodyBiographyEvidence, BodyFormPlan};
use conduit_core::{BootId, HostAdvertisement, HostId};

#[derive(serde::Deserialize)]
struct WorkspaceCatalog {
    schema: String,
    maximum_forms: usize,
    forms: Vec<CatalogForm>,
}

#[derive(serde::Deserialize)]
struct CatalogForm {
    slug: String,
    title: String,
    entry: String,
    source: String,
    source_document_id: String,
    checked_form_id: String,
    presentation_profile: u8,
    required_kinds: Vec<String>,
    unavailable_hint: String,
    graceful_fallback: Option<CatalogFallback>,
}

#[derive(serde::Deserialize)]
struct CatalogFallback {
    slug: String,
    title: String,
}

pub(crate) fn workspace_evidence() -> Result<BodyBiographyEvidence, String> {
    super::session::biography().ok_or_else(|| "Birth a body before arriving".into())
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
    observed_hosts: &[HostAdvertisement],
    host: &HostId,
    boot: &BootId,
    joined_lines: &[JoinedLineObservation],
) -> Result<conduit_workspace_model::library::FormLibrary, String> {
    use conduit_workspace_model::library::{FormLibrary, LibraryEntry, MAX_LIBRARY_FORMS};
    let catalog: WorkspaceCatalog = serde_json::from_str(source)
        .map_err(|_| "reviewed Workspace Form catalog is malformed".to_string())?;
    if catalog.schema != "conduit.workspace/reviewed-form-catalog@2"
        || catalog.maximum_forms == 0
        || catalog.maximum_forms > MAX_LIBRARY_FORMS
        || catalog.forms.is_empty()
        || catalog.forms.len() > catalog.maximum_forms
    {
        return Err("reviewed Workspace Form catalog violates its bound".into());
    }
    if !observed_hosts
        .iter()
        .any(|observed| &observed.host_id == host && &observed.boot_id == boot)
    {
        return Err("current browser Host offer was not freshly observed".into());
    }
    let forms = catalog.forms;
    FormLibrary::new(
        forms
            .iter()
            .map(|entry| {
                let availability =
                    catalog_availability(entry, observed_hosts, host, boot, joined_lines);
                Ok(LibraryEntry {
                    form: conduit_body::ResidentForm::new(
                        entry.source_document_id.clone().into(),
                        entry.checked_form_id.clone().into(),
                    ),
                    title: entry.title.clone(),
                    search_text: format!("{} {}", entry.entry, entry.required_kinds.join(" ")),
                    availability,
                    graceful_fallback: entry
                        .graceful_fallback
                        .as_ref()
                        .map(|fallback| -> Result<_, String> {
                            if fallback.slug.is_empty() || fallback.title.is_empty() {
                                return Err(
                                    "reviewed Workspace graceful fallback is malformed".into()
                                );
                            }
                            let fallback_entry = forms
                                .iter()
                                .find(|candidate| candidate.slug == fallback.slug)
                                .ok_or("reviewed Workspace graceful fallback is missing")?;
                            let availability = catalog_availability(
                                fallback_entry,
                                observed_hosts,
                                host,
                                boot,
                                joined_lines,
                            );
                            Ok(conduit_workspace_model::library::LibraryFallback {
                                title: fallback.title.clone(),
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

fn catalog_availability(
    entry: &CatalogForm,
    observed_hosts: &[HostAdvertisement],
    host: &HostId,
    boot: &BootId,
    joined_lines: &[JoinedLineObservation],
) -> conduit_workspace_model::library::LibraryAvailability {
    use conduit_workspace_model::library::LibraryAvailability;
    match catalog_form_plan(entry, observed_hosts, host, boot, joined_lines) {
        Ok(()) => LibraryAvailability::Available,
        Err(_) => LibraryAvailability::NeedsCapability(entry.unavailable_hint.clone()),
    }
}

fn catalog_form_plan(
    entry: &CatalogForm,
    observed_hosts: &[HostAdvertisement],
    host: &HostId,
    boot: &BootId,
    joined_lines: &[JoinedLineObservation],
) -> Result<(), String> {
    let presentation = match entry.presentation_profile {
        0 => crate::installed_browser::PresentationProfile::Annotation,
        1 => crate::installed_browser::PresentationProfile::Quantity,
        2 => crate::installed_browser::PresentationProfile::NormalizedDurations,
        3 => crate::installed_browser::PresentationProfile::PatternComparison,
        _ => return Err("reviewed form has an unsupported presentation profile".into()),
    };
    let document =
        super::initial_forms::check_source_for_presentation(&entry.source, presentation)?;
    if document.source_document_id.as_str() != entry.source_document_id {
        return Err("reviewed form has stale source identity".into());
    }
    let form = document
        .forms
        .iter()
        .find(|form| {
            form.name == entry.entry && form.checked_form_id.as_str() == entry.checked_form_id
        })
        .ok_or("reviewed form has stale checked identity")?;
    let (startup, mut profile) = crate::installed_browser::catalogs_for_presentation(presentation)?;
    let offers = crate::installed_browser::catalogs::install_checked_structured_selectors(
        &document,
        &mut profile,
    )?;
    let mut hosts = observed_hosts.to_vec();
    let local = hosts
        .iter_mut()
        .find(|observed| &observed.host_id == host && &observed.boot_id == boot)
        .ok_or("current browser Host offer was not freshly observed")?;
    for offer in offers {
        if !local
            .capabilities
            .iter()
            .any(|current| current.capability_id == offer.capability_id)
        {
            local.capabilities.push(offer);
        }
    }
    let backs = crate::installed_browser::backs(&startup, &profile)?;
    let expanded =
        conduit_form::expand_canonical_form_with_backs(&document, &form.name, &profile, &backs)
            .map_err(|error| format!("Workspace expansion refused: {error:?}"))?;
    let placements = conduit_planner::default_expanded_placements(&expanded, &hosts)
        .map_err(|error| error.to_string())?;
    super::review::queue_plan::plan(
        &expanded,
        &hosts,
        &placements,
        &crate::installed_browser::local_bases(),
        joined_lines,
        PlanningAuthority {
            browser_audio: true,
        },
    )?;
    Ok(())
}

pub(crate) fn plan_workspace_forms(
    evidence: &BodyBiographyEvidence,
    source: &str,
    observed_hosts: &[conduit_core::HostAdvertisement],
    host: &HostId,
    boot: &BootId,
    joined_lines: &[JoinedLineObservation],
    authority: PlanningAuthority,
) -> Result<Vec<BodyFormPlan>, String> {
    let inventory = super::initial_forms::check_inventory(source)?;
    if !observed_hosts
        .iter()
        .any(|observed| &observed.host_id == host && &observed.boot_id == boot)
    {
        return Err("current browser Host offer was not freshly observed".into());
    }
    let local = super::initial_forms::reviewed_browser_host(source, host.clone(), boot.clone())?;
    let mut hosts = vec![local];
    hosts.extend(
        observed_hosts
            .iter()
            .filter(|observed| &observed.host_id != host)
            .cloned(),
    );
    hosts.sort_by(|left, right| left.host_id.cmp(&right.host_id));
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
        let plan = super::review::queue_plan::plan(
            &expanded,
            &hosts,
            &placements,
            &bases,
            joined_lines,
            authority,
        )?;
        plans.push(BodyFormPlan {
            form: resident.clone(),
            plan,
        });
    }
    Ok(plans)
}
