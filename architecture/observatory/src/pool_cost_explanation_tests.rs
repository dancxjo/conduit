//! Historical DTO fixtures use the ordinary Planner-produced Plan shape.
use crate::*;
use alloc::vec;
use conduit_core::*;

fn exercise(inspect: impl FnOnce(Plan, PoolCostSelectionAccount)) {
    super::cost_plan_explanation_tests::exercise(|planned| {
        let mut fragment = planned.plan.fragments[0].clone();
        let gear = &mut fragment.placements[0];
        let pool_id: SharedPoolId = "report/pool".into();
        gear.pool_references.push(pool_id.clone());
        let member = PoolRealizationEnvelope {
            host_id: gear.host_id.clone(),
            boot_id: gear.boot_id.clone(),
            offer_generation: gear.offer_generation,
            capability_id: gear.capability_id.clone(),
            implementation_id: gear.implementation_id.clone(),
            artifact_id: gear.artifact_id.clone(),
            member_capacity: 1,
            resources: vec![],
            admitted_lines: vec![],
        };
        let pool = PlannedSharedPool {
            pool_id: pool_id.clone(),
            declaration_id: "report/declaration".into(),
            member_front: CheckedFront::new(vec![], vec![], vec![], None),
            maximum_members: 1,
            member_limits: PoolMemberLimits {
                queue_item_capacity: 1,
                queue_byte_capacity: 1024,
                sign_item_capacity: 16,
                sign_byte_capacity: 2048,
            },
            member_sessions_required: false,
            realization_envelope: vec![member.clone()],
            selection_policy: SharedPoolSelectionPolicy::MinimumPredictedCompletionThenCapacity,
            admission_authority: "report/authority".into(),
            consumers: vec![gear.placement_id.clone()],
        };
        fragment.shared_pools.push(pool);
        let plan = seal_plan(
            PlotIdentity {
                source_document_id: fragment.source_document_id.clone(),
                checked_plot_id: fragment.checked_plot_id.clone(),
                expanded_plot_id: fragment.expanded_plot_id.clone(),
            },
            vec![fragment],
        );
        assert!(verify_plan(&plan));
        let clock = MonotonicClockIdentity::new(
            member.host_id.clone(),
            member.boot_id.clone(),
            "report/clock".into(),
            TemporalScale::Milliseconds,
            1,
            0,
        )
        .unwrap();
        let instant = |ticks| MonotonicInstant::new(ticks, clock.clone()).unwrap();
        let calibration = PoolCostCalibrationRevision::new("report/calibration".into(), 1).unwrap();
        let mut work = ExactNumericWork::new(1_000_000_000);
        let time = |coordinate| {
            RationalQuantity::new(
                coordinate,
                Unit::resolve("s").unwrap(),
                QuantityRole::Linear,
            )
            .unwrap()
        };
        let duration = PreparedPoolDuration::prepare(
            time(ExactRational::ratio(1, 3, &mut work).unwrap()),
            time(ExactRational::ratio(1, 1_000_000_000, &mut work).unwrap()),
            &mut work,
        )
        .unwrap();
        let source_sign: SignId = "report/source".into();
        let account = PoolCostSelectionAccount {
            evidence: PoolSelectionEvidence {
                plan_id: plan.plan_id.clone(),
                pool_id,
                operation_id: "report/operation".into(),
                selected_realization: Some(0),
                observation_sign_ids: vec!["report/hard".into(), source_sign.clone()],
                disposition: PoolSelectionDisposition::Selected,
                sign_id: "report/selection".into(),
            },
            demand: [9; 32],
            choice: Some(PoolCostChoiceBasis::MinimumPredictedCompletion),
            candidates: vec![PoolCostCandidateAccount {
                realization: 0,
                admission: PoolCostHardDisposition::Eligible,
                cost: PoolCostDisposition::Known {
                    upper_nanoseconds: 333_333_334,
                    source: 0,
                },
            }],
            sources: vec![PoolCostSourceReceipt {
                realization: 0,
                host: member.host_id,
                boot: member.boot_id,
                generation: member.offer_generation,
                host_profile: "report/profile".into(),
                sign: source_sign,
                supporting_signs: vec![],
                capability: member.capability_id,
                kind: "flow/pulse".into(),
                kind_contract: "flow/pulse@1".into(),
                implementation: member.implementation_id,
                artifact: member.artifact_id,
                execution_profile: "report/execution".into(),
                resource: None,
                observed: instant(10),
                expires: instant(20),
                evaluated_at: Some(instant(15)),
                duration,
                calibration: calibration.clone(),
                evaluated_calibration: Some(calibration),
                expected_execution: None,
                current_execution: None,
                availability: PoolCostSourceAvailability::Known,
            }],
        };
        account.validate(&plan).unwrap();
        inspect(plan, account);
    });
}

#[test]
fn historical_report_keeps_fraction_projection_fidelity_and_source_windows() {
    exercise(|plan, account| {
        let text = render_pool_cost_selection(&plan, &account).unwrap();
        for expected in [
            "original_time=1/3",
            "upper_nanoseconds=333333334",
            "Precision",
            "cached_calibration",
            "report/source",
            "report/clock",
            "ticks: 20",
            "request content omitted",
        ] {
            assert!(text.contains(expected), "missing {expected}: {text}");
        }
        let mut account = account;
        account.sources[0].availability = PoolCostSourceAvailability::Unavailable(
            PoolCostUnavailable::Prepared(PoolCostPreparationRefusal::Stale),
        );
        account.sources[0].evaluated_at =
            Some(MonotonicInstant::new(20, account.sources[0].observed.clock().clone()).unwrap());
        account.candidates[0].cost = PoolCostDisposition::Unavailable(
            PoolCostUnavailable::Prepared(PoolCostPreparationRefusal::Stale),
        );
        account.choice = Some(PoolCostChoiceBasis::CapacityReference);
        let text = render_pool_cost_selection(&plan, &account).unwrap();
        assert!(
            text.contains("CapacityReference") && text.contains("Unavailable(Prepared(Stale))")
        );
    });
}

#[test]
fn report_refuses_forged_numeric_source_and_decision_correlation() {
    exercise(|plan, account| {
        let mut forged = account.clone();
        forged.sources[0].boot = "foreign/boot".into();
        assert_eq!(
            render_pool_cost_selection(&plan, &forged),
            Err(CostExplanationRefusal::InconsistentSelection)
        );
        let mut forged = account.clone();
        forged.candidates[0].cost = PoolCostDisposition::Known {
            upper_nanoseconds: 0,
            source: 0,
        };
        assert_eq!(
            render_pool_cost_selection(&plan, &forged),
            Err(CostExplanationRefusal::InconsistentSelection)
        );
        let mut forged = account;
        forged.evidence.plan_id = "foreign/plan".into();
        assert_eq!(
            render_pool_cost_selection(&plan, &forged),
            Err(CostExplanationRefusal::InconsistentSelection)
        );
    });
}

#[test]
fn report_preserves_extent_refusal_and_cannot_present_expired_cost_as_known() {
    exercise(|plan, account| {
        let mut oversized = account.clone();
        oversized.evidence.observation_sign_ids = vec!["report/hard".into(); 449];
        assert_eq!(
            render_pool_cost_selection(&plan, &oversized),
            Err(CostExplanationRefusal::Extent)
        );
        let mut expired = account;
        expired.sources[0].evaluated_at = Some(expired.sources[0].expires.clone());
        assert_eq!(
            render_pool_cost_selection(&plan, &expired),
            Err(CostExplanationRefusal::InconsistentSelection)
        );
    });
}
