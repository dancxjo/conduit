//! Todo-owned admission of initial Forms for authored `scan`.

use alloc::{format, string::String};

use crate::{TodoState, TODO_STATE_INFO_ID};

/// Register one empty list as a checked source literal for Todo scan.
/// The compiler sees only the exact Kind and the owner's validated Form.
pub fn admit_empty_todo_initial(
    catalog: &mut conduit_plot::StartupCatalog,
    title: &str,
) -> Result<(), String> {
    let state = TodoState::new(String::from(title))
        .map_err(|refusal| format!("invalid Todo initial title: {refusal:?}"))?;
    let bytes = state
        .encode_info()
        .map_err(|refusal| format!("invalid Todo initial Form: {refusal:?}"))?;
    let literal = conduit_plot::text_startup_literal(title);
    catalog.insert_exact_initial_info(
        conduit_core::kind_id(TODO_STATE_INFO_ID),
        literal,
        bytes,
        |bytes| TodoState::decode_info(bytes).is_ok(),
    )
}
