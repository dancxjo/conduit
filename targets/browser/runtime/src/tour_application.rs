//! Browser ABI for the shared Tour application transition model.

use std::cell::RefCell;

use conduit_tour_model::{TourApplicationAction, TourApplicationState};

const PREVIOUS: u8 = 1;
const NEXT: u8 = 2;
const RUN: u8 = 3;
const STOP: u8 = 4;
const RESTORE: u8 = 5;
const COMPLETE: u8 = 6;
const EDIT_SOURCE: u8 = 7;
const ERROR_ACTION: i32 = -1;
const ERROR_TRANSITION: i32 = -2;
const MAX_CHAPTER_BYTES: usize = 2 * 1024;

thread_local! {
    static APPLICATION: RefCell<TourApplicationState> = const {
        RefCell::new(TourApplicationState::canonical())
    };
    static CHAPTERS: RefCell<[u8; MAX_CHAPTER_BYTES]> = const { RefCell::new([0; MAX_CHAPTER_BYTES]) };
    static CHAPTERS_LEN: RefCell<usize> = const { RefCell::new(0) };
}

#[unsafe(no_mangle)]
pub extern "C" fn conduit_tour_application_chapters() -> i32 {
    let chapters = conduit_tour_model::TOUR_CHAPTERS
        .iter()
        .map(|chapter| {
            serde_json::json!({
                "identity": chapter.identity,
                "route": chapter.route,
                "companion": chapter.companion,
            })
        })
        .collect::<Vec<_>>();
    let Ok(encoded) = serde_json::to_vec(&chapters) else {
        return ERROR_TRANSITION;
    };
    if encoded.len() > MAX_CHAPTER_BYTES {
        return ERROR_TRANSITION;
    }
    CHAPTERS.with(|buffer| buffer.borrow_mut()[..encoded.len()].copy_from_slice(&encoded));
    CHAPTERS_LEN.with(|length| *length.borrow_mut() = encoded.len());
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn conduit_tour_application_chapters_ptr() -> usize {
    CHAPTERS.with(|chapters| chapters.borrow().as_ptr() as usize)
}

#[unsafe(no_mangle)]
pub extern "C" fn conduit_tour_application_chapters_len() -> usize {
    CHAPTERS_LEN.with(|length| *length.borrow())
}

#[unsafe(no_mangle)]
pub extern "C" fn conduit_tour_application_reset(chapter_count: u8) -> i32 {
    let Ok(state) = TourApplicationState::with_chapter_count(chapter_count) else {
        return ERROR_TRANSITION;
    };
    APPLICATION.with(|application| *application.borrow_mut() = state);
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn conduit_tour_application_open_chapter(chapter: u8) -> i32 {
    apply(TourApplicationAction::OpenChapter(chapter))
}

#[unsafe(no_mangle)]
pub extern "C" fn conduit_tour_application_apply(action: u8) -> i32 {
    let action = match action {
        PREVIOUS => TourApplicationAction::PreviousChapter,
        NEXT => TourApplicationAction::NextChapter,
        RUN => TourApplicationAction::Run,
        STOP => TourApplicationAction::Stop,
        RESTORE => TourApplicationAction::Restore,
        COMPLETE => TourApplicationAction::Complete,
        EDIT_SOURCE => {
            return APPLICATION.with(|application| {
                application
                    .borrow_mut()
                    .mark_source_edited()
                    .map(|()| 0)
                    .unwrap_or(ERROR_TRANSITION)
            });
        }
        _ => return ERROR_ACTION,
    };
    apply(action)
}

fn apply(action: TourApplicationAction) -> i32 {
    APPLICATION.with(|application| {
        application
            .borrow_mut()
            .apply(action)
            .map(|()| 0)
            .unwrap_or(ERROR_TRANSITION)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn conduit_tour_application_state() -> u32 {
    APPLICATION.with(|application| {
        let state = *application.borrow();
        u32::from(state.chapter)
            | (u32::from(state.chapter_count) << 8)
            | ((state.run as u32) << 16)
            | (u32::from(state.source_is_canonical) << 24)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_tour_model::{TourRunState, TOUR_PROJECTION_CONFORMANCE_ACTIONS};

    fn state() -> TourApplicationState {
        APPLICATION.with(|application| *application.borrow())
    }

    #[test]
    fn browser_projection_uses_shared_navigation_and_lifecycle_transitions() {
        assert_eq!(conduit_tour_application_reset(7), 0);
        for action in [NEXT, RUN, STOP, RESTORE, PREVIOUS, RUN] {
            assert_eq!(conduit_tour_application_apply(action), 0);
        }
        assert_eq!(state().chapter, 0);
        assert_eq!(state().run, TourRunState::Running);
        assert!(state().source_is_canonical);
        let mut expected = TourApplicationState::canonical();
        for action in TOUR_PROJECTION_CONFORMANCE_ACTIONS {
            expected.apply(action).unwrap();
        }
        assert_eq!(state(), expected);
    }
}
