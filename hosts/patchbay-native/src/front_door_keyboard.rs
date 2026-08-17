//! Native bindings for actions advertised by the zero-Body Presentation.

use super::PatchbayApplication;
use patchbay_model::PatchbayAction;
use winit::keyboard::{Key, NamedKey};

impl PatchbayApplication {
    pub(super) fn handle_front_door_key(&mut self, key: &Key) -> Result<bool, String> {
        if self.zero_body_front_door.is_none() || self.entrance_presentation.is_none() {
            return Ok(false);
        }
        match key {
            Key::Named(NamedKey::Enter) => {
                self.dispatch_invocation(PatchbayAction::OpenBack)?;
            }
            Key::Named(NamedKey::F2) => {
                // Exact disclosure is renderer-local Presentation state. It
                // never changes the underlying Presentation or event identity.
                self.linear_view = !self.linear_view;
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
