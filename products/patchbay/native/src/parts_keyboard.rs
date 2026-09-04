//! Keyboard parity for the native Parts destination.

use crate::{gui::GuiAction, PatchbayApplication};
use winit::keyboard::{Key, NamedKey};

impl PatchbayApplication {
    pub(super) fn handle_parts_key(&mut self, key: &Key) -> Result<bool, String> {
        if !matches!(key, Key::Named(NamedKey::F12)) {
            return Ok(false);
        }
        self.handle_parts_action(GuiAction::TogglePartsView)?;
        Ok(true)
    }
}
