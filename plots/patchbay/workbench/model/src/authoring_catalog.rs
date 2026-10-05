//! Read-only authoring queries over the editor's actual checked source contracts.

use conduit_core::{ConfigurationValue, ExpandedPlotId};
use conduit_semantic_catalog::{GearPalette, PaletteEntry};
use serde::{Deserialize, Serialize};

use crate::{PlotEditor, PlotEditorError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringStartupParameter {
    pub name: String,
    pub value_type: String,
    pub default: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoringKind {
    pub contract: PaletteEntry,
    pub authorable: bool,
    pub startup_parameters: Vec<AuthoringStartupParameter>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringConnection {
    pub sink_identity: String,
    pub compatible: bool,
    pub diagnostic: Option<String>,
    pub adapters: Vec<AuthoringAdapter>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringAdapter {
    pub kind_id: String,
    pub kind_contract_revision: String,
    pub input_port: String,
    pub output_port: String,
}

impl PlotEditor {
    /// Catalog visibility is independent of whether any current Host offers a Back.
    pub fn authoring_catalog(&self) -> Result<Vec<AuthoringKind>, PlotEditorError> {
        let palette = GearPalette::standard()
            .map_err(|error| PlotEditorError::Catalog(format!("{error:?}")))?;
        Ok(palette
            .entries()
            .iter()
            .map(|entry| {
                let signature = self.startup_catalog.signature(entry.kind_id.as_str());
                AuthoringKind {
                    contract: entry.clone(),
                    authorable: signature.is_some()
                        && self
                            .profile_catalog
                            .get(&entry.kind_id)
                            .is_some_and(|kind| {
                                kind.kind_contract_revision == entry.kind_contract_revision
                                    && kind.inputs == entry.inputs
                                    && kind.outputs == entry.outputs
                            }),
                    startup_parameters: signature
                        .into_iter()
                        .flat_map(|signature| &signature.startup_parameters)
                        .map(|parameter| AuthoringStartupParameter {
                            name: parameter.name.clone(),
                            value_type: parameter.value_type.clone(),
                            default: parameter.default.clone(),
                        })
                        .collect(),
                }
            })
            .collect())
    }

    /// Preflight each destination through the very same semantic edit used to
    /// commit a Cord. No successful query mutates this editor or its revision.
    pub fn authoring_connections(
        &self,
        revision: u64,
        expanded_plot_id: &ExpandedPlotId,
        source: &str,
    ) -> Result<Vec<AuthoringConnection>, PlotEditorError> {
        self.require_revision(revision)?;
        let graph = self.patchbay_graph_for_authoring(&self.open_plot)?;
        if &graph.expanded_plot_id != expanded_plot_id {
            return Err(PlotEditorError::StaleGraphBasis);
        }
        if !graph
            .subject_identities()
            .any(|identity| identity == source)
        {
            return Err(PlotEditorError::UnknownPort(source.into()));
        }
        let palette = GearPalette::standard()
            .map_err(|error| PlotEditorError::Catalog(format!("{error:?}")))?;
        let descriptor = |identity: &str| {
            graph
                .gears
                .iter()
                .flat_map(|gear| gear.inputs.iter().chain(&gear.outputs))
                .map(|port| (port.identity.as_str(), &port.descriptor))
                .chain(
                    graph
                        .front_inputs
                        .iter()
                        .chain(&graph.front_outputs)
                        .map(|port| (port.identity.as_str(), &port.descriptor)),
                )
                .chain(
                    graph
                        .compositions
                        .iter()
                        .flat_map(|composition| {
                            composition.inputs.iter().chain(&composition.outputs)
                        })
                        .map(|port| (port.identity.as_str(), &port.descriptor)),
                )
                .find_map(|(id, descriptor)| (id == identity).then_some(descriptor))
        };
        Ok(graph
            .connection_candidates(source)
            .into_iter()
            .map(|candidate| {
                let result = self.clone().connect_ports(
                    revision,
                    expanded_plot_id,
                    source,
                    &candidate.sink_identity,
                );
                AuthoringConnection {
                    adapters: descriptor(source)
                        .zip(descriptor(&candidate.sink_identity))
                        .into_iter()
                        .flat_map(|(source, sink)| palette.adapter_suggestions(source, sink))
                        .filter(|adapter| {
                            self.startup_catalog
                                .signature(adapter.kind_id.as_str())
                                .is_some()
                        })
                        .map(|adapter| AuthoringAdapter {
                            kind_id: adapter.kind_id.as_str().into(),
                            kind_contract_revision: adapter.kind_contract_revision.as_str().into(),
                            input_port: adapter.input.port_id.as_str().into(),
                            output_port: adapter.output.port_id.as_str().into(),
                        })
                        .collect(),
                    sink_identity: candidate.sink_identity,
                    compatible: result.is_ok(),
                    diagnostic: result.err().map(|error| error.to_string()),
                }
            })
            .collect())
    }

    pub fn validate_authoring_configuration(
        &self,
        revision: u64,
        expanded_plot_id: &ExpandedPlotId,
        gear: &str,
        key: &str,
        value: ConfigurationValue,
    ) -> Result<(), PlotEditorError> {
        self.clone()
            .set_gear_configuration(revision, expanded_plot_id, gear, key, value)
    }
}
