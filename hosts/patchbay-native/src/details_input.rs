//! Keyboard traversal and read-only enforcement for the bounded Details surface.

use super::PatchbayApplication;
use winit::keyboard::{Key, NamedKey};

impl PatchbayApplication {
    pub(super) fn handle_details_key(&mut self, key: &Key) -> bool {
        if !self.linear_view {
            return false;
        }
        match key {
            Key::Named(NamedKey::ArrowLeft) => {
                self.details_lens.move_by(-1);
                self.details_scroll = 0;
            }
            Key::Named(NamedKey::ArrowRight) => {
                self.details_lens.move_by(1);
                self.details_scroll = 0;
            }
            Key::Named(NamedKey::ArrowUp) => {
                self.details_scroll = self.details_scroll.saturating_sub(1);
            }
            Key::Named(NamedKey::ArrowDown) => {
                let count = self.details_content_lines().len().saturating_sub(2);
                self.details_scroll = self
                    .details_scroll
                    .saturating_add(1)
                    .min(count.saturating_sub(1));
            }
            Key::Character(_)
            | Key::Named(NamedKey::Backspace | NamedKey::Delete | NamedKey::Enter) => {
                self.publish_refusal(
                    "Source is read-only; use semantic controls to author the Form",
                );
                return true;
            }
            _ => return false,
        }
        if let Some(window) = &self.window {
            window.request_redraw();
        }
        true
    }
}
