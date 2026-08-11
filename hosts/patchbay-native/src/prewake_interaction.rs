//! Native PREWAKE controls over the single model controller.

use crate::{gui::GuiAction, PatchbayApplication};
use patchbay_model::PrewakeState;
use winit::keyboard::{Key, NamedKey};

impl PatchbayApplication {
    pub(super) fn handle_prewake_key(&mut self, key: &Key) -> Result<bool, String> {
        if self.prewake.is_none() {
            return Ok(false);
        }
        let action = match key {
            Key::Named(NamedKey::F3) => GuiAction::PrewakeToggleWorkspace,
            Key::Named(NamedKey::F6) => GuiAction::PrewakeToggleHold,
            Key::Named(NamedKey::F7) => GuiAction::PrewakeRelease,
            Key::Named(NamedKey::Escape) => GuiAction::PrewakeExit,
            _ => return Ok(false),
        };
        self.handle_prewake_action(action)?;
        Ok(true)
    }

    pub(super) fn handle_prewake_action(&mut self, action: GuiAction) -> Result<(), String> {
        match action {
            GuiAction::PrewakeToggleWorkspace => {
                self.prewake_environment_view = !self.prewake_environment_view
            }
            GuiAction::PrewakeToggleHold => {
                let enabled = !self
                    .prewake
                    .as_ref()
                    .expect("PREWAKE present")
                    .hold_enabled();
                self.prewake
                    .as_mut()
                    .expect("PREWAKE present")
                    .set_hold(enabled);
                self.refresh_prewake()?;
            }
            GuiAction::PrewakeRelease => {
                let editor = self.form_editor.as_ref().ok_or("PREWAKE Form is absent")?;
                let environment = self
                    .environment
                    .as_ref()
                    .ok_or("PREWAKE environment is absent")?;
                let _ = self
                    .prewake
                    .as_mut()
                    .expect("PREWAKE present")
                    .release(editor, environment);
            }
            GuiAction::PrewakeExit => self.prewake.as_mut().expect("PREWAKE present").exit(),
            GuiAction::PrewakeNextImplementation(subject) => {
                let editor = self.form_editor.as_ref().ok_or("PREWAKE Form is absent")?;
                let environment = self
                    .environment
                    .as_ref()
                    .ok_or("PREWAKE environment is absent")?;
                let _ = self
                    .prewake
                    .as_mut()
                    .expect("PREWAKE present")
                    .request_next_implementation(editor, environment, &subject);
            }
            _ => return Err("non-PREWAKE action reached PREWAKE controls".into()),
        }
        if let Some(window) = &self.window {
            window.request_redraw();
        }
        Ok(())
    }

    pub(super) fn refresh_prewake(&mut self) -> Result<(), String> {
        let Some(controller) = &mut self.prewake else {
            return Ok(());
        };
        if matches!(controller.state(), PrewakeState::Off) {
            return Ok(());
        }
        let _ = controller.rehearse(
            self.form_editor.as_ref().ok_or("PREWAKE Form is absent")?,
            self.environment
                .as_ref()
                .ok_or("PREWAKE environment is absent")?,
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arguments::Arguments;
    use patchbay_model::{PrewakeError, PrewakeState};

    #[test]
    fn native_prewake_toggles_hold_rehearses_edits_and_never_gains_authority() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let mut application = PatchbayApplication::new(Arguments {
            prewake: true,
            form_path: Some(root.join("examples/hello.conduit")),
            environment_path: Some(root.join("examples/maker-workbench.json")),
            ..Default::default()
        })
        .unwrap();
        let first_plan = match application.prewake.as_ref().unwrap().state() {
            PrewakeState::Auto { plan, .. } => plan.plan_id.clone(),
            state => panic!("unexpected state {state:?}"),
        };
        application
            .handle_prewake_action(GuiAction::PrewakeToggleHold)
            .unwrap();
        assert!(matches!(
            application.prewake.as_ref().unwrap().state(),
            PrewakeState::Held { .. }
        ));
        let editor = application.form_editor.as_mut().unwrap();
        let changed = editor
            .view()
            .source
            .replace("Hello, world.", "Hello, PREWAKE.");
        editor.replace_source(changed).unwrap();
        editor.recheck().unwrap();
        application.refresh_prewake().unwrap();
        let held_plan = match application.prewake.as_ref().unwrap().state() {
            PrewakeState::Held { plan, .. } => plan.plan_id.clone(),
            _ => panic!(),
        };
        assert_ne!(held_plan, first_plan);
        let editor = application.form_editor.as_mut().unwrap();
        let stale = editor.view().source.replace("PREWAKE", "STALE");
        editor.replace_source(stale).unwrap();
        editor.recheck().unwrap();
        application
            .handle_prewake_action(GuiAction::PrewakeRelease)
            .unwrap();
        assert_eq!(
            application.prewake.as_ref().unwrap().last_refusal(),
            Some(&PrewakeError::StaleHeldPlan {
                plan_id: held_plan.clone()
            })
        );
        assert!(
            matches!(application.prewake.as_ref().unwrap().state(), PrewakeState::Held { plan, .. } if plan.plan_id == held_plan)
        );
        let provenance = application.prewake.as_ref().unwrap().provenance();
        assert!(!provenance.observed_live_truth);
        assert!(!provenance.physical_effect_authority);
        assert!(!provenance.promotable_to_physical_plan);
        application
            .handle_prewake_action(GuiAction::PrewakeExit)
            .unwrap();
        assert!(matches!(
            application.prewake.as_ref().unwrap().state(),
            PrewakeState::Off
        ));
    }
}
