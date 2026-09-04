//! Renderer-local keyboard adapter for the finite authoritative Gear chooser.

use super::PatchbayApplication;
use crate::{gui::GuiAction, palette_state::PaletteMove};
use winit::keyboard::{Key, NamedKey};

impl PatchbayApplication {
    pub(super) fn handle_palette_key(&mut self, key: &Key) -> bool {
        let result = match key {
            Key::Character(value) if value == "/" && !self.palette.search_active() => {
                self.palette.focus();
                return true;
            }
            Key::Character(value) if self.palette.search_active() => self.palette.append(value),
            Key::Named(NamedKey::Backspace) if self.palette.search_active() => {
                self.palette.backspace();
                return true;
            }
            Key::Named(NamedKey::Escape) if self.palette.search_active() => {
                self.palette.exit_search();
                return true;
            }
            Key::Named(NamedKey::ArrowUp) if self.palette.search_active() => {
                self.palette.move_selection(PaletteMove::Previous)
            }
            Key::Named(NamedKey::ArrowDown) if self.palette.search_active() => {
                self.palette.move_selection(PaletteMove::Next)
            }
            Key::Named(NamedKey::Enter) if self.palette.search_active() => {
                self.place_selected_palette_kind();
                return true;
            }
            _ => return false,
        };
        if let Err(error) = result {
            self.publish_refusal(error.message());
        }
        true
    }

    fn place_selected_palette_kind(&mut self) {
        let result = self.palette.selected_kind().and_then(|kind| {
            let visible_subject_count = self
                .graphical_form
                .as_ref()
                .map(|graph| graph.gears.len() + graph.compositions.len())
                .unwrap_or(0);
            crate::palette_state::PaletteChooser::keyboard_target(visible_subject_count)
                .map(|target| (kind, target))
        });
        match result {
            Ok((kind, target)) => {
                if let Err(error) =
                    self.handle_gui_action(GuiAction::PlacePaletteKind { kind, target })
                {
                    self.publish_refusal(format!("Palette placement refused: {error}"));
                }
            }
            Err(error) => self.publish_refusal(error.message()),
        }
    }
}
