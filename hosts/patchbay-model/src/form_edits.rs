//! Bounded source-preserving semantic edits over the canonical Form document.

use crate::form_editor::{
    check_revision, ensure_source_bound, FormEditor, FormEditorError, GraphItemKind,
};
use crate::PatchbayGraph;

impl FormEditor {
    /// Places one fresh semantic Gear by editing canonical Form source. The
    /// offered revision is a stale-gesture precondition; no model state changes
    /// unless the resulting source parses and checks successfully.
    pub fn place_palette_kind(
        &mut self,
        offered_revision: u64,
        kind_id: &conduit_core::KindId,
    ) -> Result<String, FormEditorError> {
        if offered_revision != self.revision {
            return Err(FormEditorError::StaleRevision {
                current: self.revision,
                offered: offered_revision,
            });
        }
        let palette = crate::GearPalette::standard()
            .map_err(|error| FormEditorError::Catalog(format!("{error:?}")))?;
        if palette.find(kind_id).is_none() {
            return Err(FormEditorError::UnknownPaletteKind(kind_id.as_str().into()));
        }
        let form = self
            .checked
            .forms
            .iter()
            .find(|form| form.name == self.open_form)
            .ok_or_else(|| FormEditorError::UnknownForm(self.open_form.clone()))?;
        let stem = canonical_gear_stem(kind_id.as_str())?;
        let mut suffix = 1_u32;
        let name = loop {
            let candidate = if suffix == 1 {
                stem.clone()
            } else {
                format!("{stem}-{suffix}")
            };
            let identity = format!("form/{}/gear/{candidate}", self.open_form);
            if !form.items.iter().any(|item| item.identity == identity) {
                break candidate;
            }
            suffix = suffix
                .checked_add(1)
                .ok_or(FormEditorError::GraphTooLarge)?;
        };
        let close = self.source[form.source_span.start..form.source_span.end]
            .rfind('}')
            .map(|offset| form.source_span.start + offset)
            .ok_or_else(|| FormEditorError::UnknownForm(self.open_form.clone()))?;
        let mut candidate = self.source.clone();
        candidate.insert_str(close, &format!("    {name}: {}\n", kind_id.as_str()));
        ensure_source_bound(&candidate)?;
        let next_revision = self.revision.saturating_add(1);
        let checked = check_revision(next_revision, &candidate)?;
        if let Some(diagnostic) = checked.diagnostics.first() {
            return Err(FormEditorError::Catalog(diagnostic.message.clone()));
        }
        self.source = candidate;
        self.revision = next_revision;
        self.checked = checked;
        self.selection = None;
        Ok(name)
    }

    /// Duplicates the exact authored Gear statement with a fresh local name.
    pub fn duplicate_gear(
        &mut self,
        offered_revision: u64,
        gear_name: &str,
    ) -> Result<String, FormEditorError> {
        self.require_revision(offered_revision)?;
        let form = self.open_graph_form()?;
        let prefix = format!("form/{}/gear/", self.open_form);
        let item = form
            .items
            .iter()
            .find(|item| {
                item.kind == GraphItemKind::Gear
                    && item.identity.strip_prefix(&prefix) == Some(gear_name)
            })
            .ok_or_else(|| FormEditorError::UnknownGear(gear_name.into()))?;
        let statement = self.source[item.source_span.start..item.source_span.end].to_owned();
        let colon = statement
            .find(':')
            .ok_or(FormEditorError::InvalidGearName)?;
        let name = unique_gear_name(form, gear_name)?;
        let replacement = format!("{name}{}", &statement[colon..]);
        let close = form_close(&self.source, form)?;
        let mut candidate = self.source.clone();
        candidate.insert_str(close, &format!("    {replacement}\n"));
        self.apply_candidate(candidate)?;
        Ok(name)
    }

    /// Removes one authored Gear and every authored Cord statement that names it.
    pub fn remove_gear(
        &mut self,
        offered_revision: u64,
        gear_name: &str,
    ) -> Result<(), FormEditorError> {
        self.require_revision(offered_revision)?;
        let form = self.open_graph_form()?;
        let prefix = format!("form/{}/gear/", self.open_form);
        let gear = form
            .items
            .iter()
            .find(|item| {
                item.kind == GraphItemKind::Gear
                    && item.identity.strip_prefix(&prefix) == Some(gear_name)
            })
            .ok_or_else(|| FormEditorError::UnknownGear(gear_name.into()))?;
        let mut ranges = vec![line_range(
            &self.source,
            gear.source_span.start,
            gear.source_span.end,
        )];
        for (cord, item) in form.cords.iter().zip(
            form.items
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
                crate::GraphCordStage::InlineGear { .. } | crate::GraphCordStage::Literal => false,
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
        offered_expanded_form_id: &conduit_core::ExpandedFormId,
        source_port_identity: &str,
        sink_port_identity: &str,
    ) -> Result<(), FormEditorError> {
        self.require_revision(offered_revision)?;
        let expanded = self.expand_form_for_authoring(&self.open_form)?;
        let graph = PatchbayGraph::from_authoring(&expanded)
            .map_err(|error| FormEditorError::Catalog(error.to_string()))?;
        if &graph.expanded_form_id != offered_expanded_form_id {
            return Err(FormEditorError::StaleGraphBasis);
        }
        let internal_source = graph
            .gears
            .iter()
            .flat_map(|gear| &gear.outputs)
            .find(|port| port.identity == source_port_identity);
        let face_source = graph
            .face_inputs
            .iter()
            .find(|port| port.identity == source_port_identity);
        let internal_sink = graph
            .gears
            .iter()
            .flat_map(|gear| &gear.inputs)
            .find(|port| port.identity == sink_port_identity);
        let face_sink = graph
            .face_outputs
            .iter()
            .find(|port| port.identity == sink_port_identity);
        let source_descriptor = internal_source
            .map(|port| &port.descriptor)
            .or_else(|| face_source.map(|port| &port.descriptor))
            .ok_or_else(|| FormEditorError::UnknownPort(source_port_identity.into()))?;
        let sink_descriptor = internal_sink
            .map(|port| &port.descriptor)
            .or_else(|| face_sink.map(|port| &port.descriptor))
            .ok_or_else(|| FormEditorError::UnknownPort(sink_port_identity.into()))?;
        if source_descriptor.value_kind != sink_descriptor.value_kind {
            return Err(FormEditorError::IncompatiblePorts(format!(
                "Info {} cannot feed {}",
                source_descriptor.value_kind.as_str(),
                sink_descriptor.value_kind.as_str()
            )));
        }
        if source_descriptor.temporal != sink_descriptor.temporal {
            return Err(FormEditorError::IncompatiblePorts(format!(
                "temporal contract {:?} cannot feed {:?}",
                source_descriptor.temporal, sink_descriptor.temporal
            )));
        }
        if graph.cords.iter().any(|cord| {
            cord.source_port == source_port_identity && cord.sink_port == sink_port_identity
        }) {
            return Err(FormEditorError::DuplicateCord);
        }
        let source_reference = if let Some(source) = internal_source {
            let source_name = direct_gear_name(&self.open_form, source.gear_id.as_str())?;
            format!("{source_name}.{}", source.descriptor.port_id.as_str())
        } else {
            face_source
                .expect("source descriptor was resolved")
                .descriptor
                .port_id
                .as_str()
                .to_owned()
        };
        let sink_reference = if let Some(sink) = internal_sink {
            let sink_name = direct_gear_name(&self.open_form, sink.gear_id.as_str())?;
            format!("{sink_name}.{}", sink.descriptor.port_id.as_str())
        } else {
            face_sink
                .expect("sink descriptor was resolved")
                .descriptor
                .port_id
                .as_str()
                .to_owned()
        };
        let form = self.open_graph_form()?;
        let close = form_close(&self.source, form)?;
        let statement = format!("    {source_reference} > {sink_reference}\n");
        let mut candidate = self.source.clone();
        candidate.insert_str(close, &statement);
        self.apply_candidate(candidate)
    }

    /// Removes one exact direct authored Cord from the current expanded graph.
    pub fn remove_cord(
        &mut self,
        offered_revision: u64,
        offered_expanded_form_id: &conduit_core::ExpandedFormId,
        cord_identity: &str,
    ) -> Result<(), FormEditorError> {
        self.require_revision(offered_revision)?;
        let expanded = self.expand_form(&self.open_form)?;
        let graph = PatchbayGraph::from_expanded(&expanded)
            .map_err(|error| FormEditorError::Catalog(error.to_string()))?;
        if &graph.expanded_form_id != offered_expanded_form_id {
            return Err(FormEditorError::StaleGraphBasis);
        }
        let cord = graph
            .cords
            .iter()
            .find(|cord| cord.identity == cord_identity)
            .ok_or_else(|| FormEditorError::UnknownCord(cord_identity.into()))?;
        let source = direct_port_reference(&self.open_form, &cord.source_port, "output")?;
        let sink = direct_port_reference(&self.open_form, &cord.sink_port, "input")?;
        let form = self.open_graph_form()?;
        let (cord, item) = form
            .cords
            .iter()
            .zip(
                form.items
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
            .ok_or_else(|| FormEditorError::UnknownCord(cord_identity.into()))?;
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
        offered_expanded_form_id: &conduit_core::ExpandedFormId,
        cord_identity: &str,
        endpoint_port_identity: &str,
    ) -> Result<(), FormEditorError> {
        self.require_revision(offered_revision)?;
        let expanded = self.expand_form(&self.open_form)?;
        let graph = PatchbayGraph::from_expanded(&expanded)
            .map_err(|error| FormEditorError::Catalog(error.to_string()))?;
        if &graph.expanded_form_id != offered_expanded_form_id {
            return Err(FormEditorError::StaleGraphBasis);
        }
        let cord = graph
            .cords
            .iter()
            .find(|cord| cord.identity == cord_identity)
            .ok_or_else(|| FormEditorError::UnknownCord(cord_identity.into()))?;
        let old_source_port = graph
            .gears
            .iter()
            .flat_map(|gear| &gear.outputs)
            .find(|port| port.identity == cord.source_port)
            .ok_or_else(|| FormEditorError::UnknownPort(cord.source_port.clone()))?;
        let old_sink_port = graph
            .gears
            .iter()
            .flat_map(|gear| &gear.inputs)
            .find(|port| port.identity == cord.sink_port)
            .ok_or_else(|| FormEditorError::UnknownPort(cord.sink_port.clone()))?;
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
            _ => return Err(FormEditorError::UnknownPort(endpoint_port_identity.into())),
        };
        if source_port.descriptor.value_kind != sink_port.descriptor.value_kind {
            return Err(FormEditorError::IncompatiblePorts(format!(
                "Info {} cannot feed {}",
                source_port.descriptor.value_kind.as_str(),
                sink_port.descriptor.value_kind.as_str()
            )));
        }
        if source_port.descriptor.temporal != sink_port.descriptor.temporal {
            return Err(FormEditorError::IncompatiblePorts(format!(
                "temporal contract {:?} cannot feed {:?}",
                source_port.descriptor.temporal, sink_port.descriptor.temporal
            )));
        }
        if graph.cords.iter().any(|candidate| {
            candidate.identity != cord_identity
                && candidate.source_port == source_port.identity
                && candidate.sink_port == sink_port.identity
        }) {
            return Err(FormEditorError::DuplicateCord);
        }
        let old_source = direct_port_reference(&self.open_form, &cord.source_port, "output")?;
        let old_sink = direct_port_reference(&self.open_form, &cord.sink_port, "input")?;
        let form = self.open_graph_form()?;
        let item = form
            .cords
            .iter()
            .zip(
                form.items
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
            .ok_or_else(|| FormEditorError::UnknownCord(cord_identity.into()))?;
        let source_name = direct_gear_name(&self.open_form, source_port.gear_id.as_str())?;
        let sink_name = direct_gear_name(&self.open_form, sink_port.gear_id.as_str())?;
        let statement = format!(
            "{source_name}.{} > {sink_name}.{}",
            source_port.descriptor.port_id.as_str(),
            sink_port.descriptor.port_id.as_str()
        );
        let mut candidate = self.source.clone();
        candidate.replace_range(item.source_span.start..item.source_span.end, &statement);
        self.apply_candidate(candidate)
    }

    pub(crate) fn require_revision(&self, offered: u64) -> Result<(), FormEditorError> {
        if offered != self.revision {
            return Err(FormEditorError::StaleRevision {
                current: self.revision,
                offered,
            });
        }
        Ok(())
    }

    fn open_graph_form(&self) -> Result<&crate::GraphForm, FormEditorError> {
        self.checked
            .forms
            .iter()
            .find(|form| form.name == self.open_form)
            .ok_or_else(|| FormEditorError::UnknownForm(self.open_form.clone()))
    }

    pub(crate) fn apply_candidate(&mut self, candidate: String) -> Result<(), FormEditorError> {
        ensure_source_bound(&candidate)?;
        let next_revision = self.revision.saturating_add(1);
        let checked = check_revision(next_revision, &candidate)?;
        if let Some(diagnostic) = checked.diagnostics.first() {
            return Err(FormEditorError::Catalog(diagnostic.message.clone()));
        }
        self.source = candidate;
        self.revision = next_revision;
        self.checked = checked;
        self.selection = None;
        Ok(())
    }
}

fn unique_gear_name(form: &crate::GraphForm, stem: &str) -> Result<String, FormEditorError> {
    let prefix = format!("form/{}/gear/", form.name);
    for suffix in 2_u32..=u32::MAX {
        let candidate = format!("{stem}-{suffix}");
        if !form
            .items
            .iter()
            .any(|item| item.identity.strip_prefix(&prefix) == Some(&candidate))
        {
            return Ok(candidate);
        }
    }
    Err(FormEditorError::GraphTooLarge)
}

fn form_close(source: &str, form: &crate::GraphForm) -> Result<usize, FormEditorError> {
    source[form.source_span.start..form.source_span.end]
        .rfind('}')
        .map(|offset| form.source_span.start + offset)
        .ok_or_else(|| FormEditorError::UnknownForm(form.name.clone()))
}

fn direct_gear_name(form: &str, gear_id: &str) -> Result<String, FormEditorError> {
    let prefix = format!("{form}/");
    let name = gear_id
        .strip_prefix(&prefix)
        .ok_or_else(|| FormEditorError::UnknownGear(gear_id.into()))?;
    if name.contains('/') {
        return Err(FormEditorError::NestedGearEditUnsupported(gear_id.into()));
    }
    Ok(name.into())
}

fn direct_port_reference(
    form: &str,
    identity: &str,
    direction: &str,
) -> Result<String, FormEditorError> {
    let prefix = format!("port/{form}/");
    let suffix = identity
        .strip_prefix(&prefix)
        .ok_or_else(|| FormEditorError::UnknownPort(identity.into()))?;
    let marker = format!("/{direction}/");
    let (gear, port) = suffix
        .split_once(&marker)
        .ok_or_else(|| FormEditorError::UnknownPort(identity.into()))?;
    if gear.contains('/') || port.contains('/') || gear.is_empty() || port.is_empty() {
        return Err(FormEditorError::NestedGearEditUnsupported(identity.into()));
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

fn canonical_gear_stem(kind: &str) -> Result<String, FormEditorError> {
    let stem = kind.rsplit('/').next().unwrap_or(kind);
    if stem.is_empty()
        || !stem
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(FormEditorError::InvalidGearName);
    }
    Ok(stem.into())
}
