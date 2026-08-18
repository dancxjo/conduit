//! Native bindings for actions advertised by the zero-Body Presentation.

use super::PatchbayApplication;
use conduit_presentation::{NavigationOperation, PresentationDepth};
use patchbay_model::PatchbayAction;
use winit::keyboard::{Key, NamedKey};

use crate::presentation::focused_action_for_binding;

impl PatchbayApplication {
    pub(super) fn handle_front_door_key(&mut self, key: &Key) -> Result<bool, String> {
        if self.zero_body_front_door.is_none() || self.entrance.is_none() {
            return Ok(false);
        }
        match key {
            Key::Named(NamedKey::Tab) if self.modifiers.control_key() => {
                self.cycle_front_door_place(1)?
            }
            Key::Named(NamedKey::PageUp) if self.modifiers.control_key() => {
                self.cycle_front_door_aspect(-1)?
            }
            Key::Named(NamedKey::PageDown) if self.modifiers.control_key() => {
                self.cycle_front_door_aspect(1)?
            }
            Key::Named(NamedKey::Enter) => {
                self.invoke_focused_action(PatchbayAction::OpenBack)?;
            }
            Key::Named(NamedKey::F2) => {
                if self.linear_view {
                    self.navigate_front_door(NavigationOperation::Back)?;
                    self.linear_view = false;
                } else {
                    self.navigate_front_door(NavigationOperation::Disclose(
                        PresentationDepth::Exact,
                    ))?;
                    self.linear_view = true;
                }
            }
            Key::Named(NamedKey::F3) if self.modifiers.shift_key() => {
                self.cycle_front_door_follow()?
            }
            Key::Named(NamedKey::F3) => self.follow_front_door()?,
            Key::Named(NamedKey::F4) => {
                self.invoke_focused_action(PatchbayAction::Birth)?;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn invoke_focused_action(&mut self, binding: PatchbayAction) -> Result<(), String> {
        let Some(entrance) = self.entrance.as_ref() else {
            return Err("native front-door presentation is absent".into());
        };
        let action =
            focused_action_for_binding(&entrance.presentation, &entrance.navigation, binding)?;
        let Some(action) = action else {
            let label = match binding {
                PatchbayAction::OpenBack => "ENTER",
                PatchbayAction::Birth => "F4",
                _ => "ACTION",
            };
            self.publish_refusal(format!(
                "{label} is unavailable: no current semantic action"
            ));
            return Ok(());
        };
        self.dispatch_invocation_with_action_id(&action.identity)
            .map_err(|error| format!("front-door action failed: {error}"))
    }
}
