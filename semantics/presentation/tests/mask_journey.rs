use conduit_presentation::{
    actualize_mask_journey, MaskJourneyAction, MaskJourneyCursor, MaskJourneyEmbodiment,
    MaskJourneyOrderError, MASK_JOURNEY_ACTIONS,
};

struct Embodiment {
    name: &'static str,
}

impl MaskJourneyEmbodiment for Embodiment {
    type Outcome = String;
    type Error = ();

    fn perform(&mut self, action: MaskJourneyAction) -> Result<Self::Outcome, Self::Error> {
        let material_result = match (self.name, action) {
            ("hugo", MaskJourneyAction::WearAlternateMask) => "refused-no-graphical-host",
            ("hugo", MaskJourneyAction::InspectReplannedShow) => "spoken-plus-graphical-show",
            ("tiago", _) => "browser-dom-show",
            ("orifina", _) => "native-framebuffer-show",
            _ => "spoken-show",
        };
        Ok(material_result.into())
    }
}

#[test]
fn every_embodiment_must_advance_the_one_ordered_action_journey() {
    for _embodiment in ["orifina", "tiago", "hugo"] {
        let mut cursor = MaskJourneyCursor::new();
        for action in MASK_JOURNEY_ACTIONS {
            assert_eq!(cursor.next_action(), Some(action));
            cursor.advance(action).unwrap();
        }
        assert!(cursor.is_complete());
        assert_eq!(
            cursor.advance(MaskJourneyAction::InspectInitialShow),
            Err(MaskJourneyOrderError::AlreadyComplete)
        );
    }
}

#[test]
fn an_embodiment_cannot_replace_the_shared_journey_with_its_own_stage_play() {
    let mut cursor = MaskJourneyCursor::new();
    assert_eq!(
        cursor.advance(MaskJourneyAction::WearAlternateMask),
        Err(MaskJourneyOrderError::OutOfOrder {
            expected: MaskJourneyAction::InspectInitialShow,
            observed: MaskJourneyAction::WearAlternateMask,
        })
    );
    assert_eq!(
        MASK_JOURNEY_ACTIONS.map(MaskJourneyAction::id).as_slice(),
        &[
            "mask.inspect-initial-show",
            "mask.wear-alternate",
            "mask.prefer-alternate",
            "mask.withdraw-selected-route",
            "mask.inspect-unavailable-show",
            "mask.add-presentation-host",
            "mask.admit-replacement-plan",
            "mask.inspect-replanned-show",
            "mask.doff-alternate",
            "mask.inspect-restored-show",
        ]
    );
}

#[test]
fn one_loop_retains_honestly_different_outcomes_in_the_same_action_order() {
    let mut tracks = Vec::new();
    for name in ["orifina", "tiago", "hugo"] {
        let mut embodiment = Embodiment { name };
        let mut track = Vec::new();
        actualize_mask_journey(&mut embodiment, |action, outcome| {
            track.push((action, outcome.clone()));
        })
        .unwrap();
        tracks.push(track);
    }
    assert!(tracks.iter().all(|track| {
        track
            .iter()
            .map(|(action, _)| *action)
            .eq(MASK_JOURNEY_ACTIONS)
    }));
    assert_eq!(tracks[2][1].1, "refused-no-graphical-host");
    assert_ne!(tracks[0][0].1, tracks[1][0].1);
    assert_ne!(tracks[1][0].1, tracks[2][0].1);
}
