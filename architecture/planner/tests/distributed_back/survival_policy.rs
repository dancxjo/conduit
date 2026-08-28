use super::*;
use conduit_planner::{
    select_plan_with_survival_policy, triage_scarce_resource, ExplicitCriticality,
    ScarceResourceDisposition, SurvivalCandidate, SurvivalCandidateDisposition,
    SurvivalPlanningMode, SurvivalPlanningPolicy, SurvivalPolicyRefusal, SurvivalTradeoff,
    WorkloadResourceRequest,
};

const PROFILE: &str = "test/provider-generate@full-v1";

fn policy(mode: SurvivalPlanningMode) -> SurvivalPlanningPolicy {
    SurvivalPlanningPolicy {
        policy_id: match mode {
            SurvivalPlanningMode::Normal => "policy/voyager/normal@1",
            SurvivalPlanningMode::Survival => "policy/voyager/survival@1",
        }
        .into(),
        revision: 1,
        mode,
        normal_maximum_hops: 2,
        normal_maximum_latency_us: 10_000,
        admit_costly_full_profile: mode == SurvivalPlanningMode::Survival,
        admit_reviewed_degradation: false,
        tradeoffs: vec![
            SurvivalTradeoff::PreferFullProfile,
            SurvivalTradeoff::MinimizeSharedDependencyExposure,
            SurvivalTradeoff::MinimizeHopCount,
        ],
    }
}

fn candidate<'a>(
    plan: &'a conduit_core::Plan,
    current: bool,
    currently_available: bool,
) -> SurvivalCandidate<'a> {
    SurvivalCandidate {
        plan,
        semantic_profile: PROFILE,
        disposition: SurvivalCandidateDisposition::FullyCompatible,
        current,
        currently_available,
        authority_admitted: true,
        all_host_reservations_admitted: true,
        unavailable_prerequisites: 0,
        shared_dependency_exposures: 1,
        hop_count: 5,
        estimated_item_latency_us: 40_000,
        resource_units: 80,
    }
}

#[test]
fn same_damaged_candidates_refuse_normally_and_recover_under_explicit_survival_policy() {
    let (_, direct) = direct_plan();
    let (_, recursive) = plan_with_http_part(host("b", &[HTTP, DECODE]));
    let direct_snapshot = direct.clone();
    let recursive_snapshot = recursive.clone();
    let candidates = [
        candidate(&direct, true, false),
        candidate(&recursive, false, true),
    ];

    assert_eq!(
        select_plan_with_survival_policy(
            PROFILE,
            &candidates,
            &policy(SurvivalPlanningMode::Normal)
        ),
        Err(SurvivalPolicyRefusal::NormalCostEnvelopeExceeded)
    );
    let selected = select_plan_with_survival_policy(
        PROFILE,
        &candidates,
        &policy(SurvivalPlanningMode::Survival),
    )
    .unwrap();
    assert_eq!(selected.selected_plan_id, recursive.plan_id);
    assert_eq!(selected.previous_plan_id.as_ref(), Some(&direct.plan_id));
    assert!(selected.fresh_plan);
    assert_eq!(selected.mode, SurvivalPlanningMode::Survival);
    assert_eq!(selected.principal_tradeoffs.len(), 3);
    assert_eq!(direct, direct_snapshot, "the old Plan remains immutable");
    assert_eq!(
        recursive, recursive_snapshot,
        "the candidate Plan remains immutable"
    );
}

#[test]
fn same_truthful_candidates_select_different_immutable_plans_by_reviewed_policy() {
    let (_, direct) = direct_plan();
    let (_, recursive) = plan_with_http_part(host("b", &[HTTP, DECODE]));
    let mut direct_candidate = candidate(&direct, true, true);
    direct_candidate.hop_count = 0;
    direct_candidate.estimated_item_latency_us = 1_000;
    direct_candidate.resource_units = 4;
    direct_candidate.shared_dependency_exposures = 5;
    let mut recursive_candidate = candidate(&recursive, false, true);
    recursive_candidate.shared_dependency_exposures = 1;
    let candidates = [direct_candidate, recursive_candidate];

    let normal = select_plan_with_survival_policy(
        PROFILE,
        &candidates,
        &policy(SurvivalPlanningMode::Normal),
    )
    .unwrap();
    let survival = select_plan_with_survival_policy(
        PROFILE,
        &candidates,
        &policy(SurvivalPlanningMode::Survival),
    )
    .unwrap();
    assert_eq!(normal.selected_plan_id, direct.plan_id);
    assert!(!normal.fresh_plan);
    assert_eq!(survival.selected_plan_id, recursive.plan_id);
    assert!(survival.fresh_plan);
    assert_ne!(normal.selected_plan_id, survival.selected_plan_id);
}

#[test]
fn survival_policy_still_refuses_hard_semantics_authority_and_implicit_degradation() {
    let (_, recursive) = plan_with_http_part(host("b", &[HTTP, DECODE]));
    let survival = policy(SurvivalPlanningMode::Survival);

    let mut mismatch = candidate(&recursive, false, true);
    mismatch.semantic_profile = "test/provider-generate@weaker-v1";
    assert_eq!(
        select_plan_with_survival_policy(PROFILE, &[mismatch], &survival),
        Err(SurvivalPolicyRefusal::HardSemanticRequirementUnsatisfied)
    );

    let mut authority = candidate(&recursive, false, true);
    authority.authority_admitted = false;
    assert_eq!(
        select_plan_with_survival_policy(PROFILE, &[authority], &survival),
        Err(SurvivalPolicyRefusal::AuthorityUnavailable)
    );

    let mut degraded = candidate(&recursive, false, true);
    degraded.disposition = SurvivalCandidateDisposition::ReviewedDegraded {
        profile_id: "test/provider-generate@degraded-v1".into(),
        admission_policy_id: "policy/voyager/reviewed-degradation@1".into(),
    };
    assert_eq!(
        select_plan_with_survival_policy(PROFILE, &[degraded.clone()], &survival),
        Err(SurvivalPolicyRefusal::ReviewedDegradationRequired)
    );
    let mut explicit = survival;
    explicit.admit_reviewed_degradation = true;
    assert!(select_plan_with_survival_policy(PROFILE, &[degraded], &explicit).is_ok());
}

#[test]
fn scarce_resource_triage_uses_explicit_provenance_not_workload_names() {
    let requests = [
        WorkloadResourceRequest {
            workload_id: "alphabetically-first-but-deferrable".into(),
            resource_units: 4,
            criticality: ExplicitCriticality::Deferrable,
            policy_source_id: "policy/voyager/operator-reviewed@7".into(),
            policy_source_revision: 7,
        },
        WorkloadResourceRequest {
            workload_id: "z-last-but-essential".into(),
            resource_units: 4,
            criticality: ExplicitCriticality::Essential,
            policy_source_id: "policy/voyager/operator-reviewed@7".into(),
            policy_source_revision: 7,
        },
    ];
    let triage = triage_scarce_resource(4, &requests).unwrap();
    assert_eq!(triage.reserved_units, 4);
    assert_eq!(triage.decisions[0].workload_id, "z-last-but-essential");
    assert_eq!(
        triage.decisions[0].disposition,
        ScarceResourceDisposition::Reserved
    );
    assert_eq!(
        triage.decisions[1].disposition,
        ScarceResourceDisposition::RefusedCapacity
    );
    assert_eq!(
        triage.decisions[0].policy_source_id,
        "policy/voyager/operator-reviewed@7"
    );
}

#[test]
fn duplicate_policy_dimensions_and_unproven_degradation_are_invalid_inputs() {
    let (_, recursive) = plan_with_http_part(host("b", &[HTTP, DECODE]));
    let mut duplicate = policy(SurvivalPlanningMode::Survival);
    duplicate.tradeoffs = vec![
        SurvivalTradeoff::MinimizeLatency,
        SurvivalTradeoff::MinimizeLatency,
    ];
    assert_eq!(
        select_plan_with_survival_policy(
            PROFILE,
            &[candidate(&recursive, false, true)],
            &duplicate
        ),
        Err(SurvivalPolicyRefusal::InvalidPolicy)
    );

    let mut unproven = candidate(&recursive, false, true);
    unproven.disposition = SurvivalCandidateDisposition::ReviewedDegraded {
        profile_id: String::new(),
        admission_policy_id: String::new(),
    };
    assert_eq!(
        select_plan_with_survival_policy(
            PROFILE,
            &[unproven],
            &policy(SurvivalPlanningMode::Survival)
        ),
        Err(SurvivalPolicyRefusal::InvalidCandidateSet)
    );
}
