//! Native bindings for actions advertised by the zero-Body Presentation.

use super::PatchbayApplication;
use conduit_presentation::{NavigationOperation, PresentationDepth};
use patchbay_model::PatchbayAction;
use winit::keyboard::{Key, NamedKey};

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
                self.dispatch_invocation(PatchbayAction::OpenBack)?;
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
            Key::Named(NamedKey::F4) => {
                // Resolve the exact current advertised action and let the
                // ordinary invocation boundary enforce its availability.
                self.dispatch_invocation(PatchbayAction::Birth)?;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}
