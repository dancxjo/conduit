use std::cell::RefCell;

use conduit_tour_model::TourPageNavigation;

const MAX_VIEW_BYTES: usize = 4 * 1024;
const ERROR_NAVIGATION: i32 = -1;
const ERROR_VIEW: i32 = -2;

thread_local! {
    static VIEW: RefCell<[u8; MAX_VIEW_BYTES]> = const { RefCell::new([0; MAX_VIEW_BYTES]) };
    static VIEW_LEN: RefCell<usize> = const { RefCell::new(0) };
}

#[unsafe(no_mangle)]
pub extern "C" fn conduit_tour_navigation_view(
    revision: u32,
    current_page: u32,
    page_count: u32,
) -> i32 {
    VIEW_LEN.with(|length| *length.borrow_mut() = 0);
    let encoded = match (TourPageNavigation {
        revision,
        current_page,
        page_count,
    })
    .presentation()
    .and_then(|view| view.lower())
    .and_then(|view| view.encode().map_err(Into::into))
    {
        Ok(encoded) if encoded.len() <= MAX_VIEW_BYTES => encoded,
        Err(conduit_presentation::SemanticPresentationRefusal::InvalidNavigation) => {
            return ERROR_NAVIGATION
        }
        _ => return ERROR_VIEW,
    };
    VIEW.with(|view| view.borrow_mut()[..encoded.len()].copy_from_slice(&encoded));
    VIEW_LEN.with(|length| *length.borrow_mut() = encoded.len());
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn conduit_tour_navigation_view_ptr() -> usize {
    VIEW.with(|view| view.borrow().as_ptr() as usize)
}

#[unsafe(no_mangle)]
pub extern "C" fn conduit_tour_navigation_view_len() -> usize {
    VIEW_LEN.with(|length| *length.borrow())
}

#[cfg(test)]
mod tests {
    use conduit_presentation::{ApplicationComponent, ApplicationView};

    use super::*;

    #[test]
    fn browser_exports_the_tour_owned_navigation_view() {
        assert_eq!(conduit_tour_navigation_view(9, 1, 3), 0);
        let decoded = VIEW.with(|view| {
            ApplicationView::decode(&view.borrow()[..conduit_tour_navigation_view_len()]).unwrap()
        });
        assert_eq!(decoded.revision, 9);
        assert!(decoded.nodes.iter().any(|node| {
            node.key == "navigation" && node.component == ApplicationComponent::Navigation
        }));
        assert!(decoded
            .nodes
            .iter()
            .any(|node| node.key == "previous" && node.action.is_some()));
        assert!(decoded
            .nodes
            .iter()
            .any(|node| node.key == "next" && node.action.is_some()));
        assert_eq!(conduit_tour_navigation_view(10, 3, 3), ERROR_NAVIGATION);
        assert_eq!(conduit_tour_navigation_view_len(), 0);
    }
}
