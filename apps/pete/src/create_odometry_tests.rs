use super::*;

#[test]
fn straight_turn_curve_reverse_and_wrap_are_exact_and_deterministic() {
    let mut straight = CreateOdometryAccumulator::new();
    assert_eq!(
        straight.integrate(100, 0).unwrap().value.components(),
        (100, 0, 0)
    );
    assert_eq!(straight.current().unwrap().sample_generation, 1);

    let mut turn_then_forward = CreateOdometryAccumulator::new();
    assert_eq!(
        turn_then_forward
            .integrate(0, 90)
            .unwrap()
            .value
            .components(),
        (0, 0, conduit_core::HALF_PI_MICRORADIANS)
    );
    let north = turn_then_forward.integrate(100, 0).unwrap();
    assert_eq!(north.value.components(), (0, 100, 1_570_797));

    let mut curved = CreateOdometryAccumulator::new();
    assert_eq!(
        curved.integrate(100, 90).unwrap().value.components(),
        (71, 71, 1_570_797)
    );

    let mut reverse = CreateOdometryAccumulator::new();
    assert_eq!(
        reverse.integrate(-100, 0).unwrap().value.components(),
        (-100, 0, 0)
    );

    let mut wrapped = CreateOdometryAccumulator::new();
    for _ in 0..4 {
        wrapped.integrate(0, 90).unwrap();
    }
    assert_eq!(wrapped.current().unwrap().value.components(), (0, 0, 0));
}

#[test]
fn submillimeter_components_accumulate_instead_of_disappearing() {
    let mut accumulator = CreateOdometryAccumulator::new();
    accumulator.integrate(0, 45).unwrap();
    for _ in 0..10 {
        accumulator.integrate(1, 0).unwrap();
    }
    assert_eq!(
        accumulator.current().unwrap().value.components(),
        (7, 7, 785_398)
    );
}

#[test]
fn overflow_and_generation_exhaustion_are_transactional() {
    let mut accumulator = CreateOdometryAccumulator::new();
    for _ in 0..305 {
        accumulator.integrate(i16::MAX, 0).unwrap();
    }
    let before = accumulator;
    assert_eq!(
        accumulator.integrate(i16::MAX, 0),
        Err(CreateOdometryError::PositionOverflow)
    );
    assert_eq!(accumulator, before);

    let mut exhausted = CreateOdometryAccumulator::new();
    exhausted.sample_generation = u32::MAX;
    let before = exhausted;
    assert_eq!(
        exhausted.integrate(1, 0),
        Err(CreateOdometryError::SampleGenerationExhausted)
    );
    assert_eq!(exhausted, before);
}

#[test]
fn reset_is_exact_authorized_non_actuating_and_advances_frame() {
    let host = HostId::from("std/pete");
    let boot = BootId::from("std/pete-boot");
    let offer_generation = OfferGeneration(7);
    let implementation_id = "pete/create1-observe-odometry@1";
    let binding = CreateOdometryResetBinding {
        host_id: &host,
        boot_id: &boot,
        offer_generation,
        implementation_id,
    };
    let mut accumulator = CreateOdometryAccumulator::new();
    accumulator.integrate(250, 30).unwrap();
    let authority = CreateOdometryResetAuthority {
        grant_id: CREATE_ODOMETRY_RESET_AUTHORITY,
        host_id: &host,
        boot_id: &boot,
        offer_generation,
        implementation_id,
        valid_until_tick: 200,
    };
    let sign = accumulator
        .reset(
            CreateOdometryResetRequest {
                request_id: "reset/1",
                expected_frame_generation: 1,
            },
            Some(authority),
            binding,
            100,
        )
        .unwrap();
    assert_eq!(sign.prior_frame_generation, 1);
    assert_eq!(sign.current_frame_generation, 2);
    assert_eq!(accumulator.current().unwrap().value.components(), (0, 0, 0));
    assert_eq!(accumulator.current().unwrap().sample_generation, 0);
}

#[test]
fn reset_refuses_stale_wrong_or_expired_truth_without_mutation() {
    let host = HostId::from("std/pete");
    let other_host = HostId::from("std/other");
    let boot = BootId::from("std/pete-boot");
    let offer_generation = OfferGeneration(7);
    let implementation_id = "pete/create1-observe-odometry@1";
    let binding = CreateOdometryResetBinding {
        host_id: &host,
        boot_id: &boot,
        offer_generation,
        implementation_id,
    };
    let mut accumulator = CreateOdometryAccumulator::new();
    accumulator.integrate(20, 0).unwrap();
    let before = accumulator;
    for (request, authority, expected) in [
        (
            CreateOdometryResetRequest {
                request_id: "reset/stale",
                expected_frame_generation: 9,
            },
            Some(CreateOdometryResetAuthority {
                grant_id: CREATE_ODOMETRY_RESET_AUTHORITY,
                host_id: &host,
                boot_id: &boot,
                offer_generation,
                implementation_id,
                valid_until_tick: 200,
            }),
            CreateOdometryResetRefusal::StaleFrameGeneration,
        ),
        (
            CreateOdometryResetRequest {
                request_id: "reset/host",
                expected_frame_generation: 1,
            },
            Some(CreateOdometryResetAuthority {
                grant_id: CREATE_ODOMETRY_RESET_AUTHORITY,
                host_id: &other_host,
                boot_id: &boot,
                offer_generation,
                implementation_id,
                valid_until_tick: 200,
            }),
            CreateOdometryResetRefusal::HostMismatch,
        ),
        (
            CreateOdometryResetRequest {
                request_id: "reset/expired",
                expected_frame_generation: 1,
            },
            Some(CreateOdometryResetAuthority {
                grant_id: CREATE_ODOMETRY_RESET_AUTHORITY,
                host_id: &host,
                boot_id: &boot,
                offer_generation,
                implementation_id,
                valid_until_tick: 100,
            }),
            CreateOdometryResetRefusal::AuthorityExpired,
        ),
        (
            CreateOdometryResetRequest {
                request_id: "reset/stale-offer",
                expected_frame_generation: 1,
            },
            Some(CreateOdometryResetAuthority {
                grant_id: CREATE_ODOMETRY_RESET_AUTHORITY,
                host_id: &host,
                boot_id: &boot,
                offer_generation: OfferGeneration(8),
                implementation_id,
                valid_until_tick: 200,
            }),
            CreateOdometryResetRefusal::OfferGenerationMismatch,
        ),
        (
            CreateOdometryResetRequest {
                request_id: "reset/wrong-realization",
                expected_frame_generation: 1,
            },
            Some(CreateOdometryResetAuthority {
                grant_id: CREATE_ODOMETRY_RESET_AUTHORITY,
                host_id: &host,
                boot_id: &boot,
                offer_generation,
                implementation_id: "pete/other-odometry@1",
                valid_until_tick: 200,
            }),
            CreateOdometryResetRefusal::ImplementationMismatch,
        ),
    ] {
        assert_eq!(
            accumulator.reset(request, authority, binding, 100,),
            Err(expected)
        );
        assert_eq!(accumulator, before);
    }
}

#[test]
fn fixed_cordic_cardinals_are_bounded_and_repeatable() {
    let east = fixed_sin_cos(0);
    assert_eq!(east, fixed_sin_cos(0));
    assert!(east.0.abs() <= 8);
    assert!(east.1.abs_diff(1_000_000) <= 8);
    let north = fixed_sin_cos(conduit_core::HALF_PI_MICRORADIANS as i64);
    assert!(north.0.abs_diff(1_000_000) <= 8);
    assert!(north.1.abs() <= 8);
    let west = fixed_sin_cos(conduit_core::PI_MICRORADIANS as i64);
    assert!(west.0.abs() <= 8);
    assert!(west.1.abs_diff(-1_000_000) <= 8);
}
