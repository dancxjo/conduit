use alloc::vec;

use super::{
    bind_active_play, bind_clue, bind_presentation, mandatory_clue_storage_requirement, BootId,
    ClueStorageBudget, ExpectedClue, HostId, PlacementId, PlanId,
};

#[test]
fn mandatory_clue_budget_counts_items_and_identity_bytes_independently() {
    let clue = vec![
        ExpectedClue::PlanFragmentReceived,
        ExpectedClue::PlacementPrepared(PlacementId::from("abc")),
        ExpectedClue::PlanTerminal,
    ];
    assert_eq!(
        mandatory_clue_storage_requirement(&clue),
        Some(ClueStorageBudget {
            item_capacity: 3,
            byte_capacity: 6,
        })
    );
}

#[test]
fn execution_identity_chain_keeps_plan_play_clue_and_presentation_distinct() {
    let plan_id = PlanId::from("plan/exact");
    let host_id = HostId::from("host/exact");
    let boot_id = BootId::from("boot/exact");
    let active = bind_active_play(&plan_id, &host_id, &boot_id, 7);
    let clue = bind_clue(&host_id, &boot_id, Some(&active.active_play_id), 11);
    let presentation = bind_presentation(
        &active.active_play_id,
        &PlacementId::from("placement/show"),
        3,
    );

    assert_eq!(active.plan_id, plan_id);
    assert_eq!(clue.active_play_id, Some(active.active_play_id.clone()));
    assert_eq!(presentation.active_play_id, active.active_play_id);
    assert_ne!(active.active_play_id.as_str(), plan_id.as_str());
    assert_ne!(clue.clue_id.as_str(), plan_id.as_str());
    assert_ne!(presentation.presentation_id.as_str(), plan_id.as_str());
    assert_ne!(clue.clue_id.as_str(), presentation.presentation_id.as_str());
    assert_ne!(
        bind_active_play(&plan_id, &host_id, &boot_id, 8).active_play_id,
        active.active_play_id
    );
    assert_ne!(
        bind_active_play(&plan_id, &host_id, &BootId::from("boot/restarted"), 7).active_play_id,
        active.active_play_id
    );
}
