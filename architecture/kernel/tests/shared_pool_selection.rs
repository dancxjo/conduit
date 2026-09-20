use conduit_kernel::{
    shared_pool::{
        admit_selected_pool_member, FixedSharedPool, LoweredObservationHealth,
        LoweredPoolObservation, LoweredPoolRealization, MemberKey, MemberPlacement, PoolId,
        PoolSelectionError, PoolSelectionPolicy,
    },
    NodeId,
};

fn key(value: u8) -> MemberKey {
    MemberKey([value; 32])
}

fn realization(index: u16) -> LoweredPoolRealization {
    LoweredPoolRealization {
        realization: index,
        placement: MemberPlacement {
            node: NodeId(index + 10),
            realization: index,
            play: 31,
        },
        boot: index + 100,
        offer_generation: u64::from(index) + 7,
        capability: index + 200,
        implementation: index + 300,
        artifact: index + 400,
        member_capacity: 1,
        required_units: 4,
    }
}

fn observation(candidate: LoweredPoolRealization, sign: u16) -> LoweredPoolObservation {
    LoweredPoolObservation {
        realization: candidate.realization,
        boot: candidate.boot,
        offer_generation: candidate.offer_generation,
        capability: candidate.capability,
        implementation: candidate.implementation,
        artifact: candidate.artifact,
        health: LoweredObservationHealth::Ready,
        unreserved_units: 8,
        utilized_units: 2,
        sign,
    }
}

#[test]
fn new_work_fills_two_sealed_realizations_without_replanning() {
    let envelope = [realization(0), realization(1)];
    let observations = [observation(envelope[0], 51), observation(envelope[1], 52)];
    let policy = PoolSelectionPolicy::MoreUnreservedThenLessUtilizedThenPlanOrder;
    let mut pool = FixedSharedPool::<2, 16>::new(PoolId(9), 2, 7, 2).unwrap();

    let first =
        admit_selected_pool_member(&mut pool, key(1), 7, &envelope, &observations, policy).unwrap();
    pool.trigger(first.member).unwrap();
    assert_eq!(first.member.placement.realization, 0);
    assert_eq!(first.observation_sign, 51);

    let second =
        admit_selected_pool_member(&mut pool, key(2), 7, &envelope, &observations, policy).unwrap();
    pool.trigger(second.member).unwrap();
    assert_eq!(second.member.placement.realization, 1);
    assert_eq!(second.observation_sign, 52);
    assert_eq!(second.examined_realizations, 2);
}

#[test]
fn provider_loss_is_not_replay_but_later_work_uses_the_sealed_alternative() {
    let envelope = [realization(0), realization(1)];
    let mut observations = [observation(envelope[0], 61), observation(envelope[1], 62)];
    let policy = PoolSelectionPolicy::MoreUnreservedThenLessUtilizedThenPlanOrder;
    let mut pool = FixedSharedPool::<2, 16>::new(PoolId(9), 2, 7, 2).unwrap();
    let in_flight =
        admit_selected_pool_member(&mut pool, key(1), 7, &envelope, &observations, policy).unwrap();
    pool.trigger(in_flight.member).unwrap();
    pool.fail_member(in_flight.member).unwrap();

    observations[0].health = LoweredObservationHealth::Unavailable;
    observations[0].sign = 63;
    let later =
        admit_selected_pool_member(&mut pool, key(2), 7, &envelope, &observations, policy).unwrap();
    assert_eq!(later.member.placement.realization, 1);
    assert_eq!(later.member.key, key(2));
    assert_eq!(pool.population_for_realization(0), 1);
    assert_eq!(pool.population_for_realization(1), 1);
}

#[test]
fn stale_and_unsealed_truth_cannot_escape_the_plan_envelope() {
    let envelope = [realization(0)];
    let mut stale = observation(envelope[0], 71);
    stale.boot += 1;
    let unsealed = observation(realization(1), 72);
    let mut pool = FixedSharedPool::<1, 8>::new(PoolId(9), 1, 7, 1).unwrap();

    assert_eq!(
        admit_selected_pool_member(
            &mut pool,
            key(1),
            7,
            &envelope,
            &[stale, unsealed],
            PoolSelectionPolicy::MoreUnreservedThenLessUtilizedThenPlanOrder,
        ),
        Err(PoolSelectionError::NoCurrentRealization {
            examined_realizations: 1,
        })
    );
    assert_eq!(pool.population(), 0);
}

#[test]
fn sealed_policy_prefers_capacity_then_utilization_and_rejects_ambiguous_truth() {
    let envelope = [realization(0), realization(1)];
    let mut a = observation(envelope[0], 81);
    let mut b = observation(envelope[1], 82);
    b.unreserved_units = 12;
    b.utilized_units = 9;
    let mut pool = FixedSharedPool::<1, 8>::new(PoolId(9), 1, 7, 2).unwrap();
    let selected = admit_selected_pool_member(
        &mut pool,
        key(1),
        7,
        &envelope,
        &[a, b],
        PoolSelectionPolicy::MoreUnreservedThenLessUtilizedThenPlanOrder,
    )
    .unwrap();
    assert_eq!(selected.member.placement.realization, 1);

    pool.fail_preparation(selected.member).unwrap();
    a.unreserved_units = 12;
    a.utilized_units = 1;
    assert_eq!(
        admit_selected_pool_member(
            &mut pool,
            key(2),
            7,
            &envelope,
            &[a, a, b],
            PoolSelectionPolicy::MoreUnreservedThenLessUtilizedThenPlanOrder,
        ),
        Err(PoolSelectionError::DuplicateObservation)
    );
}
