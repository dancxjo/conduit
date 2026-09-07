use std::cell::RefCell;

use conduit_tour_model::{TourWorkspacePhase, TourWorkspaceState};

const MAX_VIEW_BYTES: usize = 16 * 1024;
const ERROR_PHASE: i32 = -1;
const ERROR_VIEW: i32 = -2;

thread_local! {
    static VIEW: RefCell<[u8; MAX_VIEW_BYTES]> = const { RefCell::new([0; MAX_VIEW_BYTES]) };
    static VIEW_LEN: RefCell<usize> = const { RefCell::new(0) };
}

#[unsafe(no_mangle)]
pub extern "C" fn conduit_tour_workspace_view(revision: u32, phase: u8) -> i32 {
    VIEW_LEN.with(|length| *length.borrow_mut() = 0);
    let phase = match phase {
        1 => TourWorkspacePhase::LessonReady,
        2 => TourWorkspacePhase::ResultVisible,
        3 => TourWorkspacePhase::PatchbayOpen,
        _ => return ERROR_PHASE,
    };
    let encoded = match TourWorkspaceState::canonical(revision, phase)
        .presentation()
        .and_then(|view| view.lower())
        .and_then(|view| view.encode().map_err(Into::into))
    {
        Ok(encoded) if encoded.len() <= MAX_VIEW_BYTES => encoded,
        _ => return ERROR_VIEW,
    };
    VIEW.with(|view| view.borrow_mut()[..encoded.len()].copy_from_slice(&encoded));
    VIEW_LEN.with(|length| *length.borrow_mut() = encoded.len());
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn conduit_tour_workspace_view_ptr() -> usize {
    VIEW.with(|view| view.borrow().as_ptr() as usize)
}

#[unsafe(no_mangle)]
pub extern "C" fn conduit_tour_workspace_view_len() -> usize {
    VIEW_LEN.with(|length| *length.borrow())
}

#[cfg(test)]
mod tests {
    use conduit_presentation::{ApplicationComponent, ApplicationView};

    use super::*;

    #[test]
    fn browser_exports_the_tour_owned_portable_view() {
        assert_eq!(conduit_tour_workspace_view(9, 1), 0);
        let decoded = VIEW.with(|view| {
            ApplicationView::decode(&view.borrow()[..conduit_tour_workspace_view_len()]).unwrap()
        });
        assert_eq!(decoded.revision, 9);
        assert!(decoded.nodes.iter().any(|node| {
            node.key == "patchbay" && node.component == ApplicationComponent::PatchbayCanvas
        }));
        assert_eq!(conduit_tour_workspace_view(9, 0), ERROR_PHASE);
        assert_eq!(conduit_tour_workspace_view_len(), 0);
    }
}
