//! Native keyboard and pointer routing for Program, Body, and Body/Signs.

use conduit_presentation::{PresentationAspect, PresentationPlace};
use patchbay_model::PatchbayAction;
use winit::keyboard::{Key, NamedKey};

use crate::{gui::GuiAction, PatchbayApplication};

impl PatchbayApplication {
    pub(super) fn handle_workbench_key(&mut self, key: &Key) -> Result<bool, String> {
        if self.workbench.current().is_none() {
            return Ok(false);
        }
        match key {
            Key::Named(NamedKey::Tab) if self.modifiers.control_key() => {
                self.workbench
                    .current_mut()
                    .expect("checked workbench")
                    .cycle_destination();
            }
            Key::Named(NamedKey::F2) => {
                self.linear_view = !self.linear_view;
                self.workbench
                    .current_mut()
                    .expect("checked workbench")
                    .toggle_exact();
            }
            Key::Named(NamedKey::F12) => {
                self.workbench
                    .current_mut()
                    .expect("checked workbench")
                    .show(PresentationPlace::Body, PresentationAspect::Structure)
                    .map_err(|error| format!("native workbench: {error:?}"))?;
            }
            Key::Named(NamedKey::Enter)
                if self
                    .workbench
                    .current()
                    .is_some_and(crate::native_workbench::NativeBodyWorkbench::is_history) =>
            {
                self.workbench
                    .current_mut()
                    .expect("checked workbench")
                    .inspect_focused_history();
            }
            Key::Named(NamedKey::ArrowDown)
                if self
                    .workbench
                    .current()
                    .is_some_and(crate::native_workbench::NativeBodyWorkbench::is_history) =>
            {
                self.workbench
                    .current_mut()
                    .expect("checked workbench")
                    .move_history_focus(true);
            }
            Key::Named(NamedKey::ArrowUp)
                if self
                    .workbench
                    .current()
                    .is_some_and(crate::native_workbench::NativeBodyWorkbench::is_history) =>
            {
                self.workbench
                    .current_mut()
                    .expect("checked workbench")
                    .move_history_focus(false);
            }
            Key::Named(
                NamedKey::F5 | NamedKey::F6 | NamedKey::F7 | NamedKey::F8 | NamedKey::F9,
            )
            | Key::Named(NamedKey::Escape) => {
                let unchanged = self
                    .workbench
                    .current()
                    .expect("checked workbench")
                    .evidence()
                    .clone();
                let refusal = self
                    .workbench
                    .current()
                    .expect("checked workbench")
                    .request_lifecycle_action();
                debug_assert_eq!(
                    self.workbench
                        .current()
                        .expect("checked workbench")
                        .evidence(),
                    &unchanged
                );
                self.publish_refusal(format!(
                    "Attached Body lifecycle action is unavailable: {refusal:?}"
                ));
            }
            _ if !self
                .workbench
                .current()
                .expect("checked workbench")
                .is_program() =>
            {
                return Ok(true)
            }
            _ => return Ok(false),
        }
        self.request_workbench_redraw();
        Ok(true)
    }

    pub(super) fn handle_native_workbench_action(
        &mut self,
        action: &GuiAction,
    ) -> Result<bool, String> {
        if self.workbench.current().is_none() {
            return Ok(false);
        }
        match action {
            GuiAction::ShowWorkbench { place, aspect } => self
                .workbench
                .current_mut()
                .expect("checked workbench")
                .show(*place, *aspect)
                .map_err(|error| format!("native workbench: {error:?}"))?,
            GuiAction::InspectHistoryEntry(index) => {
                let workbench = self.workbench.current_mut().expect("checked workbench");
                while workbench.history_focus() != *index {
                    workbench.move_history_focus(true);
                }
                workbench.inspect_focused_history();
            }
            GuiAction::Lifecycle(
                PatchbayAction::Birth
                | PatchbayAction::Wake
                | PatchbayAction::Lull
                | PatchbayAction::Plan
                | PatchbayAction::Play
                | PatchbayAction::Stop
                | PatchbayAction::Hold,
            ) => {
                self.publish_refusal(
                    "Attached Body lifecycle requires an authoritative command boundary",
                );
            }
            _ => return Ok(false),
        }
        self.request_workbench_redraw();
        Ok(true)
    }

    fn request_workbench_redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}
