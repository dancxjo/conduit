//! Exact Host/Boot dependencies and loss of one independent provider.
use super::*;

#[test]
fn host_and_boot_replacement_invalidate_only_the_changed_real_plan() {
    for domain in [FactDomain::Host, FactDomain::Boot] {
        let mut facts = Facts::default();
        let mut planner = IncrementalPlanner::new(2).unwrap();
        let mut retained = BTreeMap::new();
        let first = compare(&mut planner, &facts, &mut retained).unwrap();
        let original = retained[&first.selected_result_identity].clone();
        let independent = ordinary(&facts, 0).unwrap();
        match domain {
            FactDomain::Host => facts.host_revision[1] = 1,
            FactDomain::Boot => facts.boot_revision[1] = 1,
            _ => unreachable!(),
        }
        let next = compare(&mut planner, &facts, &mut retained).unwrap();
        assert_eq!(next.selected_candidate_id, "1");
        assert_eq!(next.metrics.invalidated_candidates, 1);
        assert_eq!(next.metrics.reused_candidates, 1);
        assert_ne!(
            first.selected_result_identity,
            next.selected_result_identity
        );
        assert_eq!(retained[&first.selected_result_identity], original);
        assert_eq!(retained[independent.plan_id.as_str()], independent);
        let replacement = &retained[&next.selected_result_identity];
        assert_eq!(replacement.checked_plot_id, original.checked_plot_id);
        assert_eq!(replacement.expanded_plot_id, original.expanded_plot_id);
        assert_ne!(
            replacement.fragments[0].fragment_id,
            original.fragments[0].fragment_id
        );
        assert!(verify_plan(&original) && verify_plan(replacement));
        let selected_host = host(&facts, 1);
        assert_eq!(replacement.fragments[0].host_id, selected_host.host_id);
        assert_eq!(replacement.fragments[0].boot_id, selected_host.boot_id);
        for placement in &replacement.fragments[0].placements {
            assert_eq!(placement.host_id, selected_host.host_id);
            assert_eq!(placement.boot_id, selected_host.boot_id);
            assert!(!placement.resources.is_empty());
        }
        println!(
            "{}",
            serde_json::json!({
                "case": format!("{domain:?}-replacement"),
                "original": original, "replacement": replacement, "independent": independent,
                "reused_candidates": next.metrics.reused_candidates,
                "invalidated_candidates": next.metrics.invalidated_candidates,
            })
        );
        let stable = compare(&mut planner, &facts, &mut retained).unwrap();
        assert_eq!(stable.metrics.reused_candidates, 2);
        assert_eq!(
            stable.selected_result_identity,
            next.selected_result_identity
        );
    }
}

#[test]
fn partial_provider_loss_preserves_independent_plan_then_reboot_invalidates_returning_provider() {
    let mut facts = Facts::default();
    let mut planner = IncrementalPlanner::new(2).unwrap();
    let mut retained = BTreeMap::new();
    let first = compare(&mut planner, &facts, &mut retained).unwrap();
    let original = retained[&first.selected_result_identity].clone();
    let independent = ordinary(&facts, 0).unwrap();
    facts.present[1] = false;
    let surviving = compare(&mut planner, &facts, &mut retained).unwrap();
    assert_eq!(
        surviving.selected_result_identity,
        independent.plan_id.as_str()
    );
    assert_eq!(surviving.considered.len(), 1);
    assert_eq!(surviving.metrics.reused_candidates, 1);
    assert_eq!(surviving.metrics.evaluated_candidates, 0);
    assert_eq!(retained[&first.selected_result_identity], original);
    assert_eq!(retained[independent.plan_id.as_str()], independent);
    facts.boot_revision[1] = 1;
    facts.present[1] = true;
    let returning = compare(&mut planner, &facts, &mut retained).unwrap();
    assert_eq!(returning.metrics.invalidated_candidates, 1);
    assert_eq!(returning.metrics.reused_candidates, 1);
    assert_ne!(
        returning.selected_result_identity,
        first.selected_result_identity
    );
    assert!(verify_plan(&retained[&returning.selected_result_identity]));
    assert!(verify_plan(&original) && verify_plan(&independent));
    println!(
        "{}",
        serde_json::json!({
            "case": "provider-loss-and-returning-reboot", "original": original,
            "surviving": independent, "returning": retained[&returning.selected_result_identity],
            "loss_evaluations": surviving.metrics.evaluated_candidates,
            "return_invalidations": returning.metrics.invalidated_candidates,
        })
    );
}
