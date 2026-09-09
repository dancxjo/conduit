//! Native card geometry gates the shared semantic pointer action.
use super::{TourPointerOutcome, TourProduct};
use conduit_semantic_catalog::{NormalizedPointerSample, normalized_pointer_value};

impl TourProduct {
    pub fn route_workspace_pointer(
        &mut self,
        sample: NormalizedPointerSample,
        width: u16,
        height: u16,
    ) -> Result<Option<TourPointerOutcome>, &'static str> {
        normalized_pointer_value(sample).map_err(|_| "tour-pointer-invalid")?;
        let layout =
            crate::tour_workspace::layout_for_state(width, height, self.controller.state())
                .map_err(|_| "tour-pointer-layout-refused")?;
        let coordinate = |value: i64, extent: u16| {
            ((value as u64 * u64::from(extent) / 1_000_000).min(u64::from(extent - 1))) as u16
        };
        if !crate::tour_workspace::hits_card(
            &layout,
            coordinate(sample.position_x, width),
            coordinate(sample.position_y, height),
        )
        .map_err(|_| "tour-pointer-layout-refused")?
        {
            self.controller
                .leave_pointer(sample.sequence)
                .map_err(|_| "tour-pointer-refused")?;
            return Ok(None);
        }
        self.accept_pointer(sample, width, height).map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_card_gaps_clear_hover_without_changing_selection() {
        let mut product = TourProduct::canonical(1);
        product
            .controller
            .request(&conduit_presentation::ApplicationEvent {
                revision: 1,
                action: conduit_tour_model::OPEN_PATCHBAY_ACTION_ID.into(),
                kind: conduit_presentation::ApplicationEventKind::Activate,
                value: alloc::vec![],
            })
            .unwrap();
        let sample = |x, y, sequence| NormalizedPointerSample {
            position_x: x,
            position_y: y,
            delta_x: 0,
            delta_y: 0,
            primary_pressed: true,
            coalesced: 0,
            dropped: 0,
            queue_capacity: 4,
            sequence,
        };
        assert!(
            product
                .route_workspace_pointer(sample(700_000, 100_000, 1), 640, 480)
                .unwrap()
                .is_none()
        );
        assert!(
            product
                .controller
                .state()
                .selected_patchbay_subject
                .is_none()
        );
        assert!(
            product
                .route_workspace_pointer(sample(700_000, 200_000, 2), 640, 480)
                .unwrap()
                .is_some()
        );
        let selected = product.controller.state().selected_patchbay_subject.clone();
        assert!(
            product
                .route_workspace_pointer(sample(10_000, 10_000, 3), 640, 480)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            product.controller.state().selected_patchbay_subject,
            selected
        );
        assert!(
            product
                .controller
                .state()
                .hovered_patchbay_subject
                .is_none()
        );
        let before = product.controller.state().clone();
        assert!(
            product
                .route_workspace_pointer(sample(10_000, 10_000, 3), 640, 480)
                .is_err()
        );
        assert_eq!(product.controller.state(), &before);
    }
}
