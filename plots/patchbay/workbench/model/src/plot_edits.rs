//! Bounded source-preserving semantic edits over the canonical Plot document.

use crate::plot_editor::{
    check_revision_with_catalog, ensure_source_bound, GraphItemKind, PlotEditor, PlotEditorError,
};
use crate::PatchbayGraph;

impl PlotEditor {
    /// Duplicates the exact authored gear statement with a fresh local name.
    pub fn duplicate_gear(
        &mut self,
        offered_revision: u64,
        gear_name: &str,
    ) -> Result<String, PlotEditorError> {
        self.require_revision(offered_revision)?;
        let plot = self.open_graph_plot()?;
        let prefix = format!("plot/{}/gear/", self.open_plot);
        let item = plot
            .items
            .iter()
            .find(|item| {
                item.kind == GraphItemKind::Gear
                    && item.identity.strip_prefix(&prefix) == Some(gear_name)
            })
            .ok_or_else(|| PlotEditorError::UnknownGear(gear_name.into()))?;
        let statement = self.source[item.source_span.start..item.source_span.end].to_owned();
        let colon = statement
            .find(':')
            .ok_or(PlotEditorError::InvalidGearName)?;
        let name = unique_gear_name(plot, gear_name)?;
        let replacement = format!("{name}{}", &statement[colon..]);
        let close = plot_close(&self.source, plot)?;
        let mut candidate = self.source.clone();
        candidate.insert_str(close, &format!("    {replacement}\n"));
        self.apply_candidate(candidate)?;
        Ok(name)
    }

    /// Removes one authored gear and every authored Cord statement that names it.
    pub fn remove_gear(
        &mut self,
        offered_revision: u64,
        gear_name: &str,
    ) -> Result<(), PlotEditorError> {
        self.require_revision(offered_revision)?;
        let plot = self.open_graph_plot()?;
        let prefix = format!("plot/{}/gear/", self.open_plot);
        let gear = plot
            .items
            .iter()
            .find(|item| {
                item.kind == GraphItemKind::Gear
                    && item.identity.strip_prefix(&prefix) == Some(gear_name)
            })
            .ok_or_else(|| PlotEditorError::UnknownGear(gear_name.into()))?;
        let mut ranges = vec![line_range(
            &self.source,
            gear.source_span.start,
            gear.source_span.end,
        )];
        for (cord, item) in plot.cords.iter().zip(
            plot.items
                .iter()
                .filter(|item| item.kind == GraphItemKind::Cord),
        ) {
            if cord.stages.iter().any(|stage| match stage {
                crate::GraphCordStage::Reference(reference) => {
                    reference == gear_name
                        || reference
                            .strip_prefix(gear_name)
                            .is_some_and(|suffix| suffix.starts_with('.'))
                }
                crate::GraphCordStage::TerminalProjection { endpoint, .. } => {
                    endpoint == gear_name
                        || endpoint
                            .strip_prefix(gear_name)
                            .is_some_and(|suffix| suffix.starts_with('.'))
                }
                crate::GraphCordStage::Cancellation { gear } => gear == gear_name,
                crate::GraphCordStage::RelationalGear { operands, .. } => {
                    operands.iter().any(|operand| {
                        operand == gear_name
                            || operand
                                .strip_prefix(gear_name)
                                .is_some_and(|suffix| suffix.starts_with('.'))
                    })
                }
                crate::GraphCordStage::InlineGear { .. }
                | crate::GraphCordStage::Literal
                | crate::GraphCordStage::When
                | crate::GraphCordStage::PureExpression
                | crate::GraphCordStage::StructuredSelector => false,
            }) {
                ranges.push(line_range(
                    &self.source,
                    item.source_span.start,
                    item.source_span.end,
                ));
            }
        }
        ranges.sort_unstable_by_key(|range| std::cmp::Reverse(range.0));
        let mut candidate = self.source.clone();
        for (start, end) in ranges {
            candidate.replace_range(start..end, "");
        }
        self.apply_candidate(candidate)
    }

    /// Creates one authored Cord after exact expanded-basis and typed-Port checks.
    pub fn connect_ports(
        &mut self,
        offered_revision: u64,
        offered_expanded_plot_id: &conduit_core::ExpandedPlotId,
        source_port_identity: &str,
        sink_port_identity: &str,
    ) -> Result<(), PlotEditorError> {
        self.require_revision(offered_revision)?;
        let graph = self.patchbay_graph_for_authoring(&self.open_plot)?;
        if &graph.expanded_plot_id != offered_expanded_plot_id {
            return Err(PlotEditorError::StaleGraphBasis);
        }
        let internal_source = graph
            .gears
            .iter()
            .flat_map(|gear| &gear.outputs)
            .find(|port| port.identity == source_port_identity);
        let front_source = graph
            .front_inputs
            .iter()
            .find(|port| port.identity == source_port_identity);
        let composition_source = graph.compositions.iter().find_map(|composition| {
            composition
                .outputs
                .iter()
                .find(|port| port.identity == source_port_identity)
                .map(|port| (composition, port))
        });
        let internal_sink = graph
            .gears
            .iter()
            .flat_map(|gear| &gear.inputs)
            .find(|port| port.identity == sink_port_identity);
        let front_sink = graph
            .front_outputs
            .iter()
            .find(|port| port.identity == sink_port_identity);
        let composition_sink = graph.compositions.iter().find_map(|composition| {
            composition
                .inputs
                .iter()
                .find(|port| port.identity == sink_port_identity)
                .map(|port| (composition, port))
        });
        require_compatible_connection(&graph, source_port_identity, sink_port_identity)?;
        let source_reference = if let Some(source) = internal_source {
            let source_name = direct_gear_name(&self.open_plot, source.gear_id.as_str())?;
            format!("{source_name}.{}", source.descriptor.port_id.as_str())
        } else if let Some((composition, port)) = composition_source {
            format!(
                "{}.{}",
                composition.gear_name,
                port.descriptor.port_id.as_str()
            )
        } else {
            front_source
                .expect("source compatibility was resolved")
                .descriptor
                .port_id
                .as_str()
                .to_owned()
        };
        let sink_reference = if let Some(sink) = internal_sink {
            let sink_name = direct_gear_name(&self.open_plot, sink.gear_id.as_str())?;
            format!("{sink_name}.{}", sink.descriptor.port_id.as_str())
        } else if let Some((composition, port)) = composition_sink {
            format!(
                "{}.{}",
                composition.gear_name,
                port.descriptor.port_id.as_str()
            )
        } else {
            front_sink
                .expect("sink compatibility was resolved")
                .descriptor
                .port_id
                .as_str()
                .to_owned()
        };
        let plot = self.open_graph_plot()?;
        let close = plot_close(&self.source, plot)?;
        let statement = format!("    {source_reference} >> {sink_reference}\n");
        let mut candidate = self.source.clone();
        candidate.insert_str(close, &statement);
        self.apply_candidate(candidate)
    }

    /// Removes one exact direct authored Cord from the current expanded graph.
    pub fn remove_cord(
        &mut self,
        offered_revision: u64,
        offered_expanded_plot_id: &conduit_core::ExpandedPlotId,
        cord_identity: &str,
    ) -> Result<(), PlotEditorError> {
        self.require_revision(offered_revision)?;
        let expanded = self.expand_plot(&self.open_plot)?;
        let graph = PatchbayGraph::from_expanded(&expanded)
            .map_err(|error| PlotEditorError::Catalog(error.to_string()))?;
        if &graph.expanded_plot_id != offered_expanded_plot_id {
            return Err(PlotEditorError::StaleGraphBasis);
        }
        let cord = graph
            .cords
            .iter()
            .find(|cord| cord.identity == cord_identity)
            .ok_or_else(|| PlotEditorError::UnknownCord(cord_identity.into()))?;
        let source = direct_port_reference(&self.open_plot, &cord.source_port, "output")?;
        let sink = direct_port_reference(&self.open_plot, &cord.sink_port, "input")?;
        let plot = self.open_graph_plot()?;
        let (cord, item) = plot
            .cords
            .iter()
            .zip(
                plot.items
                    .iter()
                    .filter(|item| item.kind == GraphItemKind::Cord),
            )
            .find(|(cord, _)| {
                cord.stages.as_slice()
                    == [
                        crate::GraphCordStage::Reference(source.clone()),
                        crate::GraphCordStage::Reference(sink.clone()),
                    ]
            })
            .ok_or_else(|| PlotEditorError::UnknownCord(cord_identity.into()))?;
        debug_assert_eq!(cord.stages.len(), 2);
        let (start, end) = line_range(&self.source, item.source_span.start, item.source_span.end);
        let mut candidate = self.source.clone();
        candidate.replace_range(start..end, "");
        self.apply_candidate(candidate)
    }

    /// Replaces either endpoint of one exact direct authored Cord after
    /// applying the same direction, Info, and temporal checks as connection.
    pub fn reroute_cord_endpoint(
        &mut self,
        offered_revision: u64,
        offered_expanded_plot_id: &conduit_core::ExpandedPlotId,
        cord_identity: &str,
        endpoint_port_identity: &str,
    ) -> Result<(), PlotEditorError> {
        self.require_revision(offered_revision)?;
        let expanded = self.expand_plot(&self.open_plot)?;
        let graph = PatchbayGraph::from_expanded(&expanded)
            .map_err(|error| PlotEditorError::Catalog(error.to_string()))?;
        if &graph.expanded_plot_id != offered_expanded_plot_id {
            return Err(PlotEditorError::StaleGraphBasis);
        }
        let cord = graph
            .cords
            .iter()
            .find(|cord| cord.identity == cord_identity)
            .ok_or_else(|| PlotEditorError::UnknownCord(cord_identity.into()))?;
        let old_source_port = graph
            .gears
            .iter()
            .flat_map(|gear| &gear.outputs)
            .find(|port| port.identity == cord.source_port)
            .ok_or_else(|| PlotEditorError::UnknownPort(cord.source_port.clone()))?;
        let old_sink_port = graph
            .gears
            .iter()
            .flat_map(|gear| &gear.inputs)
            .find(|port| port.identity == cord.sink_port)
            .ok_or_else(|| PlotEditorError::UnknownPort(cord.sink_port.clone()))?;
        let offered_source = graph
            .gears
            .iter()
            .flat_map(|gear| &gear.outputs)
            .find(|port| port.identity == endpoint_port_identity);
        let offered_sink = graph
            .gears
            .iter()
            .flat_map(|gear| &gear.inputs)
            .find(|port| port.identity == endpoint_port_identity);
        let (source_port, sink_port) = match (offered_source, offered_sink) {
            (Some(source), None) => (source, old_sink_port),
            (None, Some(sink)) => (old_source_port, sink),
            _ => return Err(PlotEditorError::UnknownPort(endpoint_port_identity.into())),
        };
        // The Cord being replaced must not count as a duplicate of itself.
        let mut candidate_graph = graph.clone();
        candidate_graph
            .cords
            .retain(|candidate| candidate.identity != cord_identity);
        require_compatible_connection(
            &candidate_graph,
            &source_port.identity,
            &sink_port.identity,
        )?;
        let old_source = direct_port_reference(&self.open_plot, &cord.source_port, "output")?;
        let old_sink = direct_port_reference(&self.open_plot, &cord.sink_port, "input")?;
        let plot = self.open_graph_plot()?;
        let item = plot
            .cords
            .iter()
            .zip(
                plot.items
                    .iter()
                    .filter(|item| item.kind == GraphItemKind::Cord),
            )
            .find(|(candidate, _)| {
                candidate.stages.as_slice()
                    == [
                        crate::GraphCordStage::Reference(old_source.clone()),
                        crate::GraphCordStage::Reference(old_sink.clone()),
                    ]
            })
            .map(|(_, item)| item)
            .ok_or_else(|| PlotEditorError::UnknownCord(cord_identity.into()))?;
        let source_name = direct_gear_name(&self.open_plot, source_port.gear_id.as_str())?;
        let sink_name = direct_gear_name(&self.open_plot, sink_port.gear_id.as_str())?;
        let statement = format!(
            "{source_name}.{} >> {sink_name}.{}",
            source_port.descriptor.port_id.as_str(),
            sink_port.descriptor.port_id.as_str()
        );
        let mut candidate = self.source.clone();
        candidate.replace_range(item.source_span.start..item.source_span.end, &statement);
        self.apply_candidate(candidate)
    }

    pub(crate) fn require_revision(&self, offered: u64) -> Result<(), PlotEditorError> {
        if offered != self.revision {
            return Err(PlotEditorError::StaleRevision {
                current: self.revision,
                offered,
            });
        }
        Ok(())
    }

    fn open_graph_plot(&self) -> Result<&crate::GraphPlot, PlotEditorError> {
        self.checked
            .plots
            .iter()
            .find(|plot| plot.name == self.open_plot)
            .ok_or_else(|| PlotEditorError::UnknownPlot(self.open_plot.clone()))
    }

    pub(crate) fn apply_candidate(&mut self, candidate: String) -> Result<(), PlotEditorError> {
        ensure_source_bound(&candidate)?;
        let next_revision = self.revision.saturating_add(1);
        let checked =
            check_revision_with_catalog(next_revision, &candidate, &self.startup_catalog)?;
        if let Some(diagnostic) = checked.diagnostics.first() {
            return Err(PlotEditorError::Catalog(diagnostic.message.clone()));
        }
        self.source = candidate;
        self.revision = next_revision;
        self.checked = checked;
        self.selection = None;
        Ok(())
    }
}

fn unique_gear_name(plot: &crate::GraphPlot, stem: &str) -> Result<String, PlotEditorError> {
    let prefix = format!("plot/{}/gear/", plot.name);
    for suffix in 2_u32..=u32::MAX {
        let candidate = format!("{stem}-{suffix}");
        if !plot
            .items
            .iter()
            .any(|item| item.identity.strip_prefix(&prefix) == Some(&candidate))
        {
            return Ok(candidate);
        }
    }
    Err(PlotEditorError::GraphTooLarge)
}

fn plot_close(source: &str, plot: &crate::GraphPlot) -> Result<usize, PlotEditorError> {
    source[plot.source_span.start..plot.source_span.end]
        .rfind('}')
        .map(|offset| plot.source_span.start + offset)
        .ok_or_else(|| PlotEditorError::UnknownPlot(plot.name.clone()))
}

fn direct_gear_name(plot: &str, gear_id: &str) -> Result<String, PlotEditorError> {
    let prefix = format!("{plot}/");
    let name = gear_id
        .strip_prefix(&prefix)
        .ok_or_else(|| PlotEditorError::UnknownGear(gear_id.into()))?;
    if name.contains('/') {
        return Err(PlotEditorError::NestedGearEditUnsupported(gear_id.into()));
    }
    Ok(name.into())
}

fn direct_port_reference(
    plot: &str,
    identity: &str,
    direction: &str,
) -> Result<String, PlotEditorError> {
    let prefix = format!("port/{plot}/");
    let suffix = identity
        .strip_prefix(&prefix)
        .ok_or_else(|| PlotEditorError::UnknownPort(identity.into()))?;
    let marker = format!("/{direction}/");
    let (gear, port) = suffix
        .split_once(&marker)
        .ok_or_else(|| PlotEditorError::UnknownPort(identity.into()))?;
    if gear.contains('/') || port.contains('/') || gear.is_empty() || port.is_empty() {
        return Err(PlotEditorError::NestedGearEditUnsupported(identity.into()));
    }
    Ok(format!("{gear}.{port}"))
}

fn line_range(source: &str, start: usize, end: usize) -> (usize, usize) {
    let line_start = source[..start].rfind('\n').map_or(0, |index| index + 1);
    let line_end = source[end..]
        .find('\n')
        .map_or(source.len(), |offset| end + offset + 1);
    (line_start, line_end)
}

fn require_compatible_connection(
    graph: &PatchbayGraph,
    source_port_identity: &str,
    sink_port_identity: &str,
) -> Result<(), PlotEditorError> {
    match graph.connection_compatibility(source_port_identity, sink_port_identity) {
        crate::PatchbayPortCompatibility::Compatible => {}
        crate::PatchbayPortCompatibility::DuplicateCord => {
            return Err(PlotEditorError::DuplicateCord)
        }
        crate::PatchbayPortCompatibility::IncompatibleInfo { source, sink } => {
            return Err(PlotEditorError::IncompatiblePorts(format!(
                "Info {} cannot feed {}",
                source.as_str(),
                sink.as_str()
            )))
        }
        crate::PatchbayPortCompatibility::IncompatibleTemporal { source, sink } => {
            return Err(PlotEditorError::IncompatiblePorts(format!(
                "temporal contract {source:?} cannot feed {sink:?}"
            )))
        }
        crate::PatchbayPortCompatibility::UnknownPort
        | crate::PatchbayPortCompatibility::InvalidDirection => {
            return Err(PlotEditorError::UnknownPort(format!(
                "{source_port_identity} >> {sink_port_identity}"
            )))
        }
    }
    Ok(())
}
