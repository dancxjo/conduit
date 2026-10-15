//! Crèche-to-workspace handoff and shared exact inventory planning.
use super::{JoinedLineObservation, PlanningAuthority};
use conduit_body::{BodyBiographyEvidence, BodyPlotPlan};
use conduit_core::{BootId, HostAdvertisement, HostId};
use sha2::{Digest, Sha256};
use std::{cell::RefCell, collections::BTreeMap};

// One immutable instance-local admission result. Source definitions and every
// dynamic planning fact are bound; workset, query and view revision remain live.
struct LibraryAdmission {
    key: [u8; 32],
    available: Vec<bool>,
}
thread_local! {
    static LIBRARY_ADMISSION: RefCell<Option<LibraryAdmission>> = const { RefCell::new(None) };
}

fn admission_key(
    source: &str,
    hosts: &[HostAdvertisement],
    host: &HostId,
    boot: &BootId,
    lines: &[JoinedLineObservation],
) -> Result<[u8; 32], String> {
    let lines: Vec<_> = lines
        .iter()
        .map(|line| (&line.host_id, &line.boot_id, &line.carrier))
        .collect();
    let evidence = serde_json::to_vec(&(source, hosts, host, boot, lines))
        .map_err(|error| format!("library admission evidence: {error}"))?;
    Ok(Sha256::digest(evidence).into())
}

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
    face_profile: u8,
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
    let key = admission_key(source, observed_hosts, host, boot, joined_lines)?;
    let available = LIBRARY_ADMISSION
        .with(|cache| {
            cache
                .borrow()
                .as_ref()
                .filter(|prior| prior.key == key)
                .map(|prior| prior.available.clone())
        })
        .unwrap_or_else(|| {
            let mut catalogs = super::catalog_preparation::CatalogPreparation::default();
            let mut order: Vec<usize> = (0..plots.len()).collect();
            order.sort_by_key(|index| plots[*index].face_profile);
            let mut available = vec![false; plots.len()];
            for index in order {
                available[index] = catalog_plot_plan(
                    &plots[index],
                    observed_hosts,
                    host,
                    boot,
                    joined_lines,
                    &mut catalogs,
                )
                .is_ok();
            }
            available
        });
    let availability = |index: usize| {
        if available[index] {
            conduit_plot_library::LibraryAvailability::Available
        } else {
            conduit_plot_library::LibraryAvailability::needs_capability(
                plots[index].unavailable_hint.clone(),
            )
            .expect("reviewed library capability hint is bounded")
        }
    };
    let library = PlotLibrary::new(
        plots
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let entry_availability = availability(index);
                Ok(LibraryEntry {
                    plot: conduit_body::ResidentPlot::new(
                        entry.source_document_id.clone().into(),
                        entry.checked_plot_id.clone().into(),
                    ),
                    title: entry.title.clone(),
                    search_text: format!("{} {}", entry.entry, entry.required_kinds.join(" ")),
                    availability: entry_availability,
                    graceful_fallback: entry
                        .graceful_fallback
                        .as_ref()
                        .map(|fallback| -> Result<_, String> {
                            if fallback.slug.is_empty() || fallback.title.is_empty() {
                                return Err(
                                    "reviewed Workspace graceful fallback is malformed".into()
                                );
                            }
                            let fallback_index = plots
                                .iter()
                                .position(|candidate| candidate.slug == fallback.slug)
                                .ok_or("reviewed Workspace graceful fallback is missing")?;
                            let availability = availability(fallback_index);
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
    .map_err(|error| format!("Plot library refused: {error:?}"))?;
    // Publish only after the complete inventory and face metadata admit.
    LIBRARY_ADMISSION.with(|cache| *cache.borrow_mut() = Some(LibraryAdmission { key, available }));
    Ok(library)
}

fn catalog_plot_plan(
    entry: &CatalogPlot,
    observed_hosts: &[HostAdvertisement],
    host: &HostId,
    boot: &BootId,
    joined_lines: &[JoinedLineObservation],
    catalogs: &mut super::catalog_preparation::CatalogPreparation,
) -> Result<(), String> {
    let face = match entry.face_profile {
        0 => crate::installed_browser::FaceProfile::Annotation,
        1 => crate::installed_browser::FaceProfile::Quantity,
        2 => crate::installed_browser::FaceProfile::NormalizedDurations,
        3 => crate::installed_browser::FaceProfile::PatternComparison,
        _ => return Err("reviewed plot has an unsupported face profile".into()),
    };
    let (startup, base_profile, installed_backs) = catalogs.get_with_backs(face)?;
    let document =
        super::initial_plots::check_source_with_catalogs(&entry.source, startup, base_profile)?;
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
    let mut profile = std::borrow::Cow::Borrowed(base_profile);
    let has_selectors = document
        .plots
        .iter()
        .flat_map(|plot| &plot.cords)
        .flat_map(|cord| &cord.stages)
        .any(|stage| {
            matches!(
                stage,
                conduit_plot::CheckedCordStage::StructuredSelector { .. }
            )
        });
    let offers = if has_selectors {
        crate::installed_browser::catalogs::install_checked_structured_selectors(
            &document,
            profile.to_mut(),
        )?
    } else {
        Vec::new()
    };
    let mut hosts = std::borrow::Cow::Borrowed(observed_hosts);
    let local = hosts
        .iter()
        .position(|observed| &observed.host_id == host && &observed.boot_id == boot)
        .ok_or("current browser Host offer was not freshly observed")?;
    for offer in offers {
        if !hosts[local]
            .capabilities
            .iter()
            .any(|current| current.capability_id == offer.capability_id)
        {
            hosts.to_mut()[local].capabilities.push(offer);
        }
    }
    let backs = if has_selectors {
        std::borrow::Cow::Owned(crate::installed_browser::backs(startup, &profile)?)
    } else {
        std::borrow::Cow::Borrowed(installed_backs)
    };
    if plot.name == "thermostat/main" {
        let authoring = conduit_plot::expand_canonical_plot_for_authoring_with_backs(
            &document, &plot.name, &profile, &backs,
        )
        .map_err(|error| format!("Workspace thermostat expansion refused: {error:?}"))?;
        let placements = conduit_planner::default_expanded_placements(&authoring.expanded, &hosts)
            .map_err(|error| error.to_string())?;
        conduit_planner::plan_expanded_authoring_with_activations(
            &document,
            &authoring,
            &profile,
            &backs,
            &hosts,
            &placements,
            &crate::installed_browser::local_bases(),
            conduit_planner::PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: 32,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &crate::installed_browser::thermostat_scan_offer::fore_bounds(),
        )
        .map_err(|error| error.to_string())?;
        return Ok(());
    }
    let expanded =
        conduit_plot::expand_canonical_plot_with_backs(&document, &plot.name, &profile, &backs)
            .map_err(|error| format!("Workspace expansion refused: {error:?}"))?;
    for offer in
        crate::installed_browser::catalogs::offers_for_expanded_pure_expressions(&expanded)?
    {
        if !hosts[local]
            .capabilities
            .iter()
            .any(|current| current.capability_id == offer.capability_id)
        {
            hosts.to_mut()[local].capabilities.push(offer);
        }
    }
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
        let (document, plot, face) = inventory
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
                    .map(|plot| (&entry.checked, plot, entry.face))
            })
            .ok_or("Resident Plot has a stale or missing checked identity")?;
        let (startup, base_profile) = catalogs.get(face)?;
        let mut profile = base_profile.clone();
        crate::installed_browser::catalogs::install_checked_structured_selectors(
            document,
            &mut profile,
        )?;
        let backs = crate::installed_browser::backs(startup, &profile)?;
        if plot.name == "thermostat/main" {
            let authoring = conduit_plot::expand_canonical_plot_for_authoring_with_backs(
                document, &plot.name, &profile, &backs,
            )
            .map_err(|error| format!("Workspace thermostat expansion refused: {error:?}"))?;
            let placements =
                conduit_planner::default_expanded_placements(&authoring.expanded, &hosts)
                    .map_err(|error| error.to_string())?;
            let plan = conduit_planner::plan_expanded_authoring_with_activations(
                document,
                &authoring,
                &profile,
                &backs,
                &hosts,
                &placements,
                &bases,
                conduit_planner::PlanningOptions {
                    connection_bases: &BTreeMap::new(),
                    line_candidates: &BTreeMap::new(),
                    connection_item_capacity: 1,
                    connection_byte_capacity: 32,
                    authority_grants: &[],
                    protected_resource_grants: &[],
                    line_offers: &[],
                },
                &crate::installed_browser::thermostat_scan_offer::fore_bounds(),
            )
            .map_err(|error| format!("Workspace thermostat Plan refused: {error:?}"))?;
            plans.push(BodyPlotPlan {
                plot: resident.clone(),
                plan,
            });
            continue;
        }
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

#[cfg(test)]
mod admission_tests;
