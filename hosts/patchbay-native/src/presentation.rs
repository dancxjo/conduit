//! Native Patchbay document composition, kept separate from event-loop policy.

use super::PatchbayApplication;
use conduit_core::ConnectionBase;
use conduit_presentation::{Presentation, PresentationPropertyValue};
use patchbay_model::{GraphItemKind, RendererSelfInspection};

const MAX_FORM_PRESENTATION_LINES: usize = 256;

/// Native text is a renderer-local realization of the shared portable value.
pub(super) fn portable_presentation_lines(
    presentation: &Presentation,
) -> Result<Vec<String>, String> {
    presentation.validate().map_err(|error| error.to_string())?;
    let basis = &presentation.basis;
    let mut lines = vec![
        format!(
            "PRESENTATION {} revision={}",
            presentation.identity.as_str(),
            presentation.revision
        ),
        format!(
            "SEED {} body={} wake={}",
            basis.seed_id.as_str(),
            basis.body_id.as_str(),
            basis.wake_id.as_str()
        ),
        format!(
            "FORM source={} checked={}",
            basis.source_document_id.as_str(),
            basis.checked_form_id.as_str()
        ),
        format!(
            "PLAN {} PLAY {}",
            basis
                .plan_id
                .as_ref()
                .map_or("not present", |id| id.as_str()),
            basis
                .active_play_id
                .as_ref()
                .map_or("not present", |id| id.as_str())
        ),
    ];
    for subject in &presentation.subjects {
        lines.push(format!(
            "{:?} {} — {}",
            subject.role, subject.identity, subject.label
        ));
        for property in presentation
            .properties
            .iter()
            .filter(|property| property.subject == subject.identity)
        {
            lines.push(format!(
                "  {}={}",
                property.name,
                display_property(&property.value)
            ));
        }
        lines.extend(
            presentation
                .text
                .iter()
                .filter(|text| text.subject == subject.identity)
                .map(|text| format!("  {}", text.text)),
        );
    }
    lines.truncate(MAX_FORM_PRESENTATION_LINES);
    Ok(lines)
}

fn display_property(value: &PresentationPropertyValue) -> String {
    match value {
        PresentationPropertyValue::Identity(value) | PresentationPropertyValue::Text(value) => {
            value.clone()
        }
        PresentationPropertyValue::ConnectionBase(base) => display_base(*base).into(),
        PresentationPropertyValue::Count(value) => value.to_string(),
        PresentationPropertyValue::Flag(value) => value.to_string(),
    }
}

fn display_base(base: ConnectionBase) -> &'static str {
    match base {
        ConnectionBase::Local => "local",
        ConnectionBase::InMemory => "in-memory",
        ConnectionBase::FixtureFrame => "fixture frame",
        ConnectionBase::FixtureDatagram => "fixture datagram",
        ConnectionBase::WebSocket => "WebSocket",
        ConnectionBase::UsbCdc => "USB CDC",
    }
}

fn renderer_self_inspection_lines(
    inspection: &RendererSelfInspection,
) -> Result<Vec<String>, String> {
    let placement = inspection
        .renderer_placement()
        .map_err(|error| error.to_string())?;
    let manifestation = &inspection.manifestation;
    let mut lines = vec![
        format!(
            "RENDERER FACE {} inputs={} outputs={}",
            placement.kind_id.as_str(),
            placement.inputs.len(),
            placement.outputs.len()
        ),
        format!(
            "RENDERER PLACEMENT {} host={} boot={}",
            placement.placement_id.as_str(),
            placement.host_id.as_str(),
            placement.boot_id.as_str()
        ),
        format!(
            "RENDERER REALIZATION implementation={} artifact={} capability={} profile={} offer-generation={}",
            placement.implementation_id.as_str(),
            placement.artifact_id.as_str(),
            placement.capability_id.as_str(),
            placement.execution_profile_id.as_str(),
            placement.offer_generation.0
        ),
        format!(
            "RENDERER LIMITS active={} queue-items={} queue-bytes={}",
            placement.limits.max_active_instances,
            placement.limits.max_queue_items,
            placement.limits.max_queue_bytes
        ),
        format!(
            "MANIFESTATION {} renderer-plan={} renderer-play={} lifecycle={:?}",
            manifestation.manifestation_id.as_str(),
            manifestation.plan_id.as_str(),
            manifestation.active_play_id.as_str(),
            manifestation.lifecycle
        ),
    ];
    lines.extend(
        placement
            .inputs
            .iter()
            .chain(&placement.outputs)
            .map(|port| {
                format!(
                    "RENDERER PORT {} {:?} info={} temporal={:?}",
                    port.port_id.as_str(),
                    port.direction,
                    port.value_kind.as_str(),
                    port.temporal
                )
            }),
    );
    lines.extend(placement.resources.iter().map(|resource| {
        format!(
            "RENDERER RESOURCE pool={} class={} units={}",
            resource.pool_id.as_str(),
            resource.class_id.as_str(),
            resource.units
        )
    }));
    lines.extend(placement.host_operations.iter().map(|operation| {
        format!(
            "RENDERER BASE contract={} target={} in-flight={} input-bytes={} output-bytes={}",
            operation.contract_id.as_str(),
            operation
                .target_kind
                .as_ref()
                .map_or("not present", |target| target.as_str()),
            operation.maximum_in_flight,
            operation.maximum_input_bytes,
            operation.maximum_output_bytes
        )
    }));
    lines.extend(manifestation.clues.iter().map(|clue| {
        format!(
            "RENDERER CLUE {} lifecycle={:?}",
            clue.clue_id.as_str(),
            clue.lifecycle
        )
    }));
    Ok(lines)
}

impl PatchbayApplication {
    pub(super) fn presentation_lines(&self) -> Vec<String> {
        if let Some(execution) = &self.renderer_execution {
            let mut lines = portable_presentation_lines(&execution.presentation)
                .unwrap_or_else(|error| vec![format!("PORTABLE PRESENTATION INVALID: {error}")]);
            let inspection = match execution.self_inspection() {
                Ok(inspection) => renderer_self_inspection_lines(&inspection)
                    .unwrap_or_else(|error| vec![format!("RENDERER INSPECTION INVALID: {error}")]),
                Err(error) => vec![format!("RENDERER INSPECTION INVALID: {error}")],
            };
            lines.splice(1..1, inspection);
            return lines;
        }
        let Some(editor) = &self.form_editor else {
            let mut lines = self.topology_lines.clone();
            if let Some(demo) = &self.route_demo {
                append_route_demo(&mut lines, demo);
            }
            if let Some(distributed) = &self.distributed_play {
                lines.extend_from_slice(distributed.lines());
            }
            lines.truncate(MAX_FORM_PRESENTATION_LINES);
            return lines;
        };
        let view = editor.view();
        let mut lines = self
            .build_birth
            .document(editor)
            .map(|document| document.lines)
            .unwrap_or_else(|error| vec![format!("BUILD/BIRTH DOCUMENT INVALID: {error}")]);
        lines.extend([
            format!("SOURCE {} revision={}", view.path.display(), view.revision),
            "  BUILD: edit/end Backspace/delete Ctrl-S/save Tab/open-back Up/Down/select | BODY: F4/Birth F5/Wake F6/Plan-a-Play F7/Play-the-Plan F8/Unsatisfied F9/Lull Esc/Stop-Play | File (Alt): F7/source F8/create Shift-F8/replace F9/Plan F10/Run F11/Stop".into(),
        ]);
        lines.extend(
            view.source
                .lines()
                .take(MAX_FORM_PRESENTATION_LINES.saturating_sub(4))
                .map(|line| format!("  {line}")),
        );
        if let Some(diagnostic) = view.checked.diagnostics.first() {
            lines.push(format!(
                "DIAGNOSTIC {} {}:{}-{}:{} bytes={}..{} {}",
                diagnostic.code,
                diagnostic.span.line,
                diagnostic.span.column,
                diagnostic.span.end_line,
                diagnostic.span.end_column,
                diagnostic.span.start,
                diagnostic.span.end,
                diagnostic.message
            ));
            lines.truncate(MAX_FORM_PRESENTATION_LINES);
            return lines;
        }
        lines.push(format!(
            "CHECKED source={} forms={} OPEN BACK {}",
            view.checked
                .source_document_id
                .as_ref()
                .map(|id| id.as_str())
                .unwrap_or("none"),
            view.checked.forms.len(),
            view.open_form
        ));
        if let Some(form) = view
            .checked
            .forms
            .iter()
            .find(|form| form.name == view.open_form)
        {
            for (index, item) in form.items.iter().enumerate() {
                let marker = if index == self.form_selection {
                    ">"
                } else {
                    " "
                };
                let kind = match item.kind {
                    GraphItemKind::FaceInput => "face-in",
                    GraphItemKind::FaceOutput => "face-out",
                    GraphItemKind::StartupValue => "startup",
                    GraphItemKind::Gear => "gear",
                    GraphItemKind::Cord => "cord",
                };
                lines.push(format!(
                    "{marker} {kind} {} [{}..{}] {}",
                    item.identity, item.source_span.start, item.source_span.end, item.label
                ));
            }
        }
        lines.extend(self.control.lines());
        lines.extend(self.file_task.lines());
        if let Some(demo) = &self.route_demo {
            append_route_demo(&mut lines, demo);
        }
        if let Some(distributed) = &self.distributed_play {
            lines.extend_from_slice(distributed.lines());
        }
        lines.truncate(MAX_FORM_PRESENTATION_LINES);
        lines
    }
}

fn append_route_demo(lines: &mut Vec<String>, demo: &patchbay_model::DistributedRouteDemo) {
    lines.extend(demo.visual_lines());
    lines.push("LINEAR NARRATION".into());
    lines.extend(demo.linear_lines());
    lines.push("ROUTE DETAIL — exact identities and Clues".into());
    lines.extend_from_slice(demo.lines());
}
