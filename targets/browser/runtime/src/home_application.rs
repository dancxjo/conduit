//! Browser ABI for the portable Home transition and presentation model.

use std::cell::RefCell;

use conduit_home_model::{HomeAction, HomeEvent, HomeModel, HomeView, MAX_COMMAND_BYTES};

const INSTALLED_FORMS: [&str; 4] = ["Hello", "Text Lab", "Clock", "Count"];
const MAX_INPUT_BYTES: usize = MAX_COMMAND_BYTES;
const MAX_VIEW_BYTES: usize = 8 * 1024;
const ERROR_INPUT: i32 = -1;
const ERROR_ACTION: i32 = -2;
const ERROR_VIEW: i32 = -3;

const REQUEST_NONE: i32 = 0;
const REQUEST_TOUR: i32 = 1;
const REQUEST_PATCHBAY: i32 = 2;
const REQUEST_CRECHE: i32 = 3;
const REQUEST_OPEN_FORM: i32 = 4;
const REQUEST_RUN_FORM: i32 = 5;

struct BrowserHome {
    model: HomeModel,
    revision: u32,
}

impl BrowserHome {
    fn new() -> Self {
        Self {
            model: HomeModel::new(),
            revision: 1,
        }
    }

    fn finish(&mut self, action: HomeAction) -> i32 {
        if action != HomeAction::Unchanged {
            self.revision = self.revision.wrapping_add(1).max(1);
        }
        match action {
            HomeAction::Unchanged | HomeAction::Changed => REQUEST_NONE,
            HomeAction::OpenTour => REQUEST_TOUR,
            HomeAction::OpenPatchbay => REQUEST_PATCHBAY,
            HomeAction::OpenCreche => REQUEST_CRECHE,
            HomeAction::OpenForm(_) => REQUEST_OPEN_FORM,
            HomeAction::RunForm(_) => REQUEST_RUN_FORM,
        }
    }

    fn activate_form(&mut self, index: usize) -> i32 {
        if self.model.view() != HomeView::Forms || index >= INSTALLED_FORMS.len() {
            return ERROR_ACTION;
        }
        while self.model.selected_form_index() != index {
            let _ = self.model.accept(HomeEvent::Next, &INSTALLED_FORMS);
        }
        let action = self.model.accept(HomeEvent::Activate, &INSTALLED_FORMS);
        self.finish(action)
    }
}

thread_local! {
    static HOME: RefCell<BrowserHome> = RefCell::new(BrowserHome::new());
    static INPUT: RefCell<[u8; MAX_INPUT_BYTES]> = const { RefCell::new([0; MAX_INPUT_BYTES]) };
    static VIEW: RefCell<[u8; MAX_VIEW_BYTES]> = const { RefCell::new([0; MAX_VIEW_BYTES]) };
    static VIEW_LEN: RefCell<usize> = const { RefCell::new(0) };
}

#[unsafe(no_mangle)]
pub extern "C" fn conduit_home_reset() -> i32 {
    HOME.with(|home| *home.borrow_mut() = BrowserHome::new());
    VIEW_LEN.with(|length| *length.borrow_mut() = 0);
    REQUEST_NONE
}

#[unsafe(no_mangle)]
pub extern "C" fn conduit_home_input_ptr() -> usize {
    INPUT.with(|input| input.borrow().as_ptr() as usize)
}

#[unsafe(no_mangle)]
pub extern "C" fn conduit_home_submit(length: usize) -> i32 {
    with_input(length, |input, home| {
        let action = home.model.submit_text(input, &INSTALLED_FORMS);
        home.finish(action)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn conduit_home_apply_action(length: usize) -> i32 {
    with_input(length, |identity, home| {
        let action = match identity {
            "home.open-tour" => home.model.submit_text("open tour", &INSTALLED_FORMS),
            "home.open-patchbay" => home.model.submit_text("open patchbay", &INSTALLED_FORMS),
            "home.open-forms" => home.model.submit_text("open forms", &INSTALLED_FORMS),
            "home.open-body" => home.model.submit_text("open body", &INSTALLED_FORMS),
            "home.open-creche" => home.model.submit_text("open creche", &INSTALLED_FORMS),
            "home.open-prompt" => home.model.submit_text("open prompt", &INSTALLED_FORMS),
            _ => {
                let Some(index) = identity
                    .strip_prefix("home.open-form.")
                    .and_then(|value| value.parse::<usize>().ok())
                else {
                    return ERROR_ACTION;
                };
                return home.activate_form(index);
            }
        };
        home.finish(action)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn conduit_home_view() -> i32 {
    VIEW_LEN.with(|length| *length.borrow_mut() = 0);
    let encoded = HOME.with(|home| {
        let home = home.borrow();
        home.model
            .presentation(home.revision, &INSTALLED_FORMS)
            .lower()
            .and_then(|view| view.encode().map_err(Into::into))
    });
    let Ok(encoded) = encoded else {
        return ERROR_VIEW;
    };
    if encoded.len() > MAX_VIEW_BYTES {
        return ERROR_VIEW;
    }
    VIEW.with(|view| view.borrow_mut()[..encoded.len()].copy_from_slice(&encoded));
    VIEW_LEN.with(|length| *length.borrow_mut() = encoded.len());
    REQUEST_NONE
}

#[unsafe(no_mangle)]
pub extern "C" fn conduit_home_view_ptr() -> usize {
    VIEW.with(|view| view.borrow().as_ptr() as usize)
}

#[unsafe(no_mangle)]
pub extern "C" fn conduit_home_view_len() -> usize {
    VIEW_LEN.with(|length| *length.borrow())
}

fn with_input(function_length: usize, apply: impl FnOnce(&str, &mut BrowserHome) -> i32) -> i32 {
    if function_length == 0 || function_length > MAX_INPUT_BYTES {
        return ERROR_INPUT;
    }
    INPUT.with(|input| {
        let input = input.borrow();
        let Ok(text) = core::str::from_utf8(&input[..function_length]) else {
            return ERROR_INPUT;
        };
        HOME.with(|home| apply(text, &mut home.borrow_mut()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_home_model::{JOURNEY_STEP_IDS, PATCHBAY_OPENED_STEP_ID};
    use conduit_presentation::ApplicationView;

    fn input(text: &str) {
        INPUT.with(|input| input.borrow_mut()[..text.len()].copy_from_slice(text.as_bytes()));
    }

    fn submit(text: &str) -> i32 {
        input(text);
        conduit_home_submit(text.len())
    }

    fn action(identity: &str) -> i32 {
        input(identity);
        conduit_home_apply_action(identity.len())
    }

    #[test]
    fn browser_abi_enacts_the_shared_home_journey() {
        assert_eq!(conduit_home_reset(), REQUEST_NONE);
        assert_eq!(JOURNEY_STEP_IDS.len(), 8);
        assert_eq!(action("home.open-forms"), REQUEST_NONE);
        assert_eq!(action("home.open-form.1"), REQUEST_OPEN_FORM);
        assert_eq!(submit("open prompt"), REQUEST_NONE);
        assert_eq!(submit("run hello"), REQUEST_RUN_FORM);
        assert_eq!(submit("home"), REQUEST_NONE);
        assert_eq!(action("home.open-patchbay"), REQUEST_PATCHBAY);
        assert_eq!(PATCHBAY_OPENED_STEP_ID, JOURNEY_STEP_IDS[6]);
        assert_eq!(submit("home"), REQUEST_NONE);
    }

    #[test]
    fn browser_abi_exports_the_portable_semantic_view() {
        conduit_home_reset();
        assert_eq!(conduit_home_view(), REQUEST_NONE);
        let decoded = VIEW.with(|view| {
            ApplicationView::decode(&view.borrow()[..conduit_home_view_len()]).unwrap()
        });
        assert_eq!(decoded.actions.len(), 6);
        assert!(decoded.nodes.iter().any(|node| node.key == "applications"));
    }

    #[test]
    fn browser_abi_refuses_unbounded_or_unknown_input() {
        assert_eq!(conduit_home_submit(0), ERROR_INPUT);
        assert_eq!(conduit_home_submit(MAX_INPUT_BYTES + 1), ERROR_INPUT);
        input("unknown");
        assert_eq!(conduit_home_apply_action(7), ERROR_ACTION);
    }
}
