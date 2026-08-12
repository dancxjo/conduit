//! Keyboard focus and activation for authoritative visible Face-control actions.

use crate::{
    gui::GuiAction,
    gui_face_controls::{face_action_count, focused_face_action},
};
use winit::keyboard::{Key, ModifiersState, NamedKey};

pub(super) enum FaceControlKey {
    NotHandled,
    FocusChanged,
    Action(GuiAction),
}

pub(super) fn resolve_face_control_key(
    key: &Key,
    modifiers: ModifiersState,
    graph: Option<&patchbay_model::PatchbayGraph>,
    linear_view: bool,
    selected: Option<&str>,
    focus: &mut usize,
) -> Result<FaceControlKey, String> {
    if graph.is_none() || linear_view || !modifiers.control_key() {
        return Ok(FaceControlKey::NotHandled);
    }
    if matches!(key, Key::Character(character) if character.eq_ignore_ascii_case("i")) {
        return Ok(FaceControlKey::Action(GuiAction::ToggleExactIdentity));
    }
    let graph = graph.expect("presence checked above");
    let selected = match selected {
        Some(selected) => selected,
        None if matches!(key, Key::Character(character) if character.eq_ignore_ascii_case("j"))
            || matches!(key, Key::Named(NamedKey::Enter)) =>
        {
            return Err("select a Gear before using its Face controls".into())
        }
        None => return Ok(FaceControlKey::NotHandled),
    };
    if matches!(key, Key::Character(character) if character.eq_ignore_ascii_case("j")) {
        let count = face_action_count(graph, selected);
        if count == 0 {
            return Err("select a Gear with an actionable Face control".into());
        }
        *focus = focus.saturating_add(1) % count;
        return Ok(FaceControlKey::FocusChanged);
    }
    if matches!(key, Key::Named(NamedKey::Enter)) {
        let action = focused_face_action(graph, selected, *focus)
            .ok_or("select a Gear with an actionable Face control")?;
        return Ok(FaceControlKey::Action(action));
    }
    Ok(FaceControlKey::NotHandled)
}
