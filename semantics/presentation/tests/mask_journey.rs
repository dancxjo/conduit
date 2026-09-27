use conduit_presentation::{
    MaskJourneyAction, MaskJourneyCursor, MaskJourneyOrderError, MASK_JOURNEY_ACTIONS,
};

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
