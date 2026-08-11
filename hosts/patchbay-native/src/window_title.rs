//! Truthful native window titles for each exclusive Patchbay workspace.

use crate::PatchbayApplication;

impl PatchbayApplication {
    pub(super) fn title(&self) -> String {
        if let Some(environment) = &self.environment {
            return format!(
                "Conduit Patchbay — AUTHORED SIMULATION — {} revision {} — NO PHYSICAL AUTHORITY",
                environment.environment_id, environment.revision
            );
        }
        if let Some(editor) = &self.form_editor {
            let view = editor.view();
            let mode = self
                .build_birth
                .document(editor)
                .map(|document| format!("{:?}", document.mode))
                .unwrap_or_else(|_| "BuildInvalid".into());
            return format!(
                "Conduit Patchbay — {mode} — {} — canonical Form revision {}",
                view.path.display(),
                view.revision
            );
        }
        format!(
            "Conduit Patchbay — host {} — boot {} — topology lines {}",
            self.model.projection().host_id().as_str(),
            self.model.projection().boot_id().as_str(),
            self.topology_lines.len(),
        )
    }
}
