//! Revisioned canonical Plot source and a presentation-only checked graph.

use conduit_plot::{
    BackStatement, CheckedCordStage, CheckedSyntaxDocument, PlotSyntax, Span, StartupCatalog,
    SyntaxCheckDiagnostic,
};
use std::path::{Path, PathBuf};

use crate::plot_editor_catalogs::standard_catalogs;

#[path = "plot_editor_checking.rs"]
mod checking;
pub(crate) use checking::check_revision_with_catalog;

pub use crate::plot_editor_error::PlotEditorError;

const MAX_GRAPH_ITEMS: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorDiagnostic {
    pub code: &'static str,
    pub message: String,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphItemKind {
    FaceInput,
    FaceOutput,
    StartupValue,
    Gear,
    Cord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphItem {
    pub identity: String,
    pub label: String,
    pub kind: GraphItemKind,
    pub operation: Option<String>,
    pub source_span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphPlot {
    pub name: String,
    pub checked_plot_id: conduit_core::CheckedPlotId,
    pub front: conduit_core::CheckedFront,
    pub source_span: Span,
    pub items: Vec<GraphItem>,
    pub cords: Vec<GraphCord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphCord {
    pub identity: String,
    pub stages: Vec<GraphCordStage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphCordStage {
    Reference(String),
    RelationalGear {
        operands: Vec<String>,
        kind: String,
        input_ports: Vec<String>,
        output_port: String,
    },
    InlineGear {
        kind: String,
    },
    Literal,
    TerminalProjection {
        endpoint: String,
        terminal: conduit_plot::TerminalProjection,
    },
    Cancellation {
        gear: String,
    },
    When,
    PureExpression,
    StructuredSelector,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedRevision {
    pub revision: u64,
    pub source_document_id: Option<conduit_core::SourceDocumentId>,
    pub diagnostics: Vec<EditorDiagnostic>,
    pub plots: Vec<GraphPlot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSelection {
    pub identity: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlotDocumentView {
    pub revision: u64,
    pub saved_revision: u64,
    pub path: PathBuf,
    pub source: String,
    pub checked: CheckedRevision,
    pub open_plot: String,
    pub selection: Option<SourceSelection>,
}

#[derive(Clone)]
pub struct PlotEditor {
    pub(crate) path: PathBuf,
    pub(crate) source: String,
    pub(crate) revision: u64,
    pub(crate) saved_revision: u64,
    pub(crate) checked: CheckedRevision,
    pub(crate) open_plot: String,
    pub(crate) selection: Option<SourceSelection>,
    pub(crate) startup_catalog: StartupCatalog,
    pub(crate) profile_catalog: conduit_plot::ProfileCatalog,
}

impl PlotEditor {
    pub fn from_source(path: PathBuf, source: String) -> Result<Self, PlotEditorError> {
        let (startup, profile) = standard_catalogs()?;
        Self::from_source_with_catalogs(path, source, startup, profile)
    }

    pub fn from_source_with_catalogs(
        path: PathBuf,
        source: String,
        startup_catalog: StartupCatalog,
        profile_catalog: conduit_plot::ProfileCatalog,
    ) -> Result<Self, PlotEditorError> {
        validate_path(&path)?;
        ensure_source_bound(&source)?;
        let checked = check_revision_with_catalog(0, &source, &startup_catalog, &profile_catalog)?;
        let open_plot = checked
            .plots
            .first()
            .map(|plot| plot.name.clone())
            .unwrap_or_default();
        Ok(Self {
            path,
            source,
            revision: 0,
            saved_revision: 0,
            checked,
            open_plot,
            selection: None,
            startup_catalog,
            profile_catalog,
        })
    }

    pub fn replace_source(&mut self, source: String) -> Result<u64, PlotEditorError> {
        ensure_source_bound(&source)?;
        self.revision = self.revision.saturating_add(1);
        self.source = source;
        self.selection = None;
        Ok(self.revision)
    }

    /// Computes a result independently so an async host can publish it later.
    pub fn check_current(&self) -> Result<CheckedRevision, PlotEditorError> {
        check_revision_with_catalog(
            self.revision,
            &self.source,
            &self.startup_catalog,
            &self.profile_catalog,
        )
    }

    /// Immutable physical definitions admitted from this exact editor source.
    pub fn checked_physical_catalog(&self) -> Result<StartupCatalog, PlotEditorError> {
        let document = conduit_plot::parse_syntax_document(&self.source);
        conduit_plot::checked_physical_catalog_for_document(&document, &self.startup_catalog)
            .map_err(|error| PlotEditorError::Catalog(error.message))
    }

    pub fn publish_checked(&mut self, checked: CheckedRevision) -> Result<(), PlotEditorError> {
        if checked.revision != self.revision {
            return Err(PlotEditorError::StaleRevision {
                current: self.revision,
                offered: checked.revision,
            });
        }
        if !checked.plots.iter().any(|plot| plot.name == self.open_plot) {
            self.open_plot = checked
                .plots
                .first()
                .map(|plot| plot.name.clone())
                .unwrap_or_default();
        }
        self.checked = checked;
        Ok(())
    }

    pub fn recheck(&mut self) -> Result<(), PlotEditorError> {
        let checked = self.check_current()?;
        self.publish_checked(checked)
    }

    pub fn mark_saved(&mut self, revision: u64) -> Result<(), PlotEditorError> {
        if revision != self.revision {
            return Err(PlotEditorError::StaleRevision {
                current: self.revision,
                offered: revision,
            });
        }
        self.saved_revision = revision;
        Ok(())
    }

    pub fn open_back(&mut self, name: &str) -> Result<(), PlotEditorError> {
        if !self.checked.plots.iter().any(|plot| plot.name == name) {
            return Err(PlotEditorError::UnknownPlot(name.into()));
        }
        self.open_plot = name.into();
        self.selection = None;
        Ok(())
    }

    pub fn select_graph_item(&mut self, identity: &str) -> bool {
        let item = self
            .checked
            .plots
            .iter()
            .flat_map(|plot| &plot.items)
            .find(|item| item.identity == identity);
        self.selection = item.map(|item| SourceSelection {
            identity: item.identity.clone(),
            span: item.source_span,
        });
        self.selection.is_some()
    }

    pub fn select_source_span(&mut self, span: Span) -> bool {
        let item = self
            .checked
            .plots
            .iter()
            .flat_map(|plot| &plot.items)
            .find(|item| item.source_span == span);
        self.selection = item.map(|item| SourceSelection {
            identity: item.identity.clone(),
            span: item.source_span,
        });
        self.selection.is_some()
    }

    pub fn view(&self) -> PlotDocumentView {
        PlotDocumentView {
            revision: self.revision,
            saved_revision: self.saved_revision,
            path: self.path.clone(),
            source: self.source.clone(),
            checked: self.checked.clone(),
            open_plot: self.open_plot.clone(),
            selection: self.selection.clone(),
        }
    }

    pub fn expand_plot(
        &self,
        name: &str,
    ) -> Result<conduit_plot::ExpandedCanonicalPlot, PlotEditorError> {
        let (_, checked) =
            checking::checked_source(&self.source, &self.startup_catalog, &self.profile_catalog)
                .map_err(|diagnostic| PlotEditorError::Catalog(diagnostic.message))?;
        conduit_plot::expand_canonical_plot(&checked, name, &self.profile_catalog)
            .map_err(|diagnostic| PlotEditorError::Catalog(diagnostic.to_string()))
    }

    pub fn expand_plot_for_authoring(
        &self,
        name: &str,
    ) -> Result<conduit_plot::ExpandedAuthoringPlot, PlotEditorError> {
        let (_, checked) =
            checking::checked_source(&self.source, &self.startup_catalog, &self.profile_catalog)
                .map_err(|diagnostic| PlotEditorError::Catalog(diagnostic.message))?;
        conduit_plot::expand_canonical_plot_for_authoring(&checked, name, &self.profile_catalog)
            .map_err(|diagnostic| PlotEditorError::Catalog(diagnostic.to_string()))
    }

    pub fn patchbay_graph_for_authoring(
        &self,
        name: &str,
    ) -> Result<crate::PatchbayGraph, PlotEditorError> {
        let authoring = self.expand_plot_for_authoring(name)?;
        let mut graph = crate::PatchbayGraph::from_authoring(&authoring)
            .map_err(|error| PlotEditorError::Catalog(error.to_string()))?;
        let open = self
            .checked
            .plots
            .iter()
            .find(|plot| plot.name == name)
            .ok_or_else(|| PlotEditorError::UnknownPlot(name.into()))?;
        for item in open
            .items
            .iter()
            .filter(|item| item.kind == GraphItemKind::Gear)
        {
            let Some(back_name) = item.operation.as_deref() else {
                continue;
            };
            let Some(back) = self
                .checked
                .plots
                .iter()
                .find(|plot| plot.name == back_name)
            else {
                continue;
            };
            let gear_name = item
                .identity
                .rsplit('/')
                .next()
                .expect("graph Gear identity has a final name")
                .to_owned();
            let gear_id = conduit_core::GearId::from(format!("{name}/{gear_name}"));
            let nested = self.expand_plot_for_authoring(back_name)?;
            let inputs = back
                .front
                .inputs()
                .iter()
                .cloned()
                .map(|descriptor| crate::PatchbayFrontPort {
                    identity: format!(
                        "composition/{gear_name}/input/{}",
                        descriptor.port_id.as_str()
                    ),
                    value_contract: back
                        .front
                        .value_contract(&conduit_core::FrontValueLocation::Input(
                            descriptor.port_id.clone(),
                        ))
                        .cloned(),
                    descriptor,
                })
                .collect::<Vec<_>>();
            let outputs = back
                .front
                .outputs()
                .iter()
                .cloned()
                .map(|descriptor| crate::PatchbayFrontPort {
                    identity: format!(
                        "composition/{gear_name}/output/{}",
                        descriptor.port_id.as_str()
                    ),
                    value_contract: back
                        .front
                        .value_contract(&conduit_core::FrontValueLocation::Output(
                            descriptor.port_id.clone(),
                        ))
                        .cloned(),
                    descriptor,
                })
                .collect::<Vec<_>>();
            let translated_port = |binding: &conduit_plot::AuthoringFrontBinding, direction| {
                let suffix = binding
                    .gear_id
                    .as_str()
                    .strip_prefix(back_name)
                    .unwrap_or(binding.gear_id.as_str());
                format!(
                    "port/{}{suffix}/{direction}/{}",
                    gear_id.as_str(),
                    binding.gear_port_id.as_str()
                )
            };
            graph
                .admit_composition(crate::PatchbayComposition {
                    identity: format!("composition/{gear_name}"),
                    gear_name: gear_name.clone(),
                    back_name: back_name.into(),
                    checked_plot_id: back.checked_plot_id.clone(),
                    input_bindings: nested
                        .input_bindings
                        .iter()
                        .map(|binding| crate::PatchbayCompositionBinding {
                            front_port: format!(
                                "composition/{gear_name}/input/{}",
                                binding.front_port_id.as_str()
                            ),
                            internal_port: translated_port(binding, "input"),
                        })
                        .collect(),
                    output_bindings: nested
                        .output_bindings
                        .iter()
                        .map(|binding| crate::PatchbayCompositionBinding {
                            front_port: format!(
                                "composition/{gear_name}/output/{}",
                                binding.front_port_id.as_str()
                            ),
                            internal_port: translated_port(binding, "output"),
                        })
                        .collect(),
                    inputs,
                    outputs,
                })
                .map_err(|_| PlotEditorError::GraphTooLarge)?;
        }
        Ok(graph)
    }
}

fn graph_revision(
    revision: u64,
    syntax_plots: &[PlotSyntax],
    checked: CheckedSyntaxDocument,
) -> Result<CheckedRevision, PlotEditorError> {
    let mut plots = Vec::with_capacity(checked.plots.len());
    for plot in &checked.plots {
        let syntax = syntax_plots
            .iter()
            .find(|candidate| candidate.name.text == plot.name)
            .expect("checked plots retain parsed names");
        let mut items = Vec::new();
        let mut cords = Vec::new();
        for parameter in &syntax.front.startup_parameters {
            push_item(
                &mut items,
                &plot.name,
                "startup",
                &parameter.name.text,
                GraphItemKind::StartupValue,
                parameter.span,
            )?;
        }
        for port in &syntax.front.runtime_ports {
            let kind = match port.direction {
                conduit_plot::RuntimePortDirection::Input => GraphItemKind::FaceInput,
                conduit_plot::RuntimePortDirection::Output => GraphItemKind::FaceOutput,
            };
            push_item(
                &mut items,
                &plot.name,
                "port",
                &port.name.text,
                kind,
                port.span,
            )?;
        }
        let mut cord_index = 0;
        for statement in &syntax.back {
            match statement {
                BackStatement::NamedGear(gear) => {
                    push_item(
                        &mut items,
                        &plot.name,
                        "gear",
                        &gear.name.text,
                        GraphItemKind::Gear,
                        gear.span,
                    )?;
                    let operation = plot
                        .gears
                        .iter()
                        .find(|checked_gear| checked_gear.name.as_deref() == Some(&gear.name.text))
                        .map(|checked_gear| checked_gear.kind.as_str())
                        .unwrap_or("unknown");
                    items.last_mut().expect("gear item was admitted").label =
                        format!("{}: {operation}", gear.name.text);
                    items.last_mut().expect("gear item was admitted").operation =
                        Some(operation.into());
                }
                BackStatement::Cord(cord) => {
                    let label = plot
                        .cords
                        .get(cord_index)
                        .map(cord_label)
                        .unwrap_or_else(|| "cord".into());
                    push_item(
                        &mut items,
                        &plot.name,
                        "cord",
                        &cord_index.to_string(),
                        GraphItemKind::Cord,
                        cord.span,
                    )?;
                    if let Some(item) = items.last_mut() {
                        item.label = label;
                    }
                    if let Some(checked_cord) = plot.cords.get(cord_index) {
                        cords.push(GraphCord {
                            identity: items
                                .last()
                                .expect("cord item was admitted")
                                .identity
                                .clone(),
                            stages: checked_cord.stages.iter().map(graph_cord_stage).collect(),
                        });
                    }
                    cord_index += 1;
                }
                BackStatement::MatchedRoute(route) => {
                    for arm in &route.arms {
                        let label = plot
                            .cords
                            .get(cord_index)
                            .map(cord_label)
                            .unwrap_or_else(|| "matched route track".into());
                        push_item(
                            &mut items,
                            &plot.name,
                            "cord",
                            &cord_index.to_string(),
                            GraphItemKind::Cord,
                            arm.span,
                        )?;
                        if let Some(item) = items.last_mut() {
                            item.label = label;
                        }
                        if let Some(checked_cord) = plot.cords.get(cord_index) {
                            cords.push(GraphCord {
                                identity: items
                                    .last()
                                    .expect("route track item was admitted")
                                    .identity
                                    .clone(),
                                stages: checked_cord.stages.iter().map(graph_cord_stage).collect(),
                            });
                        }
                        cord_index += 1;
                    }
                }
                BackStatement::Pool(_) | BackStatement::LocalValue(_) => {}
            }
        }
        plots.push(GraphPlot {
            name: plot.name.clone(),
            checked_plot_id: plot.checked_plot_id.clone(),
            front: plot.checked_front(),
            source_span: syntax.span,
            items,
            cords,
        });
    }
    Ok(CheckedRevision {
        revision,
        source_document_id: Some(checked.source_document_id),
        diagnostics: Vec::new(),
        plots,
    })
}

fn cord_label(cord: &conduit_plot::CheckedCanonicalCord) -> String {
    cord.stages
        .iter()
        .map(|stage| match stage {
            CheckedCordStage::Reference(name) => name.clone(),
            CheckedCordStage::RelationalGear { operands, gear, .. } => {
                format!("{}({})", gear.kind, operands.join(", "))
            }
            CheckedCordStage::TerminalProjection {
                endpoint, terminal, ..
            } => format!(
                "{endpoint}{}",
                match terminal {
                    conduit_plot::TerminalProjection::NormalClose => "|",
                    conduit_plot::TerminalProjection::Abnormal => "!",
                    conduit_plot::TerminalProjection::Quiescence => ";",
                }
            ),
            CheckedCordStage::Cancellation { gear, .. } => format!("{gear}~"),
            CheckedCordStage::When { .. } => "when(...)".into(),
            CheckedCordStage::PureExpression { .. } => "(expression)".into(),
            CheckedCordStage::InlineGear(gear) => gear.kind.clone(),
            CheckedCordStage::Literal { value, .. } => format!("{value:?}"),
            CheckedCordStage::StructuredSelector { .. } => "structured selector".into(),
        })
        .collect::<Vec<_>>()
        .join(" >> ")
}

fn graph_cord_stage(stage: &CheckedCordStage) -> GraphCordStage {
    match stage {
        CheckedCordStage::Reference(name) => GraphCordStage::Reference(name.clone()),
        CheckedCordStage::RelationalGear {
            operands,
            gear,
            input_ports,
            output_port,
        } => GraphCordStage::RelationalGear {
            operands: operands.clone(),
            kind: gear.kind.clone(),
            input_ports: input_ports.clone(),
            output_port: output_port.clone(),
        },
        CheckedCordStage::TerminalProjection {
            endpoint, terminal, ..
        } => GraphCordStage::TerminalProjection {
            endpoint: endpoint.clone(),
            terminal: *terminal,
        },
        CheckedCordStage::Cancellation { gear, .. } => {
            GraphCordStage::Cancellation { gear: gear.clone() }
        }
        CheckedCordStage::When { .. } => GraphCordStage::When,
        CheckedCordStage::PureExpression { .. } => GraphCordStage::PureExpression,
        CheckedCordStage::InlineGear(gear) => GraphCordStage::InlineGear {
            kind: gear.kind.clone(),
        },
        CheckedCordStage::Literal { .. } => GraphCordStage::Literal,
        CheckedCordStage::StructuredSelector { .. } => GraphCordStage::StructuredSelector,
    }
}

fn push_item(
    items: &mut Vec<GraphItem>,
    plot: &str,
    class: &str,
    name: &str,
    kind: GraphItemKind,
    source_span: Span,
) -> Result<(), PlotEditorError> {
    if items.len() == MAX_GRAPH_ITEMS {
        return Err(PlotEditorError::GraphTooLarge);
    }
    items.push(GraphItem {
        identity: format!("plot/{plot}/{class}/{name}"),
        label: name.into(),
        kind,
        operation: None,
        source_span,
    });
    Ok(())
}

fn check_error_revision(revision: u64, diagnostic: SyntaxCheckDiagnostic) -> CheckedRevision {
    invalid_revision(
        revision,
        diagnostic.code,
        &diagnostic.message,
        diagnostic.span,
    )
}

fn invalid_revision(
    revision: u64,
    code: &'static str,
    message: &str,
    span: Span,
) -> CheckedRevision {
    CheckedRevision {
        revision,
        source_document_id: None,
        diagnostics: vec![EditorDiagnostic {
            code,
            message: message.into(),
            span,
        }],
        plots: Vec::new(),
    }
}

fn validate_path(path: &Path) -> Result<(), PlotEditorError> {
    if path.extension().and_then(|extension| extension.to_str()) != Some("conduit") {
        return Err(PlotEditorError::NotCanonicalPlotPath);
    }
    Ok(())
}

pub(crate) fn ensure_source_bound(source: &str) -> Result<(), PlotEditorError> {
    if source.len() > conduit_plot::MAXIMUM_PLOT_SOURCE_BYTES {
        Err(PlotEditorError::SourceTooLarge)
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "plot_editor_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "plot_editor_glyph_tests.rs"]
mod glyph_tests;
