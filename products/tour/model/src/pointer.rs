use alloc::string::{String, ToString};

use conduit_semantic_catalog::{
    GeneralizedInputRefusal, NormalizedPointerSample, normalized_pointer_value,
};

use crate::{
    CANONICAL_PATCHBAY_GEARS, TourLayoutRefusal, TourRect, TourWorkspaceController,
    TourWorkspaceLayout, TourWorkspacePhase, TourWorkspaceRefusal,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TourPointerOutcome {
    Hovered { subject: String },
    Selected { subject: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TourPointerRefusal {
    Semantic(GeneralizedInputRefusal),
    Layout(TourLayoutRefusal),
    Workspace(TourWorkspaceRefusal),
    PatchbayClosed,
    StaleSequence,
    OutsidePatchbay,
}

impl TourWorkspaceController {
    pub fn accept_pointer(
        &mut self,
        sample: NormalizedPointerSample,
        layout: &TourWorkspaceLayout,
    ) -> Result<TourPointerOutcome, TourPointerRefusal> {
        normalized_pointer_value(sample).map_err(TourPointerRefusal::Semantic)?;
        layout.validate().map_err(TourPointerRefusal::Layout)?;
        if self.state().phase != TourWorkspacePhase::PatchbayOpen {
            return Err(TourPointerRefusal::PatchbayClosed);
        }
        if self
            .last_pointer_sequence()
            .is_some_and(|previous| sample.sequence <= previous)
        {
            return Err(TourPointerRefusal::StaleSequence);
        }
        let x = normalized_coordinate(sample.position_x, layout.viewport.x, layout.viewport.width);
        let y = normalized_coordinate(sample.position_y, layout.viewport.y, layout.viewport.height);
        if !contains(layout.patchbay, x, y) {
            return Err(TourPointerRefusal::OutsidePatchbay);
        }
        let local_x = x - layout.patchbay.x;
        let index = (usize::from(local_x) * CANONICAL_PATCHBAY_GEARS.len()
            / usize::from(layout.patchbay.width))
        .min(CANONICAL_PATCHBAY_GEARS.len() - 1);
        let subject = CANONICAL_PATCHBAY_GEARS[index].to_string();
        let selected = sample.primary_pressed.then(|| subject.clone());
        let revision = self
            .next_revision()
            .map_err(TourPointerRefusal::Workspace)?;
        self.commit_pointer(sample.sequence, subject.clone(), selected, revision);
        Ok(if sample.primary_pressed {
            TourPointerOutcome::Selected { subject }
        } else {
            TourPointerOutcome::Hovered { subject }
        })
    }
}

fn normalized_coordinate(value: i64, origin: u16, extent: u16) -> u16 {
    let offset = (u64::try_from(value).unwrap_or(0) * u64::from(extent) / 1_000_000)
        .min(u64::from(extent - 1));
    origin + u16::try_from(offset).unwrap_or(extent - 1)
}

fn contains(rect: TourRect, x: u16, y: u16) -> bool {
    x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;
    use conduit_presentation::{ApplicationEvent, ApplicationEventKind};

    use super::*;
    use crate::OPEN_PATCHBAY_ACTION_ID;

    fn sample(x: i64, y: i64, pressed: bool, sequence: u64) -> NormalizedPointerSample {
        NormalizedPointerSample {
            position_x: x,
            position_y: y,
            delta_x: 0,
            delta_y: 0,
            primary_pressed: pressed,
            coalesced: 0,
            dropped: 0,
            queue_capacity: 4,
            sequence,
        }
    }

    fn opened() -> TourWorkspaceController {
        let mut controller = TourWorkspaceController::canonical(1);
        controller
            .request(&ApplicationEvent {
                revision: 1,
                action: OPEN_PATCHBAY_ACTION_ID.into(),
                kind: ApplicationEventKind::Activate,
                value: Vec::new(),
            })
            .unwrap();
        controller
    }

    #[test]
    fn portable_pointer_hover_and_press_select_exact_canonical_gears() {
        let layout = TourWorkspaceLayout::default_for(640, 480).unwrap();
        let mut controller = opened();
        assert_eq!(
            controller.accept_pointer(sample(500_000, 100_000, false, 1), &layout),
            Ok(TourPointerOutcome::Hovered {
                subject: "meet-one-gear/words".into()
            })
        );
        assert_eq!(
            controller.accept_pointer(sample(700_000, 100_000, true, 2), &layout),
            Ok(TourPointerOutcome::Selected {
                subject: "meet-one-gear/change".into()
            })
        );
        assert_eq!(
            controller.state().selected_patchbay_subject.as_deref(),
            Some("meet-one-gear/change")
        );
        let view = controller.state().presentation().unwrap().lower().unwrap();
        assert!(view.nodes.iter().any(|node| {
            node.key == "patchbay" && node.text.contains("selected meet-one-gear/change")
        }));
    }

    #[test]
    fn stale_outside_and_closed_samples_do_not_mutate_application_state() {
        let layout = TourWorkspaceLayout::default_for(640, 480).unwrap();
        let mut controller = opened();
        controller
            .accept_pointer(sample(600_000, 100_000, false, 4), &layout)
            .unwrap();
        let before = controller.state().clone();
        assert_eq!(
            controller.accept_pointer(sample(600_000, 100_000, true, 4), &layout),
            Err(TourPointerRefusal::StaleSequence)
        );
        assert_eq!(controller.state(), &before);
        assert_eq!(
            controller.accept_pointer(sample(100_000, 900_000, true, 5), &layout),
            Err(TourPointerRefusal::OutsidePatchbay)
        );
        assert_eq!(controller.state(), &before);

        let mut closed = TourWorkspaceController::canonical(1);
        assert_eq!(
            closed.accept_pointer(sample(600_000, 100_000, true, 1), &layout),
            Err(TourPointerRefusal::PatchbayClosed)
        );
    }
}
