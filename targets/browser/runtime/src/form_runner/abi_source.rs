//! Typed source-interaction admission for the shared browser execution slot.
use super::*;

/// Admits the exact editable source through the portable typed human-interaction
/// flow before parsing. The retained evidence contains byte count and exact
/// identities, never the submitted source text.
#[no_mangle]
pub extern "C" fn conduit_tour_admit_source_interaction(
    source_length: usize,
    sequence: u64,
) -> i32 {
    clear_output();
    SOURCE_INTERACTION.with(|slot| *slot.borrow_mut() = None);
    if source_length == 0 || source_length > INPUT_BYTES {
        return ERROR_INPUT;
    }
    INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let result = crate::source_interaction::admit_source(&input[..source_length], sequence);
        input[..source_length].fill(0);
        match result {
            Ok(evidence) => {
                if write_output(&evidence).is_err() {
                    return ERROR_OUTPUT;
                }
                SOURCE_INTERACTION.with(|slot| *slot.borrow_mut() = Some(evidence));
                STATUS_READY
            }
            Err(message) => {
                let _ = write_output(&super::super::refusal(message));
                ERROR_INTERACTION
            }
        }
    })
}
