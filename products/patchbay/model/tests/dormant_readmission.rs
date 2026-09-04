use conduit_core::{
    AuthorityGrantId, BootId, CapabilityId, GearId, HostId, ImplementationId, OfferGeneration,
    PlanId, SignId,
};
use conduit_planner::{CurrentDormantCandidate, DormantReadmissionEvidence};
use patchbay_model::{explain_dormant_readmission, DormantReadmissionExplanationError};

fn evidence() -> DormantReadmissionEvidence {
    DormantReadmissionEvidence {
        candidate: CurrentDormantCandidate {
            body_membership_id: "body/household/slow-laptop".into(),
            gear_id: GearId::from("dormant/sink"),
            host_id: HostId::from("host-slow"),
            boot_id: BootId::from("boot-slow-fresh"),
            offer_generation: OfferGeneration(5),
            capability_id: CapabilityId::from("host-slow/test-dormant-sink"),
            implementation_id: ImplementationId::from("test/host-slow/test/dormant-sink@1"),
            required_lines: vec![],
            resource_observation_signs: vec![SignId::from("sign/slow-cpu-fresh")],
            line_observation_signs: vec![SignId::from("sign/slow-serial-ready")],
            authority_grant_ids: vec![AuthorityGrantId::from("grant/host-slow/fresh")],
            unused_before: true,
            available_now: true,
        },
        previous_plan_id: PlanId::from("plan/preferred"),
        plan_id: PlanId::from("plan/returned"),
        selected_because_preferred_path_is_gone: true,
        historical_boot_reused: false,
        historical_authority_restored: false,
    }
}

#[test]
fn patchbay_names_unused_current_and_selection_truth_without_legacy_rank() {
    let explanation = explain_dormant_readmission(&evidence()).unwrap();
    assert!(explanation.unused_before);
    assert!(explanation.available_now);
    assert!(explanation.selected_because_preferred_path_is_gone);
    assert!(!explanation.historical_boot_reused);
    assert!(!explanation.historical_authority_restored);
    assert_eq!(explanation.boot_id, "boot-slow-fresh");
    assert_eq!(explanation.offer_generation, 5);
    assert_eq!(
        explanation.resource_observation_signs,
        ["sign/slow-cpu-fresh"]
    );
    assert_eq!(
        explanation.line_observation_signs,
        ["sign/slow-serial-ready"]
    );
    for phrase in [
        "unused before",
        "available now",
        "selected because the preferred path is gone",
        "Historical Boot and authority were not reused",
    ] {
        assert!(explanation.summary.contains(phrase));
    }
    assert!(!explanation.summary.contains("legacy rank"));
}

#[test]
fn patchbay_refuses_laundered_history_or_nonreplacement_evidence() {
    let mut reused_boot = evidence();
    reused_boot.historical_boot_reused = true;
    assert_eq!(
        explain_dormant_readmission(&reused_boot),
        Err(DormantReadmissionExplanationError::IncoherentEvidence)
    );

    let mut restored_authority = evidence();
    restored_authority.historical_authority_restored = true;
    assert_eq!(
        explain_dormant_readmission(&restored_authority),
        Err(DormantReadmissionExplanationError::IncoherentEvidence)
    );

    let mut same_plan = evidence();
    same_plan.plan_id = same_plan.previous_plan_id.clone();
    assert_eq!(
        explain_dormant_readmission(&same_plan),
        Err(DormantReadmissionExplanationError::IncoherentEvidence)
    );
}
