//! Explicit selection among the installed structured presentation profiles.
use super::*;

#[no_mangle]
pub extern "C" fn conduit_browser_form_start_with_presentation(
    host_length: usize,
    boot_length: usize,
    source_length: usize,
    play_sequence: u64,
    presentation: u32,
) -> i32 {
    use crate::installed_browser::PresentationProfile;
    let presentation = match presentation {
        0 => PresentationProfile::Annotation,
        1 => PresentationProfile::Quantity,
        2 => PresentationProfile::NormalizedDurations,
        3 => PresentationProfile::PatternComparison,
        _ => return ERROR_INPUT,
    };
    start(
        host_length,
        boot_length,
        source_length,
        play_sequence,
        false,
        presentation,
    )
}

/// Read-only projection under the same exact structured profile used for Play.
#[no_mangle]
pub extern "C" fn conduit_browser_form_project_with_presentation(
    source_length: usize,
    sequence: u64,
    presentation: u32,
) -> i32 {
    use crate::installed_browser::PresentationProfile;
    let presentation = match presentation {
        0 => PresentationProfile::Annotation,
        1 => PresentationProfile::Quantity,
        2 => PresentationProfile::NormalizedDurations,
        3 => PresentationProfile::PatternComparison,
        _ => return ERROR_INPUT,
    };
    clear_output();
    if source_length == 0 || source_length > INPUT_BYTES {
        return ERROR_INPUT;
    }
    INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let result = core::str::from_utf8(&input[..source_length])
            .map_err(|_| "compact Tour Patchbay source is not UTF-8".to_owned())
            .and_then(|source| {
                super::super::compact_patchbay::project_with_presentation(
                    source,
                    sequence,
                    false,
                    presentation,
                )
            });
        input[..source_length].fill(0);
        match result {
            Ok(projection) => write_output(&projection)
                .map(|()| STATUS_READY)
                .unwrap_or(ERROR_OUTPUT),
            Err(message) => {
                let _ = write_output(&super::super::refusal(message));
                ERROR_PROJECTION
            }
        }
    })
}
