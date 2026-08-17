use conduit_core::{
    BootId, ClockCorrelation, HostId, MonotonicClockIdentity, MonotonicDuration, MonotonicInstant,
    MonotonicTimeRefusal, TemporalInstant, TemporalScale,
};

fn clock(boot: &str) -> MonotonicClockIdentity {
    MonotonicClockIdentity::new(
        HostId::from("host/a"),
        BootId::from(boot),
        "steady-clock/1".into(),
        TemporalScale::Milliseconds,
        1,
        2,
    )
    .unwrap()
}

fn monotonic(ticks: u64, boot: &str) -> MonotonicInstant {
    MonotonicInstant::new(ticks, clock(boot)).unwrap()
}

fn wall(ticks: u64) -> TemporalInstant {
    TemporalInstant {
        ticks,
        scale: TemporalScale::Milliseconds,
        clock_basis: "unix-epoch".into(),
        resolution_ticks: 1,
        uncertainty_ticks: 5,
    }
}

#[test]
fn wall_clock_steps_do_not_change_monotonic_elapsed_time() {
    let first = monotonic(100, "boot/1");
    let second = monotonic(125, "boot/1");
    let forward =
        ClockCorrelation::new("sample/forward".into(), first.clone(), wall(5_000), 7).unwrap();
    let backward =
        ClockCorrelation::new("sample/backward".into(), second.clone(), wall(1_000), 9).unwrap();

    assert_eq!(
        second.elapsed_since(&first),
        Ok(MonotonicDuration::new(25, TemporalScale::Milliseconds))
    );
    assert!(backward.wall().ticks < forward.wall().ticks);
    assert_eq!(forward.wall_uncertainty_ticks(), 7);
    assert_eq!(backward.monotonic().clock().uncertainty_ticks(), 2);
}

#[test]
fn reboot_makes_monotonic_instants_incomparable() {
    assert_eq!(
        monotonic(1, "boot/2").elapsed_since(&monotonic(500, "boot/1")),
        Err(MonotonicTimeRefusal::DifferentClock)
    );
}

#[test]
fn regression_scale_mismatch_and_overflow_refuse() {
    assert_eq!(
        monotonic(9, "boot/1").elapsed_since(&monotonic(10, "boot/1")),
        Err(MonotonicTimeRefusal::Regressed)
    );
    assert_eq!(
        monotonic(1, "boot/1").deadline_after(MonotonicDuration::new(1, TemporalScale::Seconds)),
        Err(MonotonicTimeRefusal::DifferentScale)
    );
    assert_eq!(
        monotonic(u64::MAX, "boot/1")
            .deadline_after(MonotonicDuration::new(1, TemporalScale::Milliseconds)),
        Err(MonotonicTimeRefusal::Overflow)
    );
}

#[test]
fn deadline_is_boot_scoped_and_expires_without_wall_time() {
    let armed = monotonic(100, "boot/1")
        .deadline_after(MonotonicDuration::new(25, TemporalScale::Milliseconds))
        .unwrap();
    assert_eq!(
        armed.remaining_at(&monotonic(105, "boot/1")),
        Ok(Some(MonotonicDuration::new(
            20,
            TemporalScale::Milliseconds
        )))
    );
    assert_eq!(armed.remaining_at(&monotonic(125, "boot/1")), Ok(None));
    assert_eq!(
        armed.remaining_at(&monotonic(1, "boot/2")),
        Err(MonotonicTimeRefusal::DifferentClock)
    );
}

#[test]
fn invalid_clock_and_wall_correlation_refuse() {
    assert_eq!(
        MonotonicClockIdentity::new(
            HostId::from("host/a"),
            BootId::from("boot/1"),
            "steady-clock/1".into(),
            TemporalScale::Milliseconds,
            0,
            0,
        ),
        Err(MonotonicTimeRefusal::InvalidResolution)
    );
    let mut invalid_wall = wall(1);
    invalid_wall.clock_basis.clear();
    assert_eq!(
        ClockCorrelation::new(
            "sample/invalid".into(),
            monotonic(1, "boot/1"),
            invalid_wall,
            0,
        ),
        Err(MonotonicTimeRefusal::InvalidWallInstant)
    );
}
