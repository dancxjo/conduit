//! Crèche-to-workspace handoff and shared exact inventory planning.
use super::{JoinedLineObservation, PlanningAuthority};
use conduit_body::{BodyBiographyEvidence, BodyPlotPlan};
use conduit_core::{BootId, HostAdvertisement, HostId};

#[derive(serde::Deserialize)]
struct WorkspaceCatalog {
    schema: String,
    maximum_plots: usize,
    plots: Vec<CatalogPlot>,
}

#[derive(serde::Deserialize)]
struct CatalogPlot {
    slug: String,
    title: String,
    entry: String,
    source: String,
    source_document_id: String,
    checked_plot_id: String,
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

pub(crate) fn require_workspace_plot(
    source: &str,
    plot: &conduit_body::ResidentPlot,
) -> Result<(), String> {
    let inventory = super::initial_plots::reviewed_inventory(source)?;
    if !inventory.plots.iter().any(|entry| {
        entry.source_document_id == plot.source_document_id.as_str()
            && entry.checked_plot_id == plot.checked_plot_id.as_str()
    }) {
        return Err("Plot has a stale or missing reviewed identity".into());
    }
    Ok(())
}

pub(crate) fn workspace_library(
    source: &str,
    observed_hosts: &[HostAdvertisement],
    host: &HostId,
    boot: &BootId,
    joined_lines: &[JoinedLineObservation],
) -> Result<conduit_plot_library::PlotLibrary, String> {
    use conduit_plot_library::{LibraryEntry, PlotLibrary, MAX_LIBRARY_PLOTS};
    let catalog: WorkspaceCatalog = serde_json::from_str(source)
        .map_err(|_| "reviewed Workspace Plot catalog is malformed".to_string())?;
    if catalog.schema != "conduit.workspace/reviewed-plot-catalog@3"
        || catalog.maximum_plots == 0
        || catalog.maximum_plots > MAX_LIBRARY_PLOTS
        || catalog.plots.is_empty()
        || catalog.plots.len() > catalog.maximum_plots
    {
        return Err("reviewed Workspace Plot catalog violates its bound".into());
    }
    if !observed_hosts
        .iter()
        .any(|observed| &observed.host_id == host && &observed.boot_id == boot)
    {
        return Err("current browser Host offer was not freshly observed".into());
    }
    let plots = catalog.plots;
    PlotLibrary::new(
        plots
            .iter()
            .map(|entry| {
                let availability =
                    catalog_availability(entry, observed_hosts, host, boot, joined_lines);
                Ok(LibraryEntry {
                    plot: conduit_body::ResidentPlot::new(
                        entry.source_document_id.clone().into(),
                        entry.checked_plot_id.clone().into(),
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
                            let fallback_entry = plots
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
                            Ok(conduit_plot_library::LibraryFallback {
                                title: fallback.title.clone(),
                                availability,
                            })
                        })
                        .transpose()?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?,
    )
    .map_err(|error| format!("Plot library refused: {error:?}"))
}

fn catalog_availability(
    entry: &CatalogPlot,
    observed_hosts: &[HostAdvertisement],
    host: &HostId,
    boot: &BootId,
    joined_lines: &[JoinedLineObservation],
) -> conduit_plot_library::LibraryAvailability {
    use conduit_plot_library::LibraryAvailability;
    match catalog_plot_plan(entry, observed_hosts, host, boot, joined_lines) {
        Ok(()) => LibraryAvailability::Available,
        Err(_) => LibraryAvailability::needs_capability(entry.unavailable_hint.clone())
            .expect("reviewed library capability hint is bounded"),
    }
}

fn catalog_plot_plan(
    entry: &CatalogPlot,
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
        _ => return Err("reviewed plot has an unsupported presentation profile".into()),
    };
    let document =
        super::initial_plots::check_source_for_presentation(&entry.source, presentation)?;
    if document.source_document_id.as_str() != entry.source_document_id {
        return Err("reviewed plot has stale source identity".into());
    }
    let plot = document
        .plots
        .iter()
        .find(|plot| {
            plot.name == entry.entry && plot.checked_plot_id.as_str() == entry.checked_plot_id
        })
        .ok_or("reviewed plot has stale checked identity")?;
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
        conduit_plot::expand_canonical_plot_with_backs(&document, &plot.name, &profile, &backs)
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

pub(crate) fn plan_workspace_plots(
    evidence: &BodyBiographyEvidence,
    source: &str,
    observed_hosts: &[conduit_core::HostAdvertisement],
    host: &HostId,
    boot: &BootId,
    joined_lines: &[JoinedLineObservation],
    authority: PlanningAuthority,
) -> Result<Vec<BodyPlotPlan>, String> {
    let mut catalogs = super::catalog_preparation::CatalogPreparation::default();
    let inventory = super::initial_plots::check_inventory_with_catalogs(source, &mut catalogs)?;
    if !observed_hosts
        .iter()
        .any(|observed| &observed.host_id == host && &observed.boot_id == boot)
    {
        return Err("current browser Host offer was not freshly observed".into());
    }
    let local = super::initial_plots::reviewed_browser_host_with_inventory(
        &inventory,
        host.clone(),
        boot.clone(),
        &mut catalogs,
    )?;
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
    for resident in evidence.body.workset.plots() {
        let (document, plot, presentation) = inventory
            .iter()
            .find_map(|entry| {
                if entry.checked.source_document_id != resident.source_document_id {
                    return None;
                }
                entry
                    .checked
                    .plots
                    .iter()
                    .find(|plot| plot.checked_plot_id == resident.checked_plot_id)
                    .map(|plot| (&entry.checked, plot, entry.presentation))
            })
            .ok_or("Resident Plot has a stale or missing checked identity")?;
        let (startup, base_profile) = catalogs.get(presentation)?;
        let mut profile = base_profile.clone();
        crate::installed_browser::catalogs::install_checked_structured_selectors(
            document,
            &mut profile,
        )?;
        let backs = crate::installed_browser::backs(startup, &profile)?;
        let expanded =
            conduit_plot::expand_canonical_plot_with_backs(document, &plot.name, &profile, &backs)
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
        plans.push(BodyPlotPlan {
            plot: resident.clone(),
            plan,
        });
    }
    Ok(plans)
}
