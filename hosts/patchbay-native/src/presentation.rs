//! Native Patchbay document composition, kept separate from event-loop policy.

use super::PatchbayApplication;
use patchbay_model::GraphItemKind;

const MAX_FORM_PRESENTATION_LINES: usize = 256;

impl PatchbayApplication {
    pub(super) fn presentation_lines(&self) -> Vec<String> {
        let Some(editor) = &self.form_editor else {
            let mut lines = self.topology_lines.clone();
            if let Some(demo) = &self.route_demo {
                lines.extend_from_slice(demo.lines());
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
            lines.extend_from_slice(demo.lines());
        }
        lines.truncate(MAX_FORM_PRESENTATION_LINES);
        lines
    }
}
