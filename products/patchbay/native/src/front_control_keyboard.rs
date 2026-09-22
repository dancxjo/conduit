//! Keyboard focus and activation for authoritative visible Front-control actions.

use crate::{
    gui::GuiAction,
    gui_front_controls::{focused_front_action, front_action_count},
};
use winit::keyboard::{Key, ModifiersState, NamedKey};

pub(super) enum FaceControlKey {
    NotHandled,
    FocusChanged,
    Action(GuiAction),
}

pub(super) fn resolve_front_control_key(
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
            return Err("select a gear before using its front controls".into())
        }
        None => return Ok(FaceControlKey::NotHandled),
    };
    if matches!(key, Key::Character(character) if character.eq_ignore_ascii_case("j")) {
        let count = front_action_count(graph, selected);
        if count == 0 {
            return Err("select a gear with an actionable Front control".into());
        }
        *focus = focus.saturating_add(1) % count;
        return Ok(FaceControlKey::FocusChanged);
    }
    if matches!(key, Key::Named(NamedKey::Enter)) {
        let action = focused_front_action(graph, selected, *focus)
            .ok_or("select a gear with an actionable Front control")?;
        return Ok(FaceControlKey::Action(action));
    }
    Ok(FaceControlKey::NotHandled)
}
