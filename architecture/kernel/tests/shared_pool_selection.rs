use conduit_kernel::{
    shared_pool::{
        admit_selected_pool_member, FixedSharedPool, LoweredObservationHealth,
        LoweredPoolObservation, LoweredPoolRealization, LoweredPoolResourceRequirement, MemberKey,
        MemberPlacement, PoolId, PoolSelectionError, PoolSelectionPolicy,
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
    }
}

fn requirement(candidate: LoweredPoolRealization, resource: u16) -> LoweredPoolResourceRequirement {
    LoweredPoolResourceRequirement {
        realization: candidate.realization,
        resource,
        units: 4,
    }
}

fn observation(
    candidate: LoweredPoolRealization,
    resource: u16,
    sign: u16,
) -> LoweredPoolObservation {
    LoweredPoolObservation {
        realization: candidate.realization,
        resource,
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

fn admit<const SLOTS: usize, const SIGNS: usize>(
    pool: &mut FixedSharedPool<SLOTS, SIGNS>,
    key: MemberKey,
    envelope: &[LoweredPoolRealization],
    requirements: &[LoweredPoolResourceRequirement],
    observations: &[LoweredPoolObservation],
    evidence: &mut [u16],
) -> Result<conduit_kernel::shared_pool::SelectedPoolMember, PoolSelectionError> {
    admit_selected_pool_member(
        pool,
        key,
        7,
        envelope,
        requirements,
        observations,
        PoolSelectionPolicy::MoreUnreservedThenLessUtilizedThenPlanOrder,
        evidence,
    )
}

#[test]
fn new_work_fills_two_sealed_realizations_without_replanning() {
    let envelope = [realization(0), realization(1)];
    let requirements = [requirement(envelope[0], 0), requirement(envelope[1], 0)];
    let observations = [
        observation(envelope[0], 0, 51),
        observation(envelope[1], 0, 52),
    ];
    let mut pool = FixedSharedPool::<2, 16>::new(PoolId(9), 2, 7, 2).unwrap();
    let mut evidence = [0; 1];

    let first = admit(
        &mut pool,
        key(1),
        &envelope,
        &requirements,
        &observations,
        &mut evidence,
    )
    .unwrap();
    pool.trigger(first.member).unwrap();
    assert_eq!(first.member.placement.realization, 0);
    assert_eq!(
        &evidence[..usize::from(first.observation_sign_count)],
        &[51]
    );

    let second = admit(
        &mut pool,
        key(2),
        &envelope,
        &requirements,
        &observations,
        &mut evidence,
    )
    .unwrap();
    pool.trigger(second.member).unwrap();
    assert_eq!(second.member.placement.realization, 1);
    assert_eq!(
        &evidence[..usize::from(second.observation_sign_count)],
        &[52]
    );
    assert_eq!(second.examined_realizations, 2);
}

#[test]
fn provider_loss_is_not_replay_but_later_work_uses_the_sealed_alternative() {
    let envelope = [realization(0), realization(1)];
    let requirements = [requirement(envelope[0], 0), requirement(envelope[1], 0)];
    let mut observations = [
        observation(envelope[0], 0, 61),
        observation(envelope[1], 0, 62),
    ];
    let mut pool = FixedSharedPool::<2, 16>::new(PoolId(9), 2, 7, 2).unwrap();
    let mut evidence = [0; 1];
    let in_flight = admit(
        &mut pool,
        key(1),
        &envelope,
        &requirements,
        &observations,
        &mut evidence,
    )
    .unwrap();
    pool.trigger(in_flight.member).unwrap();
    pool.fail_member(in_flight.member).unwrap();

    observations[0].health = LoweredObservationHealth::Unavailable;
    observations[0].sign = 63;
    let later = admit(
        &mut pool,
        key(2),
        &envelope,
        &requirements,
        &observations,
        &mut evidence,
    )
    .unwrap();
    assert_eq!(later.member.placement.realization, 1);
    assert_eq!(later.member.key, key(2));
    assert_eq!(pool.population_for_realization(0), 1);
    assert_eq!(pool.population_for_realization(1), 1);
}

#[test]
fn stale_and_unsealed_truth_cannot_escape_the_plan_envelope() {
    let envelope = [realization(0)];
    let requirements = [requirement(envelope[0], 0)];
    let mut stale = observation(envelope[0], 0, 71);
    stale.boot += 1;
    let unsealed_candidate = realization(1);
    let unsealed = observation(unsealed_candidate, 0, 72);
    let mut pool = FixedSharedPool::<1, 8>::new(PoolId(9), 1, 7, 1).unwrap();
    let mut evidence = [0; 1];

    assert_eq!(
        admit(
            &mut pool,
            key(1),
            &envelope,
            &requirements,
            &[stale, unsealed],
            &mut evidence
        ),
        Err(PoolSelectionError::NoCurrentRealization {
            examined_realizations: 1
        })
    );
    assert_eq!(pool.population(), 0);
}

#[test]
fn every_resource_must_be_current_and_each_observation_sign_is_retained() {
    let envelope = [realization(0), realization(1)];
    let requirements = [
        requirement(envelope[0], 0),
        requirement(envelope[0], 1),
        requirement(envelope[1], 0),
        requirement(envelope[1], 1),
    ];
    let a_compute = observation(envelope[0], 0, 81);
    let mut a_memory = observation(envelope[0], 1, 82);
    a_memory.health = LoweredObservationHealth::Unavailable;
    let mut b_compute = observation(envelope[1], 0, 83);
    b_compute.unreserved_units = 12;
    let b_memory = observation(envelope[1], 1, 84);
    let observations = [a_compute, a_memory, b_compute, b_memory];
    let mut pool = FixedSharedPool::<1, 8>::new(PoolId(9), 1, 7, 2).unwrap();
    let mut evidence = [0; 2];
    let selected = admit(
        &mut pool,
        key(1),
        &envelope,
        &requirements,
        &observations,
        &mut evidence,
    )
    .unwrap();
    assert_eq!(selected.member.placement.realization, 1);
    assert_eq!(selected.observation_sign_count, 2);
    assert_eq!(evidence, [83, 84]);

    pool.fail_preparation(selected.member).unwrap();
    assert_eq!(
        admit(
            &mut pool,
            key(2),
            &envelope,
            &requirements,
            &[a_compute, a_compute, a_memory, b_compute, b_memory],
            &mut evidence
        ),
        Err(PoolSelectionError::DuplicateObservation)
    );
}
