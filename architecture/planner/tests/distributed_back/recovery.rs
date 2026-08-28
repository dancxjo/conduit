use super::*;
use conduit_planner::{
    RecursiveRecoveryCandidate, RecursiveRecoveryLimits, RecursiveRecoveryRefusal,
};

fn limits() -> RecursiveRecoveryLimits {
    RecursiveRecoveryLimits {
        maximum_depth: 4,
        maximum_work: 32,
        maximum_candidates: 8,
        maximum_gears: 12,
        maximum_remote_connections: 8,
        maximum_item_latency_us: 50_000,
    }
}

fn candidate<'a>(
    direct: &'a conduit_core::Plan,
    recursive: &'a conduit_core::Plan,
) -> RecursiveRecoveryCandidate<'a> {
    RecursiveRecoveryCandidate {
        lost_direct_plan: direct,
        replacement_plan: recursive,
        required_semantic_profile: "test/provider-generate@full-v1",
        offered_semantic_profile: "test/provider-generate@full-v1",
        offered_profile_is_reviewed_degradation: false,
        direct_implementation_unavailable: true,
        all_host_reservations_admitted: true,
        all_required_authority_admitted: true,
        expansion_depth: 1,
        search_work: 7,
        candidates_considered: 2,
        estimated_item_latency_us: 20_000,
    }
}

#[test]
fn lost_direct_capability_recovers_as_bounded_full_profile_cross_host_back() {
    let (direct_form, direct) = direct_plan();
    let (recursive_form, recursive) = plan_with_http_part(host("b", &[HTTP, DECODE]));
    let snapshot = direct.clone();
    assert!(direct.realization_backs.is_empty());
    assert_eq!(direct.fragments.len(), 1);
    assert!(default_expanded_placements(
        &direct_form,
        &[host("a", &[SOURCE, REQUEST, ENCODE, RESULT, SINK])]
    )
    .is_err());
    assert_eq!(direct.source_document_id, recursive.source_document_id);
    assert_eq!(direct.checked_form_id, recursive.checked_form_id);
    assert_ne!(direct.expanded_form_id, recursive.expanded_form_id);

    let evidence =
        conduit_planner::prove_recursive_recovery(&candidate(&direct, &recursive), limits())
            .unwrap();
    assert_eq!(evidence.semantic_profile, "test/provider-generate@full-v1");
    assert_eq!((evidence.host_count, evidence.expanded_gear_count), (2, 7));
    assert_eq!(evidence.remote_connection_count, 4);
    assert_eq!(
        (
            evidence.resource_binding_count,
            evidence.authority_binding_count
        ),
        (0, 0)
    );
    assert_eq!(
        recursive.realization_backs,
        recursive_form.realization_backs
    );
    assert_eq!(direct, snapshot, "the lost Plan remains immutable");
}

#[test]
fn full_recovery_refuses_degradation_mismatch_latency_bounds_and_admission() {
    let (_, direct) = direct_plan();
    let (_, recursive) = plan_with_http_part(host("b", &[HTTP, DECODE]));
    let bounds = limits();
    let mut cases = Vec::new();

    let mut item = candidate(&direct, &recursive);
    item.offered_semantic_profile = "test/provider-generate@degraded-v1";
    item.offered_profile_is_reviewed_degradation = true;
    cases.push((
        item,
        RecursiveRecoveryRefusal::RequiresDegradedProfileAdmission,
    ));
    let mut item = candidate(&direct, &recursive);
    item.offered_semantic_profile = "test/unrelated@1";
    cases.push((item, RecursiveRecoveryRefusal::SemanticProfileMismatch));
    let mut item = candidate(&direct, &recursive);
    item.estimated_item_latency_us = bounds.maximum_item_latency_us + 1;
    cases.push((
        item,
        RecursiveRecoveryRefusal::LatencyRequirementUnsatisfied,
    ));
    let mut item = candidate(&direct, &recursive);
    item.search_work = bounds.maximum_work + 1;
    cases.push((item, RecursiveRecoveryRefusal::SearchBoundExceeded));
    let mut item = candidate(&direct, &recursive);
    item.all_host_reservations_admitted = false;
    cases.push((item, RecursiveRecoveryRefusal::HostReservationRefused));
    let mut item = candidate(&direct, &recursive);
    item.all_required_authority_admitted = false;
    cases.push((item, RecursiveRecoveryRefusal::AuthorityUnavailable));

    for (item, refusal) in cases {
        assert_eq!(
            conduit_planner::prove_recursive_recovery(&item, bounds),
            Err(refusal)
        );
    }
}
