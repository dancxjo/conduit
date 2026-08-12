//! Composition of portable physical-key input and renderer-local shortcuts.

use super::*;

impl PatchbayApplication {
    pub(super) fn handle_keyboard_input(
        &mut self,
        event_loop: &ActiveEventLoop,
        event: &winit::event::KeyEvent,
    ) {
        // The portable adapter sees every physical transition independently of
        // renderer-local logical-key handling. Unsupported UI keys remain
        // explicit adapter refusals rather than localized semantic text.
        let _ = self
            .native_keyboard
            .observe(event.physical_key, event.state, event.repeat);
        if !event.state.is_pressed() {
            return;
        }
        let prewake_handled = match self.handle_prewake_key(&event.logical_key) {
            Ok(handled) => handled,
            Err(error) => {
                self.failure = Some(format!("PREWAKE control failed: {error}"));
                event_loop.exit();
                true
            }
        };
        let environment_handled = match if self.prewake.is_none() || self.prewake_environment_view {
            self.handle_environment_key(&event.logical_key)
        } else {
            Ok(false)
        } {
            Ok(handled) => handled,
            Err(error) => {
                self.failure = Some(format!("authored environment edit failed: {error}"));
                event_loop.exit();
                true
            }
        };
        if prewake_handled
            || environment_handled
            || self.handle_viewport_key(&event.logical_key)
            || self.handle_palette_key(&event.logical_key)
        {
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        } else if let Err(error) = self.handle_form_key(&event.logical_key) {
            if is_ordinary_form_refusal(&error) {
                self.publish_refusal(sentence_case(&error));
            } else {
                self.failure = Some(format!("canonical Form edit failed: {error}"));
                event_loop.exit();
            }
        }
    }
}

fn is_ordinary_form_refusal(error: &str) -> bool {
    error.starts_with("select a ")
}

fn sentence_case(message: &str) -> String {
    let mut characters = message.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_ordinary_user_preconditions_are_nonterminal() {
        assert!(is_ordinary_form_refusal(
            "select a Gear before duplicating it"
        ));
        assert!(!is_ordinary_form_refusal("interaction failed"));
        assert!(!is_ordinary_form_refusal(
            "cannot save /missing/path: permission denied"
        ));
    }
}
