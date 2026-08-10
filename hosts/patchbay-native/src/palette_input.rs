//! Renderer-local keyboard state for bounded palette search.

use super::PatchbayApplication;
use winit::keyboard::{Key, NamedKey};

impl PatchbayApplication {
    pub(super) fn handle_palette_key(&mut self, key: &Key) -> bool {
        match key {
            Key::Character(value) if value == "/" && !self.palette_search_active => {
                self.palette_search_active = true;
                true
            }
            Key::Character(value) if self.palette_search_active => {
                for character in value.chars() {
                    if self.palette_query.len() + character.len_utf8()
                        > patchbay_model::MAX_PALETTE_QUERY_BYTES
                    {
                        break;
                    }
                    self.palette_query.push(character);
                }
                true
            }
            Key::Named(NamedKey::Backspace) if self.palette_search_active => {
                self.palette_query.pop();
                true
            }
            Key::Named(NamedKey::Escape) if self.palette_search_active => {
                self.palette_query.clear();
                self.palette_search_active = false;
                true
            }
            Key::Named(NamedKey::Enter) if self.palette_search_active => {
                self.palette_search_active = false;
                true
            }
            _ => false,
        }
    }
}
