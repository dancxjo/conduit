//! Bounded read-only Patchbay facts for one checked Plot.
//!
//! This projection is produced from the same production parse, check, and
//! expansion path used before Tour execution. It contains no renderer geometry,
//! Host offer, placement, implementation, Plan, Play, or mutable editor state.

use crate::installed_browser::{
    backs, catalogs_for_presentation, PresentationProfile, MAXIMUM_BROWSER_PLOT_CORDS,
    MAXIMUM_BROWSER_PLOT_GEARS,
};
use conduit_core::{CheckedFront, CheckedValueContract, FrontValueLocation, PortDirection};
use conduit_plot::ExpandedCanonicalPlot;
use serde::Serialize;

const MAXIMUM_COMPACT_PORTS: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct CompactPatchbayProjection {
    pub(super) schema: &'static str,
    pub(super) sequence: u64,
    pub(super) source_proposal_id: String,
    pub(super) source_document_id: String,
    pub(super) checked_plot_id: String,
    pub(super) visible_expanded_plot_id: String,
    pub(super) realization_expanded_plot_id: String,
    pub(super) plot_name: String,
    pub(super) realization: &'static str,
    pub(super) front_inputs: Vec<CompactPort>,
    pub(super) front_outputs: Vec<CompactPort>,
    pub(super) gears: Vec<CompactGear>,
    pub(super) cords: Vec<CompactCord>,
    pub(super) realization_gears: Vec<CompactGear>,
    pub(super) realization_cords: Vec<CompactCord>,
    pub(super) realization_backs: Vec<CompactBack>,
    pub(super) diagnostics: Vec<CompactDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct CompactGear {
    pub(super) gear_id: String,
    pub(super) kind_id: String,
    pub(super) inputs: Vec<CompactPort>,
    pub(super) outputs: Vec<CompactPort>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct CompactPort {
    pub(super) port_id: String,
    pub(super) info_kind: String,
    pub(super) temporal: &'static str,
    pub(super) value_contract: Option<CheckedValueContract>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct CompactCord {
    pub(super) source_gear_id: String,
    pub(super) source_port_id: String,
    pub(super) sink_gear_id: String,
    pub(super) sink_port_id: String,
    pub(super) info_kind: String,
    pub(super) temporal: &'static str,
    pub(super) invalid: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct CompactDiagnostic {
    pub(super) code: &'static str,
    pub(super) message: String,
    pub(super) fix: String,
    pub(super) subjects: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct CompactBack {
    pub(super) invocation_path: String,
    pub(super) kind_id: String,
    pub(super) checked_plot_id: String,
}

pub(super) fn project(
    source: &str,
    sequence: u64,
    recursive: bool,
) -> Result<CompactPatchbayProjection, String> {
    project_with_schema(
        source,
        sequence,
        recursive,
        PresentationProfile::Annotation,
        "conduit.tour/compact-patchbay@1",
    )
}

/// Product-neutral checked-Plot projection for supported external consumers.
/// Tour and the browser SDK share the same Rust checking/expansion path; the
/// schema states which public contract the caller requested.
pub(super) fn project_plot(
    source: &str,
    sequence: u64,
) -> Result<CompactPatchbayProjection, String> {
    project_with_schema(
        source,
        sequence,
        false,
        PresentationProfile::Annotation,
        "conduit.patchbay/checked-plot-projection@1",
    )
}

pub(super) fn project_with_presentation(
    source: &str,
    sequence: u64,
    recursive: bool,
    presentation: PresentationProfile,
) -> Result<CompactPatchbayProjection, String> {
    project_with_schema(
        source,
        sequence,
        recursive,
        presentation,
        "conduit.tour/compact-patchbay@1",
    )
}

fn project_with_schema(
    source: &str,
    sequence: u64,
    recursive: bool,
    presentation: PresentationProfile,
    schema: &'static str,
) -> Result<CompactPatchbayProjection, String> {
    let interaction = crate::source_interaction::admit_source(source.as_bytes(), sequence)?;
    let (startup, mut catalog) = catalogs_for_presentation(presentation)?;
    let syntax = conduit_plot::parse_syntax_document(source);
    if let Some(diagnostic) = syntax.diagnostics.first() {
        return Err(format!(
            "parse checked-Plot Patchbay: {}",
            diagnostic.message
        ));
    }
    let checked = conduit_plot::check_syntax_document(&syntax, &startup)
        .map_err(|error| format!("check checked-Plot Patchbay: {error:?}"))?;
    crate::installed_browser::catalogs::install_checked_structured_selectors(
        &checked,
        &mut catalog,
    )?;
    // Closed listings must show the exact root selected for Play, even when
    // reusable open Plots follow it. A source containing only open Plots is
    // still inspectable as an authoring surface.
    let entry = super::executable_entry(&checked).or_else(|_| {
        checked
            .plots
            .last()
            .map(|plot| plot.name.clone())
            .ok_or_else(|| "checked-Plot Patchbay source has no Plot".to_owned())
    })?;

    // The visible graph is authored meaning. A recursive realization may have a
    // different expanded identity and Back evidence, but it cannot replace the
    // checked gear/Port/Cord front shown beside the source.
    let visible =
        match conduit_plot::expand_canonical_plot_for_authoring(&checked, &entry, &catalog) {
            Ok(visible) => visible,
            Err(error) if error.code == "CND-FRM-045" => {
                return project_incompatible_cord(
                    schema,
                    interaction.proposal_identity,
                    sequence,
                    &checked,
                    &entry,
                    &catalog,
                    error,
                )
            }
            Err(error) => return Err(format!("expand checked-Plot Patchbay: {error:?}")),
        };
    admit_topology(&visible.expanded)?;
    let realized = recursive
        .then(|| {
            conduit_plot::expand_canonical_plot_for_authoring_with_backs(
                &checked,
                &entry,
                &catalog,
                &backs(&startup, &catalog)?,
            )
            .map(|realized| realized.expanded)
            .map_err(|error| format!("expand recursive checked-Plot Patchbay: {error:?}"))
        })
        .transpose()?;
    let realization = realized.as_ref().unwrap_or(&visible.expanded);
    admit_topology(realization)?;

    Ok(CompactPatchbayProjection {
        schema,
        sequence,
        source_proposal_id: interaction.proposal_identity,
        source_document_id: visible.expanded.source_document_id.as_str().into(),
        checked_plot_id: visible.expanded.checked_plot_id.as_str().into(),
        visible_expanded_plot_id: visible.expanded.expanded_plot_id.as_str().into(),
        realization_expanded_plot_id: realization.expanded_plot_id.as_str().into(),
        plot_name: visible.expanded.name.clone(),
        realization: if recursive { "recursive" } else { "direct" },
        front_inputs: front_ports(&visible.front, PortDirection::Input),
        front_outputs: front_ports(&visible.front, PortDirection::Output),
        gears: visible
            .expanded
            .gears
            .iter()
            .map(|gear| CompactGear {
                gear_id: gear.gear_id.as_str().into(),
                kind_id: gear.kind_id.as_str().into(),
                inputs: front_ports(&gear.checked_front(), PortDirection::Input),
                outputs: front_ports(&gear.checked_front(), PortDirection::Output),
            })
            .collect(),
        cords: visible
            .expanded
            .connections
            .iter()
            .map(|cord| CompactCord {
                source_gear_id: cord.source_gear_id.as_str().into(),
                source_port_id: cord.source_port_id.as_str().into(),
                sink_gear_id: cord.sink_gear_id.as_str().into(),
                sink_port_id: cord.sink_port_id.as_str().into(),
                info_kind: cord.value_kind.as_str().into(),
                temporal: cord.temporal.as_str(),
                invalid: false,
            })
            .collect(),
        realization_gears: realization
            .gears
            .iter()
            .map(|gear| CompactGear {
                gear_id: gear.gear_id.as_str().into(),
                kind_id: gear.kind_id.as_str().into(),
                inputs: front_ports(&gear.checked_front(), PortDirection::Input),
                outputs: front_ports(&gear.checked_front(), PortDirection::Output),
            })
            .collect(),
        realization_cords: realization
            .connections
            .iter()
            .map(|cord| CompactCord {
                source_gear_id: cord.source_gear_id.as_str().into(),
                source_port_id: cord.source_port_id.as_str().into(),
                sink_gear_id: cord.sink_gear_id.as_str().into(),
                sink_port_id: cord.sink_port_id.as_str().into(),
                info_kind: cord.value_kind.as_str().into(),
                temporal: cord.temporal.as_str(),
                invalid: false,
            })
            .collect(),
        realization_backs: realization
            .realization_backs
            .iter()
            .map(|back| CompactBack {
                invocation_path: back.invocation_path.clone(),
                kind_id: back.kind_id.as_str().into(),
                checked_plot_id: back.checked_plot_id.as_str().into(),
            })
            .collect(),
        diagnostics: Vec::new(),
    })
}

fn project_incompatible_cord(
    schema: &'static str,
    source_proposal_id: String,
    sequence: u64,
    checked: &conduit_plot::CheckedSyntaxDocument,
    entry: &str,
    catalog: &conduit_plot::ProfileCatalog,
    error: conduit_plot::CanonicalExpansionDiagnostic,
) -> Result<CompactPatchbayProjection, String> {
    let plot = checked
        .plots
        .iter()
        .find(|plot| plot.name == entry)
        .ok_or_else(|| "Patchbay checked Plot disappeared".to_owned())?;
    let prefix = format!("{entry}/");
    let mut gears = Vec::new();
    for gear in &plot.gears {
        let name = gear
            .name
            .as_deref()
            .ok_or_else(|| "invalid compact Patchbay draft contains an inline Gear".to_owned())?;
        let definition = catalog
            .get(&conduit_core::KindId::new(gear.kind.clone()))
            .ok_or_else(|| format!("invalid compact Patchbay Kind '{}' disappeared", gear.kind))?;
        gears.push(CompactGear {
            gear_id: format!("{prefix}{name}"),
            kind_id: gear.kind.clone(),
            inputs: descriptor_ports(&definition.inputs),
            outputs: descriptor_ports(&definition.outputs),
        });
    }
    let mut cords = Vec::new();
    let mut diagnostic = None;
    for cord in &plot.cords {
        for pair in cord.stages.windows(2) {
            let [conduit_plot::CheckedCordStage::Reference(source_name), conduit_plot::CheckedCordStage::Reference(sink_name)] =
                pair
            else {
                return Err(
                    "invalid compact Patchbay draft contains a non-reference Cord stage".into(),
                );
            };
            let source = gears
                .iter()
                .find(|gear| gear.gear_id == format!("{prefix}{source_name}"))
                .ok_or_else(|| {
                    format!("invalid compact Patchbay source Gear '{source_name}' disappeared")
                })?;
            let sink = gears
                .iter()
                .find(|gear| gear.gear_id == format!("{prefix}{sink_name}"))
                .ok_or_else(|| {
                    format!("invalid compact Patchbay sink Gear '{sink_name}' disappeared")
                })?;
            let output = source
                .outputs
                .first()
                .ok_or_else(|| format!("Gear '{source_name}' has no output"))?;
            let input = sink
                .inputs
                .first()
                .ok_or_else(|| format!("Gear '{sink_name}' has no input"))?;
            let invalid = output.info_kind != input.info_kind || output.temporal != input.temporal;
            let cord_index = cords.len();
            cords.push(CompactCord {
                source_gear_id: source.gear_id.clone(),
                source_port_id: output.port_id.clone(),
                sink_gear_id: sink.gear_id.clone(),
                sink_port_id: input.port_id.clone(),
                info_kind: output.info_kind.clone(),
                temporal: output.temporal,
                invalid,
            });
            if invalid && diagnostic.is_none() {
                diagnostic = Some(CompactDiagnostic {
                    code: error.code,
                    message: error.message.clone(),
                    fix: format!(
                        "Replace '{sink_name}' with a gear whose input is {} ({}) or change '{source_name}' to emit {} ({}).",
                        output.info_kind, output.temporal, input.info_kind, input.temporal
                    ),
                    subjects: vec![
                        format!("cord:{cord_index}:{}.{}->{}.{}", source.gear_id, output.port_id, sink.gear_id, input.port_id),
                        source.gear_id.clone(),
                        format!("{}.emitting:{}", source.gear_id, output.port_id),
                        sink.gear_id.clone(),
                        format!("{}.receiving:{}", sink.gear_id, input.port_id),
                    ],
                });
            }
        }
    }
    let diagnostic =
        diagnostic.ok_or_else(|| format!("expand checked-Plot Patchbay: {error:?}"))?;
    admit_draft_topology(&gears, &cords)?;
    Ok(CompactPatchbayProjection {
        schema,
        sequence,
        source_proposal_id,
        source_document_id: checked.source_document_id.as_str().into(),
        checked_plot_id: plot.checked_plot_id.as_str().into(),
        visible_expanded_plot_id: String::new(),
        realization_expanded_plot_id: String::new(),
        plot_name: plot.name.clone(),
        realization: "invalid-source-proposal",
        front_inputs: front_ports(&plot.runtime_front, PortDirection::Input),
        front_outputs: front_ports(&plot.runtime_front, PortDirection::Output),
        realization_gears: gears.clone(),
        realization_cords: cords.clone(),
        gears,
        cords,
        realization_backs: Vec::new(),
        diagnostics: vec![diagnostic],
    })
}

fn admit_draft_topology(gears: &[CompactGear], cords: &[CompactCord]) -> Result<(), String> {
    if gears.len() > MAXIMUM_BROWSER_PLOT_GEARS || cords.len() > MAXIMUM_BROWSER_PLOT_CORDS {
        return Err("invalid compact Patchbay draft exceeds its topology bound".into());
    }
    let ports = gears.iter().try_fold(0usize, |count, gear| {
        count
            .checked_add(gear.inputs.len())?
            .checked_add(gear.outputs.len())
    });
    if ports.is_none_or(|count| count > MAXIMUM_COMPACT_PORTS) {
        return Err("invalid compact Patchbay draft exceeds its Port bound".into());
    }
    Ok(())
}

fn admit_topology(plot: &ExpandedCanonicalPlot) -> Result<(), String> {
    if plot.gears.len() > MAXIMUM_BROWSER_PLOT_GEARS {
        return Err(format!(
            "checked-Plot Patchbay Gear bound exceeded: {} > {MAXIMUM_BROWSER_PLOT_GEARS}",
            plot.gears.len()
        ));
    }
    if plot.connections.len() > MAXIMUM_BROWSER_PLOT_CORDS {
        return Err(format!(
            "checked-Plot Patchbay Cord bound exceeded: {} > {MAXIMUM_BROWSER_PLOT_CORDS}",
            plot.connections.len()
        ));
    }
    let ports = plot.gears.iter().try_fold(0usize, |count, gear| {
        count
            .checked_add(gear.inputs.len())?
            .checked_add(gear.outputs.len())
    });
    if ports.is_none_or(|count| count > MAXIMUM_COMPACT_PORTS) {
        return Err("checked-Plot Patchbay Port bound exceeded".into());
    }
    Ok(())
}

fn front_ports(front: &CheckedFront, direction: PortDirection) -> Vec<CompactPort> {
    let descriptors = match direction {
        PortDirection::Input => front.inputs(),
        PortDirection::Output => front.outputs(),
    };
    descriptors
        .iter()
        .map(|descriptor| {
            let location = match direction {
                PortDirection::Input => FrontValueLocation::Input(descriptor.port_id.clone()),
                PortDirection::Output => FrontValueLocation::Output(descriptor.port_id.clone()),
            };
            CompactPort {
                port_id: descriptor.port_id.as_str().into(),
                info_kind: descriptor.value_kind.as_str().into(),
                temporal: descriptor.temporal.as_str(),
                value_contract: front.value_contract(&location).cloned(),
            }
        })
        .collect()
}

fn descriptor_ports(descriptors: &[conduit_core::PortDescriptor]) -> Vec<CompactPort> {
    descriptors
        .iter()
        .map(|descriptor| CompactPort {
            port_id: descriptor.port_id.as_str().into(),
            info_kind: descriptor.value_kind.as_str().into(),
            temporal: descriptor.temporal.as_str(),
            value_contract: None,
        })
        .collect()
}

#[cfg(test)]
#[path = "compact_patchbay_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "gallery_projection_tests.rs"]
mod gallery_projection_tests;
