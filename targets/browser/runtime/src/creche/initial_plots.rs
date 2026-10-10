//! Check and admit the bounded initial Plot selection as workload revision zero.

use super::protocol::InitialPlotReceipt;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Deserialize)]
struct ReviewedPlotBundle {
    schema: String,
    plots: Vec<BundledPlot>,
}

#[derive(Deserialize)]
struct BundledPlot {
    slug: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    entry: Option<String>,
    #[serde(default)]
    presentation_profile: u8,
    source: String,
}

pub(super) struct CheckedInventoryEntry {
    pub(super) source: String,
    pub(super) checked: conduit_plot::CheckedSyntaxDocument,
    pub(super) entry_name: Option<String>,
    pub(super) entry_title: Option<String>,
    pub(super) presentation: crate::installed_browser::PresentationProfile,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct InitialPlotSelection {
    pub(super) name: String,
    pub(super) source_document_id: String,
    pub(super) checked_plot_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(super) struct ReviewedPlotInventory {
    pub(super) schema: &'static str,
    pub(super) source_document_id: String,
    pub(super) maximum_selection: usize,
    pub(super) plots: Vec<ReviewedPlot>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(super) struct ReviewedPlot {
    pub(super) name: String,
    pub(super) title: String,
    pub(super) source: String,
    pub(super) source_document_id: String,
    pub(super) checked_plot_id: String,
    pub(super) required_kinds: Vec<String>,
}

pub(super) fn reviewed_inventory(source: &str) -> Result<ReviewedPlotInventory, String> {
    let checked_documents = check_inventory(source)?;
    let source_document_id = conduit_plot::source_document_identity(source)
        .as_str()
        .to_string();
    let mut plots = Vec::new();
    for entry in &checked_documents {
        for plot in entry.checked.plots.iter().filter(|plot| {
            entry
                .entry_name
                .as_ref()
                .is_none_or(|name| name == &plot.name)
        }) {
            let mut required_kinds: Vec<String> =
                plot.gears.iter().map(|gear| gear.kind.clone()).collect();
            required_kinds.sort();
            required_kinds.dedup();
            plots.push(ReviewedPlot {
                name: plot.name.clone(),
                title: entry
                    .entry_title
                    .clone()
                    .unwrap_or_else(|| title(&plot.name)),
                source: entry.source.clone(),
                source_document_id: entry.checked.source_document_id.as_str().to_string(),
                checked_plot_id: plot.checked_plot_id.as_str().to_string(),
                required_kinds,
            });
        }
    }
    Ok(ReviewedPlotInventory {
        schema: "conduit.creche/reviewed-plot-inventory@2",
        source_document_id,
        maximum_selection: conduit_body::MAX_BODY_PLOTS,
        plots,
    })
}

pub(super) fn checked_workset(
    source: &str,
    initial_plots_json: &str,
) -> Result<(conduit_body::BodyWorkset, Vec<InitialPlotReceipt>), String> {
    let selected: Vec<InitialPlotSelection> = serde_json::from_str(initial_plots_json)
        .map_err(|_| "initial Plot selection is not an exact identity list".to_string())?;
    if selected.len() > conduit_body::MAX_BODY_PLOTS {
        return Err("initial Plot selection exceeds Body capacity".into());
    }

    let checked_documents = check_inventory(source)?;
    let mut receipts = Vec::with_capacity(selected.len());
    let mut workset = conduit_body::BodyWorkset::default();
    for selection in selected {
        let (checked, checked_plot) = checked_documents
            .iter()
            .find_map(|entry| {
                entry
                    .checked
                    .plots
                    .iter()
                    .find(|plot| {
                        plot.name == selection.name
                            && entry
                                .entry_name
                                .as_ref()
                                .is_none_or(|name| name == &plot.name)
                    })
                    .map(|plot| (&entry.checked, plot))
            })
            .ok_or_else(|| {
                format!(
                    "selected initial Plot {:?} is absent from checked inventory",
                    selection.name
                )
            })?;
        if selection.source_document_id != checked.source_document_id.as_str()
            || selection.checked_plot_id != checked_plot.checked_plot_id.as_str()
        {
            return Err(format!(
                "selected initial Plot {:?} has a stale or substituted exact identity",
                selection.name
            ));
        }
        workset
            .add(conduit_body::ResidentPlot::new(
                checked.source_document_id.clone(),
                checked_plot.checked_plot_id.clone(),
            ))
            .map_err(|error| format!("admit initial Plot: {error:?}"))?;
        receipts.push(InitialPlotReceipt {
            name: selection.name,
            source_document_id: checked.source_document_id.as_str().into(),
            checked_plot_id: checked_plot.checked_plot_id.as_str().into(),
        });
    }
    Ok((workset, receipts))
}

pub(super) fn check_inventory(source: &str) -> Result<Vec<CheckedInventoryEntry>, String> {
    check_inventory_with_catalogs(
        source,
        &mut super::catalog_preparation::CatalogPreparation::default(),
    )
}

pub(super) fn check_inventory_with_catalogs(
    source: &str,
    catalogs: &mut super::catalog_preparation::CatalogPreparation,
) -> Result<Vec<CheckedInventoryEntry>, String> {
    let Ok(bundle) = serde_json::from_str::<ReviewedPlotBundle>(source) else {
        let (startup, profile) =
            catalogs.get(crate::installed_browser::PresentationProfile::Annotation)?;
        return check_source_with_catalogs(source, startup, profile).map(|checked| {
            vec![CheckedInventoryEntry {
                source: source.to_owned(),
                checked,
                entry_name: None,
                entry_title: None,
                presentation: crate::installed_browser::PresentationProfile::Annotation,
            }]
        });
    };
    if bundle.schema != "conduit.creche/reviewed-plot-bundle@2"
        || bundle.plots.is_empty()
        || bundle.plots.len() > conduit_body::MAX_BODY_PLOTS
    {
        return Err("reviewed plot bundle is malformed or over capacity".into());
    }
    let mut checked = Vec::with_capacity(bundle.plots.len());
    let mut names = BTreeSet::new();
    let mut identities = BTreeSet::new();
    for entry in bundle.plots {
        if entry.slug.is_empty()
            || entry.source.is_empty()
            || entry
                .title
                .as_ref()
                .is_some_and(|title| title.is_empty() || title.len() > 256)
        {
            return Err("reviewed plot bundle entry is malformed".into());
        }
        let presentation = match entry.presentation_profile {
            0 => crate::installed_browser::PresentationProfile::Annotation,
            1 => crate::installed_browser::PresentationProfile::Quantity,
            2 => crate::installed_browser::PresentationProfile::NormalizedDurations,
            3 => crate::installed_browser::PresentationProfile::PatternComparison,
            _ => return Err("reviewed plot has an unsupported presentation profile".into()),
        };
        let (startup, profile) = catalogs.get(presentation)?;
        let document = check_source_with_catalogs(&entry.source, startup, profile)?;
        let expected_entry = entry.entry.unwrap_or_else(|| entry.slug.replace('-', "_"));
        let plot = document
            .plots
            .iter()
            .find(|plot| plot.name == expected_entry)
            .ok_or_else(|| {
                format!(
                    "reviewed plot bundle entry {:?} has mismatched declared entry {:?}",
                    entry.slug, expected_entry
                )
            })?;
        if !names.insert(plot.name.clone())
            || !identities.insert(plot.checked_plot_id.as_str().to_string())
        {
            return Err(format!(
                "reviewed plot bundle entry {:?} has mismatched or duplicate provenance",
                entry.slug
            ));
        }
        checked.push(CheckedInventoryEntry {
            source: entry.source,
            checked: document,
            entry_name: Some(expected_entry),
            entry_title: entry.title,
            presentation,
        });
    }
    Ok(checked)
}

pub(crate) fn inventory_application_subjects(
    source: &str,
    residents: &[conduit_body::ResidentPlot],
) -> Result<Vec<(conduit_plot::ExpandedCanonicalPlot, String)>, String> {
    if residents.len() > conduit_body::MAX_BODY_PLOTS {
        return Err("resident application subjects exceed Body capacity".into());
    }
    let mut catalogs = super::catalog_preparation::CatalogPreparation::default();
    let inventory = check_inventory_with_catalogs(source, &mut catalogs)?;
    residents
        .iter()
        .map(|resident| {
            let (entry, plot) = inventory
                .iter()
                .filter(|entry| entry.checked.source_document_id == resident.source_document_id)
                .find_map(|entry| {
                    entry
                        .checked
                        .plots
                        .iter()
                        .find(|plot| {
                            plot.checked_plot_id == resident.checked_plot_id
                                && entry
                                    .entry_name
                                    .as_ref()
                                    .is_none_or(|name| name == &plot.name)
                        })
                        .map(|plot| (entry, plot))
                })
                .ok_or("resident application subject is absent from the reviewed inventory")?;
            let (_, profile) = catalogs.get(entry.presentation)?;
            let expanded = conduit_plot::expand_canonical_plot(&entry.checked, &plot.name, profile)
                .map_err(|error| format!("expand resident application subject: {error:?}"))?;
            let title = entry
                .entry_title
                .clone()
                .unwrap_or_else(|| title(&plot.name));
            Ok((expanded, title))
        })
        .collect()
}

#[cfg(test)]
pub(super) fn check_source(source: &str) -> Result<conduit_plot::CheckedSyntaxDocument, String> {
    check_source_for_presentation(
        source,
        crate::installed_browser::PresentationProfile::Annotation,
    )
}

#[cfg(test)]
pub(super) fn check_source_for_presentation(
    source: &str,
    presentation: crate::installed_browser::PresentationProfile,
) -> Result<conduit_plot::CheckedSyntaxDocument, String> {
    let (startup, profile) = crate::installed_browser::catalogs_for_presentation(presentation)?;
    check_source_with_catalogs(source, &startup, &profile)
}

pub(super) fn check_source_with_catalogs(
    source: &str,
    startup: &conduit_plot::StartupCatalog,
    profile: &conduit_plot::ProfileCatalog,
) -> Result<conduit_plot::CheckedSyntaxDocument, String> {
    let syntax = conduit_plot::parse_syntax_document_with_glyph_notations(source, startup);
    if let Some(diagnostic) = syntax.diagnostics.first() {
        return Err(format!(
            "parse reviewed plot inventory: {}",
            diagnostic.message
        ));
    }
    conduit_plot::check_syntax_document_with_literal_constructors(&syntax, startup, profile)
        .map_err(|error| format!("check reviewed plot inventory: {error:?}"))
}

fn title(name: &str) -> String {
    name.split('_')
        .map(|word| {
            let mut characters = word.chars();
            characters.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(characters).collect()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The installed generic selector realizes only exact checked selector contracts.
/// This prepares the local browser offer; it does not add offers to other Hosts.
pub(super) fn reviewed_browser_host(
    source: &str,
    host: conduit_core::HostId,
    boot: conduit_core::BootId,
) -> Result<conduit_core::HostAdvertisement, String> {
    let mut catalogs = super::catalog_preparation::CatalogPreparation::default();
    let inventory = check_inventory_with_catalogs(source, &mut catalogs)?;
    reviewed_browser_host_with_inventory(&inventory, host, boot, &mut catalogs)
}

pub(super) fn reviewed_browser_host_with_inventory(
    inventory: &[CheckedInventoryEntry],
    host: conduit_core::HostId,
    boot: conduit_core::BootId,
    catalogs: &mut super::catalog_preparation::CatalogPreparation,
) -> Result<conduit_core::HostAdvertisement, String> {
    let mut host = crate::installed_browser::advertisement(host, boot);
    for entry in inventory {
        let (startup, base_profile) = catalogs.get(entry.presentation)?;
        // Selectors are specific to this checked document, not retained in the
        // installed base profile used for the next document.
        let mut profile = base_profile.clone();
        let mut offers = crate::installed_browser::catalogs::install_checked_structured_selectors(
            &entry.checked,
            &mut profile,
        )?;
        // Expressions can occur in a called local Plot, so inspect the entire
        // checked document before deciding whether expansion is necessary.
        // Most shelf entries contain none: avoid constructing every Back and
        // expanding those documents on each durable Body transition.
        let has_expressions = entry.checked.plots.iter().any(|plot| {
            plot.cords.iter().any(|cord| {
                cord.stages.iter().any(|stage| {
                    matches!(
                        stage,
                        conduit_plot::CheckedCordStage::PureExpression { .. }
                            | conduit_plot::CheckedCordStage::When { .. }
                    )
                })
            })
        });
        if has_expressions {
            let backs = crate::installed_browser::backs(startup, &profile)?;
            for plot in &entry.checked.plots {
                if entry
                    .entry_name
                    .as_deref()
                    .is_some_and(|name| name != plot.name)
                {
                    continue;
                }
                let authored = conduit_plot::expand_canonical_plot_for_authoring_with_backs(
                    &entry.checked,
                    &plot.name,
                    &profile,
                    &backs,
                )
                .map_err(|error| format!("browser expression offer expansion: {error:?}"))?;
                offers.extend(
                    crate::installed_browser::catalogs::offers_for_expanded_pure_expressions(
                        &authored.expanded,
                    )?,
                );
            }
        }
        for offer in offers {
            if !host
                .capabilities
                .iter()
                .any(|current| current.capability_id == offer.capability_id)
            {
                host.capabilities.push(offer);
            }
        }
    }
    host.capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    Ok(host)
}

#[cfg(test)]
mod glyph_tests {
    use super::*;
    const SOURCE: &str =
        include_str!("../../../../../proof/browser/fixtures/scoped-pattern-glyph.conduit");
    #[test]
    fn resident_inventory_retains_checked_glyph_source_identity() {
        let checked = check_source(SOURCE).unwrap();
        let inventory = reviewed_inventory(SOURCE).unwrap();
        let root = inventory
            .plots
            .iter()
            .find(|plot| plot.name == "scoped-pattern-glyph")
            .unwrap();
        let checked_root = checked
            .plots
            .iter()
            .find(|plot| plot.name == root.name)
            .unwrap();
        assert_eq!(root.source_document_id, checked.source_document_id.as_str());
        assert_eq!(root.checked_plot_id, checked_root.checked_plot_id.as_str());
        assert!(
            reviewed_inventory(&SOURCE.replace("with text/pattern/notation as r\n", "")).is_err()
        );
    }
}
