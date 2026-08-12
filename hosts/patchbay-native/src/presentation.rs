//! Native Patchbay document composition, kept separate from event-loop policy.

use super::PatchbayApplication;
use conduit_presentation::{render_linear_presentation, Presentation};
use patchbay_model::{GraphItemKind, RendererSelfInspection};

const MAX_FORM_PRESENTATION_LINES: usize = 256;

/// Native text is a renderer-local realization of the shared portable value.
pub(super) fn portable_presentation_lines(
    presentation: &Presentation,
) -> Result<Vec<String>, String> {
    render_linear_presentation(presentation)
        .map(|projection| projection.lines)
        .map_err(|error| error.to_string())
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
    lines.extend(manifestation.signs.iter().map(|sign| {
        format!(
            "RENDERER SIGN {} lifecycle={:?}",
            sign.sign_id.as_str(),
            sign.lifecycle
        )
    }));
    Ok(lines)
}

impl PatchbayApplication {
    pub(super) fn details_content_lines(&self) -> Vec<String> {
        if self.details_lens == crate::details::DetailsLens::Source {
            let mut lines = vec![
                "DETAILS / SOURCE / READ ONLY".into(),
                "LEFT/RIGHT LENS  UP/DOWN TRAVERSE  F2 CLOSE  EXACT TEXT".into(),
            ];
            if let Some(editor) = &self.form_editor {
                let view = editor.view();
                lines.push(format!(
                    "SOURCE {} revision={} exact UTF-8",
                    view.path.display(),
                    view.revision
                ));
                lines.extend(view.source.lines().map(str::to_owned));
            } else {
                lines.push("  no Source document is open".into());
            }
            return lines;
        }
        let mut complete =
            crate::details::lens_lines(self.details_lens, &self.presentation_lines());
        if self.details_lens == crate::details::DetailsLens::Checked {
            if let Some(graph) = &self.graphical_form {
                complete.splice(
                    2..2,
                    [
                        format!("SOURCE DOCUMENT {}", graph.source_document_id.as_str()),
                        format!("CHECKED FORM {}", graph.checked_form_id.as_str()),
                        format!("EXPANDED FORM {}", graph.expanded_form_id.as_str()),
                    ],
                );
            }
        }
        complete
    }

    pub(super) fn details_lines(&self) -> Vec<String> {
        let complete = self.details_content_lines();
        let mut visible = complete.iter().take(2).cloned().collect::<Vec<_>>();
        visible.extend(
            complete
                .iter()
                .skip(2usize.saturating_add(self.details_scroll))
                .cloned(),
        );
        visible
    }

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
            if let Some(interaction) = &self.interaction {
                lines.extend(interaction.lines());
            }
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
            if let Some(interaction) = &self.interaction {
                lines.extend(interaction.lines());
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
            "  BUILD: edit/end Backspace/delete Ctrl-S/save Tab/open-back Up/Down/select | BODY: F4/Birth F5/Wake F6/Plan-a-Play F7/Play-the-Plan F8/Unsatisfied F9/Lull F12/Parts Esc/Stop-Play | File (Alt): F7/source F8/create Shift-F8/replace F9/Plan F10/Run F11/Stop".into(),
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
        lines.extend(
            self.form_navigator_entries()
                .iter()
                .enumerate()
                .map(|(index, entry)| {
                    let state = if index == self.navigator_selection {
                        "selected"
                    } else {
                        "not-selected"
                    };
                    let action = if entry.action.is_some() {
                        "actionable"
                    } else {
                        "unavailable"
                    };
                    format!("FORM NAVIGATOR {state} {action} {}", entry.label)
                }),
        );
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
        if let Some(interaction) = &self.interaction {
            lines.extend(interaction.lines());
        }
        lines.truncate(MAX_FORM_PRESENTATION_LINES);
        lines
    }
}

fn append_route_demo(lines: &mut Vec<String>, demo: &patchbay_model::DistributedRouteDemo) {
    lines.extend(demo.visual_lines());
    lines.push("LINEAR NARRATION".into());
    lines.extend(demo.linear_lines());
    lines.push("ROUTE DETAIL — exact identities and Signs".into());
    lines.extend_from_slice(demo.lines());
}
