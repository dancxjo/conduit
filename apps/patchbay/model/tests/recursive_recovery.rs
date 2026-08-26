use conduit_core::{BootId, HostId};
use conduit_planner::RecursiveRecoveryEvidence;
use patchbay_model::{explain_recursive_recovery, patchbay_presenter_plans};

#[test]
fn patchbay_reveals_the_scarred_graph_instead_of_flattening_it_to_fallback() {
    let proof = patchbay_presenter_plans().unwrap();
    let mut replacement = proof.recursive.clone();
    let mut second = replacement.fragments[0].clone();
    second.host_id = HostId::from("host/recursive-b");
    second.boot_id = BootId::from("boot/recursive-b");
    replacement.fragments.push(second);
    let gear_count = replacement
        .fragments
        .iter()
        .map(|fragment| fragment.placements.len())
        .sum::<usize>();
    let line_count = replacement
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.connections)
        .filter(|connection| connection.selected_line.is_some())
        .count();
    let evidence = RecursiveRecoveryEvidence {
        semantic_profile: "presentation/patchbay@full-v1".into(),
        expanded_gear_count: gear_count.try_into().unwrap(),
        host_count: 2,
        remote_connection_count: line_count.try_into().unwrap(),
        resource_binding_count: 2,
        authority_binding_count: 0,
        expansion_depth: 4,
        search_work: 8,
        candidates_considered: 2,
    };

    let explanation = explain_recursive_recovery(&proof.direct, &replacement, &evidence).unwrap();
    assert_eq!(explanation.hosts.len(), 2);
    assert!(!explanation.realization_backs.is_empty());
    assert!(explanation.summary.contains("Semantic capability"));
    assert!(explanation
        .summary
        .contains("full-profile recursive realization"));
    assert!(explanation.summary.contains("not fallback=true"));
}
