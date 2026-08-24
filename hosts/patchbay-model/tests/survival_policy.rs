use conduit_planner::{
    SurvivalCandidateDisposition, SurvivalCandidateEvidence, SurvivalPlanSelection,
    SurvivalPlanningMode, SurvivalTradeoff,
};
use patchbay_model::explain_survival_plan_selection;

#[test]
fn patchbay_names_policy_tradeoffs_and_fresh_plan_without_a_fallback_flag() {
    let selection = SurvivalPlanSelection {
        policy_id: "policy/voyager/survival@1".into(),
        policy_revision: 3,
        mode: SurvivalPlanningMode::Survival,
        selected_plan_id: conduit_core::PlanId::from("plan/surviving-five-hop"),
        previous_plan_id: Some(conduit_core::PlanId::from("plan/lost-direct")),
        fresh_plan: true,
        selected_disposition: SurvivalCandidateDisposition::FullyCompatible,
        principal_tradeoffs: vec![
            SurvivalTradeoff::PreferFullProfile,
            SurvivalTradeoff::MinimizeSharedDependencyExposure,
            SurvivalTradeoff::MinimizeHopCount,
        ],
        candidate_evidence: vec![
            (
                conduit_core::PlanId::from("plan/lost-direct"),
                SurvivalCandidateEvidence::RejectedUnavailablePrerequisite,
            ),
            (
                conduit_core::PlanId::from("plan/surviving-five-hop"),
                SurvivalCandidateEvidence::Selected,
            ),
        ],
    };
    let explanation = explain_survival_plan_selection(&selection).unwrap();
    assert_eq!(explanation.mode, "survival");
    assert_eq!(explanation.profile_disposition, "full-profile");
    assert!(explanation.fresh_plan);
    assert_eq!(explanation.principal_tradeoffs.len(), 3);
    assert!(explanation.summary.contains("ordinary Plan"));
    assert!(explanation.summary.contains("Hard semantics"));
    assert!(!explanation.summary.contains("fallback=true"));
}
