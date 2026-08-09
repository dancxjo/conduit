//! Native Patchbay document composition, kept separate from event-loop policy.

use super::PatchbayApplication;
use conduit_core::ConnectionProvider;
use conduit_presentation::{Presentation, PresentationPropertyValue};
use patchbay_model::GraphItemKind;

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
            "REALM {} deployment={} activation={}",
            basis.realm_id.as_str(),
            basis.deployment_id.as_str(),
            basis.activation_id.as_str()
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
        PresentationPropertyValue::ConnectionProvider(provider) => {
            display_provider(*provider).into()
        }
        PresentationPropertyValue::Count(value) => value.to_string(),
        PresentationPropertyValue::Flag(value) => value.to_string(),
    }
}

fn display_provider(provider: ConnectionProvider) -> &'static str {
    match provider {
        ConnectionProvider::Local => "local",
        ConnectionProvider::InMemory => "in-memory",
        ConnectionProvider::FixtureFrame => "fixture frame",
        ConnectionProvider::FixtureDatagram => "fixture datagram",
        ConnectionProvider::WebSocket => "WebSocket",
        ConnectionProvider::UsbCdc => "USB CDC",
    }
}

impl PatchbayApplication {
    pub(super) fn presentation_lines(&self) -> Vec<String> {
        if let Some(presentation) = &self.portable_presentation {
            return portable_presentation_lines(presentation)
                .unwrap_or_else(|error| vec![format!("PORTABLE PRESENTATION INVALID: {error}")]);
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
        let mut lines = vec![
            format!("SOURCE {} revision={}", view.path.display(), view.revision),
            "  Form: edit/end Backspace/delete Ctrl-S/save Tab/open-back Up/Down/select | Play: F5/Plan F6/Run Esc/Stop | File: F7/source F8/create Shift-F8/replace F9/Plan F10/Run F11/Stop".into(),
        ];
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
                    GraphItemKind::Cell => "cell",
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
    lines.push("ROUTE DETAIL — exact identities and evidence".into());
    lines.extend_from_slice(demo.lines());
}
