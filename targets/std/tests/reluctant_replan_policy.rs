//! Fixed-clock policy conformance; declared forecasts are not measured speedups.
#[path = "resource_common/replan_policy.rs"]
mod support;
use conduit_planner::resource_cost::*;
use support::*;
#[test]
fn persistent_conservative_benefit_recommends_one_distinct_ordinary_plan() {
    with_facts(|case, facts| {
        let old = case.previous.clone();
        let new = case.candidate.clone();
        let mut gate = ReluctantReplanGate::default();
        let decision = evaluate(
            &mut gate,
            case,
            facts,
            &[1_000, 1_100, 1_200],
            &[100, 100, 100],
            1_201,
        )
        .unwrap();
        assert!(decision.recommends_replacement());
        assert_ne!(decision.previous_plan_id, decision.candidate_plan_id);
        assert_eq!(decision.observations.len(), 3);
        assert_eq!(decision.replacement_sources.len(), 5);
        assert!(decision.arithmetic_work_spent > 0);
        for account in &decision.observations {
            assert_eq!(account.conservative_net_seconds, duration(75).seconds());
        }
        let again = evaluate(
            &mut gate,
            case,
            facts,
            &[2_500, 2_600, 2_700],
            &[100, 100, 100],
            2_701,
        )
        .unwrap();
        assert_eq!(
            again.keep_current,
            Some(KeepCurrentPlanReason::AlreadyRecommended)
        );
        assert_eq!(case.previous, old);
        assert_eq!(case.candidate, new);
    });
}
#[test]
fn noisy_load_and_short_bursts_keep_current_with_decisive_sources() {
    with_facts(|case, facts| {
        let mut gate = ReluctantReplanGate::default();
        for (ticks, ns, now, reason) in [
            (
                vec![1_000, 1_100, 1_200],
                vec![100, 20, 100],
                1_201,
                KeepCurrentPlanReason::NetBenefitNotPersistent,
            ),
            (
                vec![1_300, 1_400],
                vec![100, 100],
                1_401,
                KeepCurrentPlanReason::InsufficientPersistence,
            ),
            (
                vec![2_000, 2_500, 2_600],
                vec![100, 100, 100],
                2_601,
                KeepCurrentPlanReason::ObservationGap,
            ),
        ] {
            let d = evaluate(&mut gate, case, facts, &ticks, &ns, now).unwrap();
            assert_eq!(d.keep_current, Some(reason));
            assert!(d
                .observations
                .iter()
                .all(|o| !o.previous.sign_id.as_str().is_empty()
                    && !o.candidate.sign_id.as_str().is_empty()));
        }
    });
}
#[test]
fn invalid_evidence_leaves_latches_unchanged() {
    with_facts(|case, facts| {
        let mut gate = ReluctantReplanGate::default();
        for ticks in [vec![1_000, 1_000, 1_200], vec![1_200, 1_100, 1_000]] {
            assert!(matches!(
                evaluate(&mut gate, case, facts, &ticks, &[100, 100, 100], 1_201),
                Err(ReluctantReplanRefusal::DuplicateSign
                    | ReluctantReplanRefusal::ObservationOrder)
            ));
        }
        assert!(evaluate(
            &mut gate,
            case,
            facts,
            &[1_000, 1_100, 1_200],
            &[100, 100, 100],
            20_000
        )
        .is_err());
        assert!(evaluate(
            &mut gate,
            case,
            facts,
            &[1_000, 1_100, 1_200],
            &[100, 100, 100],
            1_201
        )
        .unwrap()
        .recommends_replacement());
    });
}
#[test]
fn finite_policy_refuses_unbounded_or_zero_profiles() {
    for count in [0, 1, 17, usize::MAX] {
        assert_eq!(
            ReluctantReplanPolicy::admit(
                count,
                duration(1),
                duration(10),
                duration(2),
                duration(1),
                duration(1),
                1,
                &mut work()
            )
            .unwrap_err(),
            ReluctantReplanRefusal::InvalidPolicy
        );
    }
}

#[test]
fn accepted_successor_observes_cooldown_before_recommending_another_plan() {
    with_facts(|case, facts| {
        let mut gate = ReluctantReplanGate::default();
        assert!(evaluate(
            &mut gate,
            case,
            facts,
            &[1000, 1100, 1200],
            &[100, 100, 100],
            1201
        )
        .unwrap()
        .recommends_replacement());
        let reverse = Cases {
            host: case.host.clone(),
            previous: case.candidate.clone(),
            candidate: case.previous.clone(),
            expanded: case.expanded.clone(),
        };
        let (old, new) = facts.gears();
        let revision = facts.components()[0].source().calibration();
        let calibrations = [revision];
        let now = instant(case, 1501);
        let context = CostEvaluationContext {
            host: &case.host,
            calibrations: &calibrations,
            now: &now,
            execution_observations: &[],
        };
        let reverse_facts = ReplacementCostFacts::admit(
            facts.candidate(),
            new,
            facts.previous(),
            old,
            core::array::from_fn(|i| Some(facts.components()[i])),
            facts.residences(),
            &[context],
            &mut work(),
        )
        .unwrap();
        let cooldown = evaluate(
            &mut gate,
            &reverse,
            &reverse_facts,
            &[1300, 1400, 1500],
            &[100, 100, 100],
            1501,
        )
        .unwrap();
        assert_eq!(cooldown.keep_current, Some(KeepCurrentPlanReason::Cooldown));
        let after = evaluate(
            &mut gate,
            &reverse,
            &reverse_facts,
            &[2500, 2600, 2700],
            &[100, 100, 100],
            2701,
        )
        .unwrap();
        assert!(after.recommends_replacement());
    });
}

#[test]
fn capacity_and_numeric_refusals_do_not_consume_the_recommendation() {
    with_facts(|case, facts| {
        let mut gate = ReluctantReplanGate::default();
        let ticks: Vec<_> = (0..17).map(|i| 1000 + i * 10).collect();
        assert!(matches!(
            evaluate(&mut gate, case, facts, &ticks, &[100; 17], 1201),
            Err(ReluctantReplanRefusal::Capacity)
        ));
        assert!(evaluate_with_budget(
            &mut gate,
            case,
            facts,
            &[1000, 1100, 1200],
            &[100, 100, 100],
            1201,
            0
        )
        .is_err());
        assert!(evaluate(
            &mut gate,
            case,
            facts,
            &[1000, 1100, 1200],
            &[100, 100, 100],
            1201
        )
        .unwrap()
        .recommends_replacement());
        assert!(matches!(
            evaluate(
                &mut gate,
                case,
                facts,
                &[1000, 1100, 1200],
                &[100, 100, 100],
                1201
            ),
            Err(ReluctantReplanRefusal::ObservationOrder)
        ));
    });
}
