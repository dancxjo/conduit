//! Cost comparison is subordinate to the existing finite atomic pool admission.
use conduit_kernel::{shared_pool::*, NodeId};

fn fixture() -> (
    [LoweredPoolRealization; 2],
    [LoweredPoolResourceRequirement; 2],
    [LoweredPoolObservation; 2],
    [LoweredPoolCost; 2],
) {
    let envelope = core::array::from_fn(|index| LoweredPoolRealization {
        realization: index as u16,
        placement: MemberPlacement {
            node: NodeId(index as u16),
            realization: index as u16,
            play: 7,
        },
        boot: index as u16 + 10,
        offer_generation: 2,
        capability: index as u16,
        implementation: index as u16,
        artifact: index as u16,
        member_capacity: 1,
    });
    let requirements = envelope.map(|r| LoweredPoolResourceRequirement {
        realization: r.realization,
        resource: 0,
        units: 1,
    });
    let observations = envelope.map(|r| LoweredPoolObservation {
        realization: r.realization,
        resource: 0,
        boot: r.boot,
        offer_generation: r.offer_generation,
        capability: r.capability,
        implementation: r.implementation,
        artifact: r.artifact,
        health: LoweredObservationHealth::Ready,
        unreserved_units: if r.realization == 0 { 10 } else { 2 },
        utilized_units: 0,
        sign: r.realization + 40,
    });
    let costs = envelope.map(|r| LoweredPoolCost {
        realization: r,
        demand: [9; 32],
        upper_completion: LoweredPoolDuration::from_nanoseconds(if r.realization == 0 {
            1000
        } else {
            10
        }),
        source_clock: r.boot,
        evaluation_clock: r.boot,
        observed_ticks: 100,
        expires_ticks: 110,
        evaluation_ticks: 105,
        sign: r.realization + 50,
    });
    (envelope, requirements, observations, costs)
}

#[test]
fn completion_policy_beats_capacity_reference_then_preserves_atomic_occupation_and_pinning() {
    let (envelope, requirements, observations, mut costs) = fixture();
    let mut reference = FixedSharedPool::<2, 32>::new(PoolId(1), 2, 7, 2).unwrap();
    let ordinary = admit_selected_pool_member(
        &mut reference,
        MemberKey([1; 32]),
        7,
        &envelope,
        &requirements,
        &observations,
        PoolSelectionPolicy::MoreUnreservedThenLessUtilizedThenPlanOrder,
        &mut [0; 1],
    )
    .unwrap();
    assert_eq!(ordinary.member.placement.realization, 0);
    // Unrelated Host-local origins do not participate in relative cost order.
    costs[1].observed_ticks += 1_000_000_000;
    costs[1].expires_ticks += 1_000_000_000;
    costs[1].evaluation_ticks += 1_000_000_000;
    let mut pool = FixedSharedPool::<2, 32>::new(PoolId(2), 2, 7, 2).unwrap();
    let mut decisions = [PoolCostCandidateDecision::EMPTY; 2];
    let selection = PoolCostSelection {
        demand: [9; 32],
        costs: &costs,
    };
    let first = admit_cost_selected_pool_member(
        &mut pool,
        MemberKey([1; 32]),
        7,
        &envelope,
        &requirements,
        &observations,
        selection,
        &mut [0; 1],
        &mut decisions,
    )
    .unwrap();
    assert_eq!(first.member.placement.realization, 1);
    assert_eq!(
        decisions[1].cost,
        PoolCostAvailability::Known {
            upper_completion: LoweredPoolDuration::from_nanoseconds(10),
            sign: 51
        }
    );
    pool.trigger(first.member).unwrap();
    let second = admit_cost_selected_pool_member(
        &mut pool,
        MemberKey([2; 32]),
        7,
        &envelope,
        &requirements,
        &observations,
        selection,
        &mut [0; 1],
        &mut decisions,
    )
    .unwrap();
    assert_eq!(second.member.placement.realization, 0);
    assert_eq!(decisions[1].admission, PoolCostAdmission::Full);
    assert_eq!(pool.population_for_realization(1), 1);
    assert_eq!(pool.population(), 2);
    assert!(matches!(
        admit_cost_selected_pool_member(
            &mut pool,
            MemberKey([3; 32]),
            7,
            &envelope,
            &requirements,
            &observations,
            selection,
            &mut [0; 1],
            &mut decisions
        ),
        Err(PoolSelectionError::CapacityUnavailable {
            examined_realizations: 2
        })
    ));
    assert_eq!(pool.population(), 2);
    // A later request can reuse released capacity; the first member is pinned
    // until the original owner performs both release transitions.
    pool.request_release(first.member).unwrap();
    assert_eq!(pool.population(), 2);
    pool.complete_release(first.member).unwrap();
    let third = admit_cost_selected_pool_member(
        &mut pool,
        MemberKey([3; 32]),
        7,
        &envelope,
        &requirements,
        &observations,
        selection,
        &mut [0; 1],
        &mut decisions,
    )
    .unwrap();
    assert_eq!(third.member.placement.realization, 1);
    assert_ne!(third.member.epoch, first.member.epoch);
    assert_eq!(second.member.placement.realization, 0);
}

#[test]
fn mismatched_or_stale_costs_keep_distinct_refusals_and_use_capacity_fallback() {
    let (envelope, requirements, observations, costs) = fixture();
    let original = costs[1];
    let mut cases = Vec::new();
    for change in 0..7 {
        let mut cost = original;
        match change {
            0 => cost.realization.boot += 1,
            1 => cost.realization.offer_generation += 1,
            2 => cost.realization.capability += 1,
            3 => cost.realization.implementation += 1,
            4 => cost.realization.artifact += 1,
            5 => cost.realization.placement.play += 1,
            _ => cost.realization.member_capacity += 1,
        }
        cases.push((cost, PoolCostAvailability::ScopeChanged));
    }
    let mut c = original;
    c.demand[0] = 8;
    cases.push((c, PoolCostAvailability::DemandChanged));
    let mut c = original;
    c.evaluation_clock += 1;
    cases.push((c, PoolCostAvailability::ClockChanged));
    let mut c = original;
    c.expires_ticks = c.observed_ticks;
    cases.push((c, PoolCostAvailability::InvalidWindow));
    let mut c = original;
    c.evaluation_ticks = c.observed_ticks - 1;
    cases.push((c, PoolCostAvailability::Future));
    let mut c = original;
    c.evaluation_ticks = c.expires_ticks;
    cases.push((c, PoolCostAvailability::Stale));
    for (cost, expected) in cases {
        let mut pool = FixedSharedPool::<2, 16>::new(PoolId(2), 2, 7, 2).unwrap();
        let mut decisions = [PoolCostCandidateDecision::EMPTY; 2];
        let selected = admit_cost_selected_pool_member(
            &mut pool,
            MemberKey([1; 32]),
            7,
            &envelope,
            &requirements,
            &observations,
            PoolCostSelection {
                demand: [9; 32],
                costs: &[cost],
            },
            &mut [0; 1],
            &mut decisions,
        )
        .unwrap();
        assert_eq!(selected.member.placement.realization, 0);
        assert_eq!(decisions[0].cost, PoolCostAvailability::Missing);
        assert_eq!(decisions[1].cost, expected);
    }
    let mut observations = observations;
    observations[1].health = LoweredObservationHealth::Unavailable;
    let mut pool = FixedSharedPool::<2, 16>::new(PoolId(2), 2, 7, 2).unwrap();
    let mut decisions = [PoolCostCandidateDecision::EMPTY; 2];
    let selected = admit_cost_selected_pool_member(
        &mut pool,
        MemberKey([1; 32]),
        7,
        &envelope,
        &requirements,
        &observations,
        PoolCostSelection {
            demand: [9; 32],
            costs: &costs,
        },
        &mut [0; 1],
        &mut decisions,
    )
    .unwrap();
    assert_eq!(selected.member.placement.realization, 0);
    assert_eq!(decisions[1].admission, PoolCostAdmission::NotCurrent);
    assert_eq!(decisions[1].cost, PoolCostAvailability::Missing);
}

#[test]
fn duplicate_cost_bounds_authority_and_evidence_refuse_before_occupation() {
    let (envelope, requirements, observations, costs) = fixture();
    let mut pool = FixedSharedPool::<2, 16>::new(PoolId(2), 2, 7, 2).unwrap();
    let mut decisions = [PoolCostCandidateDecision::EMPTY; 2];
    for (supplied, expected) in [
        (vec![costs[0], costs[0]], PoolSelectionError::DuplicateCost),
        (
            vec![costs[0]; MAXIMUM_POOL_COST_REALIZATIONS + 1],
            PoolSelectionError::CostInputLimit,
        ),
    ] {
        assert_eq!(
            admit_cost_selected_pool_member(
                &mut pool,
                MemberKey([1; 32]),
                7,
                &envelope,
                &requirements,
                &observations,
                PoolCostSelection {
                    demand: [9; 32],
                    costs: &supplied
                },
                &mut [0; 1],
                &mut decisions
            ),
            Err(expected)
        );
        assert_eq!(pool.population(), 0);
    }
    let selection = PoolCostSelection {
        demand: [9; 32],
        costs: &costs,
    };
    assert_eq!(
        admit_cost_selected_pool_member(
            &mut pool,
            MemberKey([1; 32]),
            7,
            &envelope,
            &requirements,
            &observations,
            selection,
            &mut [],
            &mut decisions
        ),
        Err(PoolSelectionError::EvidenceTooSmall)
    );
    assert_eq!(pool.population(), 0);
    assert_eq!(
        admit_cost_selected_pool_member(
            &mut pool,
            MemberKey([1; 32]),
            7,
            &envelope,
            &requirements,
            &observations,
            selection,
            &mut [0; 1],
            &mut []
        ),
        Err(PoolSelectionError::EvidenceTooSmall)
    );
    assert_eq!(pool.population(), 0);
    assert_eq!(
        admit_cost_selected_pool_member(
            &mut pool,
            MemberKey([1; 32]),
            8,
            &envelope,
            &requirements,
            &observations,
            selection,
            &mut [0; 1],
            &mut decisions
        ),
        Err(PoolSelectionError::Admission(PoolError::AuthorityDenied))
    );
    assert_eq!(pool.population(), 0);
}
