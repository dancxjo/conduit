use conduit_core::{
    BodyClockCorrelation, BodyClockRateEstimator, BodyTimeQuality, BodyTimeRefusal,
    BodyTimeRelation, BodyTimeRequirement, BodyTimeTolerance, BodyTimeTracker, BootId,
    ClockProvenance, HostId, MonotonicClockIdentity, MonotonicDuration, MonotonicInstant,
    TemporalScale,
};

fn sample(host: &str, boot: &str, ticks: u64) -> MonotonicInstant {
    sample_at_scale(host, boot, ticks, TemporalScale::Milliseconds)
}

fn sample_at_scale(host: &str, boot: &str, ticks: u64, scale: TemporalScale) -> MonotonicInstant {
    MonotonicInstant::new(
        ticks,
        MonotonicClockIdentity::new(
            HostId::from(host),
            BootId::from(boot),
            "steady".into(),
            scale,
            1,
            1,
        )
        .unwrap(),
    )
    .unwrap()
}

fn provenance(admission_reference: &str) -> ClockProvenance {
    ClockProvenance::External {
        provider_id: "provider/test".into(),
        admission_reference: admission_reference.into(),
        policy_id: "policy/test".into(),
    }
}

#[test]
fn different_local_scales_project_to_one_body_scale() {
    let microsecond_clock = BodyClockCorrelation::new(
        "body/one".into(),
        TemporalScale::Milliseconds,
        1,
        sample_at_scale(
            "host/micro",
            "boot/micro",
            8_000_000,
            TemporalScale::Microseconds,
        ),
        10_000,
        -100,
        500,
        2,
        10_000_000,
        provenance("admitted/exchange/micro"),
    )
    .unwrap();
    let peer = correlation("host/milli", "boot/milli", 8_000, 10_000);
    let micro = microsecond_clock
        .project(&sample_at_scale(
            "host/micro",
            "boot/micro",
            8_100_000,
            TemporalScale::Microseconds,
        ))
        .unwrap();
    let milli = peer
        .project(&sample("host/milli", "boot/milli", 8_100))
        .unwrap();
    assert_eq!(micro.scale, TemporalScale::Milliseconds);
    assert_eq!(micro.center_ticks, 10_099);
    assert_eq!(micro.correlation_age_ticks, 100_000);
    assert_eq!(milli.correlation_age_ticks, 100);
    assert_eq!(
        micro.physical_relation(&milli),
        Ok(BodyTimeRelation::Indeterminate)
    );
}

fn correlation(host: &str, boot: &str, local: u64, body: u64) -> BodyClockCorrelation {
    BodyClockCorrelation::new(
        "body/one".into(),
        TemporalScale::Milliseconds,
        1,
        sample(host, boot, local),
        body,
        100,
        500,
        2,
        10_000,
        provenance("admitted/exchange/1"),
    )
    .unwrap()
}

fn candidate(local: u64, body: u64, generation: u64) -> BodyClockCorrelation {
    BodyClockCorrelation::new(
        "body/one".into(),
        TemporalScale::Milliseconds,
        generation,
        sample("host/a", "boot/a", local),
        body,
        100,
        500,
        2,
        10_000,
        provenance("admitted/exchange/2"),
    )
    .unwrap()
}

fn measured_candidate(local: u64, body: u64, generation: u64) -> BodyClockCorrelation {
    BodyClockCorrelation::new(
        "body/one".into(),
        TemporalScale::Milliseconds,
        generation,
        sample("host/a", "boot/a", local),
        body,
        0,
        10_000,
        5,
        100_000,
        provenance("admitted/exchange/measured"),
    )
    .unwrap()
}

#[test]
fn repeated_bounded_samples_refine_rate_without_rewriting_old_generations() {
    let initial = measured_candidate(1_000, 10_000, 1);
    let mut estimator = BodyClockRateEstimator::new(initial.clone()).unwrap();
    let later = measured_candidate(101_000, 110_100, 2);
    let refined = estimator.refine(&later, 5_000).unwrap();
    assert!((800..=1_200).contains(&refined.rate_parts_per_million()));
    assert!(refined.rate_error_parts_per_million() < 500);
    assert_eq!(initial.rate_parts_per_million(), 0);
    estimator.record(refined).unwrap();
    for generation in 3..=6 {
        let offset = generation - 1;
        let next = measured_candidate(
            1_000 + offset * 100_000,
            10_000 + offset * 100_100,
            generation,
        );
        let refined = estimator.refine(&next, 5_000).unwrap();
        estimator.record(refined).unwrap();
    }
    assert_eq!(estimator.retained_samples(), 4);
    assert_eq!(initial.generation(), 1);
}

#[test]
fn rate_estimator_rejects_outlier_and_old_generation() {
    let initial = measured_candidate(1_000, 10_000, 1);
    let mut estimator = BodyClockRateEstimator::new(initial).unwrap();
    assert_eq!(
        estimator.refine(&measured_candidate(101_000, 210_000, 2), 5_000),
        Err(BodyTimeRefusal::ConflictingEvidence)
    );
    assert_eq!(
        estimator.refine(&measured_candidate(101_000, 110_100, 1), 5_000),
        Err(BodyTimeRefusal::OldGeneration)
    );
    assert_eq!(estimator.retained_samples(), 1);
    assert_eq!(estimator.rejections().len(), 2);
    assert_eq!(
        estimator.rejections()[0].reason,
        BodyTimeRefusal::ConflictingEvidence
    );
    assert_eq!(estimator.rejections()[0].generation, 2);
    assert_eq!(
        estimator.rejections()[1].reason,
        BodyTimeRefusal::OldGeneration
    );
    let encoded = serde_json::to_vec(estimator.rejections()).unwrap();
    let replayed: Vec<conduit_core::BodyClockRejection> = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(replayed, estimator.rejections());
    for _ in 0..3 {
        assert_eq!(
            estimator.refine(&measured_candidate(101_000, 110_100, 1), 5_000),
            Err(BodyTimeRefusal::OldGeneration)
        );
    }
    assert_eq!(estimator.rejections().len(), 4);
    assert!(estimator
        .rejections()
        .iter()
        .all(|rejection| rejection.reason == BodyTimeRefusal::OldGeneration));
}

#[test]
fn offset_clocks_share_one_bounded_basis_and_drift_widens_bounds() {
    let first = correlation("host/a", "boot/a", 100, 10_000);
    let second = correlation("host/b", "boot/b", 8_000, 10_000);
    let early = first.project(&sample("host/a", "boot/a", 200)).unwrap();
    let late = first.project(&sample("host/a", "boot/a", 10_100)).unwrap();
    let peer = second.project(&sample("host/b", "boot/b", 8_100)).unwrap();

    assert_eq!(early.center_ticks, 10_100);
    assert_eq!(
        early.physical_relation(&peer),
        Ok(BodyTimeRelation::Indeterminate)
    );
    assert!(late.latest_ticks - late.center_ticks > early.latest_ticks - early.center_ticks);
    assert_eq!(early.body_basis, peer.body_basis);
    assert_eq!(early.generation, 1);
    assert_eq!(early.local_sample.ticks(), 200);
    assert_eq!(late.center_ticks, 20_001);
}

#[test]
fn boot_and_horizon_are_enforced() {
    let correlation = correlation("host/a", "boot/a", 100, 10_000);
    assert_eq!(
        correlation.project(&sample("host/a", "boot/new", 200)),
        Err(BodyTimeRefusal::DifferentClock)
    );
    assert_eq!(
        correlation.project(&sample("host/a", "boot/a", 10_101)),
        Err(BodyTimeRefusal::Stale)
    );
    assert_eq!(
        correlation.project(&sample("host/a", "boot/a", 99)),
        Err(BodyTimeRefusal::Stale)
    );
}

#[test]
fn disjoint_intervals_order_physical_time_only() {
    let first = correlation("host/a", "boot/a", 100, 10_000)
        .project(&sample("host/a", "boot/a", 100))
        .unwrap();
    let second = correlation("host/b", "boot/b", 8_000, 11_000)
        .project(&sample("host/b", "boot/b", 8_000))
        .unwrap();
    assert_eq!(
        first.physical_relation(&second),
        Ok(BodyTimeRelation::Before)
    );
    assert_eq!(
        second.physical_relation(&first),
        Ok(BodyTimeRelation::After)
    );
}

#[test]
fn rejoin_accepts_consistent_new_generation_without_reversing_progress() {
    let mut tracker = BodyTimeTracker::new(
        correlation("host/a", "boot/a", 100, 10_000),
        &sample("host/a", "boot/a", 100),
    )
    .unwrap();
    let before = tracker
        .observe(&sample("host/a", "boot/a", 200))
        .unwrap()
        .clone();
    let after = tracker
        .reconcile(candidate(200, 10_102, 2), &sample("host/a", "boot/a", 200))
        .unwrap();
    assert!(after.center_ticks >= before.center_ticks);
    assert_eq!(after.generation, 2);
    assert_eq!(before.generation, 1);
    assert_eq!(before.local_sample.ticks(), 200);
}

#[test]
fn stale_replayed_regressing_and_conflicting_candidates_cannot_mutate_truth() {
    let mut tracker = BodyTimeTracker::new(
        correlation("host/a", "boot/a", 100, 10_000),
        &sample("host/a", "boot/a", 100),
    )
    .unwrap();
    let at = sample("host/a", "boot/a", 200);
    tracker.observe(&at).unwrap();
    let before = tracker.last().clone();
    assert_eq!(
        tracker.reconcile(candidate(200, 10_100, 1), &at),
        Err(BodyTimeRefusal::OldGeneration)
    );
    assert_eq!(
        tracker.reconcile(candidate(200, 10_099, 2), &at),
        Err(BodyTimeRefusal::Regressed)
    );
    assert_eq!(
        tracker.reconcile(candidate(200, 20_000, 2), &at),
        Err(BodyTimeRefusal::ConflictingEvidence)
    );
    assert_eq!(tracker.last(), &before);
    assert_eq!(tracker.correlation().generation(), 1);
}

#[test]
fn partition_expires_quality_then_rejoin_uses_widened_continuity_bounds() {
    let mut tracker = BodyTimeTracker::new(
        correlation("host/a", "boot/a", 100, 10_000),
        &sample("host/a", "boot/a", 100),
    )
    .unwrap();
    let at = sample("host/a", "boot/a", 10_201);
    assert_eq!(tracker.observe(&at), Err(BodyTimeRefusal::Stale));
    assert_eq!(tracker.last().local_sample.ticks(), 100);
    let resumed = tracker
        .reconcile(candidate(10_201, 20_102, 2), &at)
        .unwrap();
    assert_eq!(resumed.generation, 2);
    assert_eq!(resumed.local_sample.ticks(), 10_201);
    assert_eq!(resumed.center_ticks, 20_102);
}

#[test]
fn malformed_replayed_estimate_cannot_invent_physical_order() {
    let original = correlation("host/a", "boot/a", 100, 10_000)
        .project(&sample("host/a", "boot/a", 100))
        .unwrap();
    let mut impossible_age = original.clone();
    impossible_age.correlation_age_ticks = impossible_age.local_sample.ticks() + 1;
    assert_eq!(
        impossible_age.validate(),
        Err(BodyTimeRefusal::InvalidEstimate)
    );
    let mut forged = original.clone();
    forged.earliest_ticks = forged.latest_ticks + 1;
    assert_eq!(forged.validate(), Err(BodyTimeRefusal::InvalidEstimate));
    assert_eq!(
        forged.physical_relation(&original),
        Err(BodyTimeRefusal::InvalidEstimate)
    );
}

#[test]
fn cross_host_quality_admission_checks_the_whole_elapsed_horizon() {
    let correlation = correlation("host/a", "boot/a", 100, 10_000);
    let at = sample("host/a", "boot/a", 100);
    let ready = BodyTimeRequirement::new(
        "body/one".into(),
        BodyTimeTolerance::new(7, TemporalScale::Milliseconds),
        MonotonicDuration::new(1_000, TemporalScale::Milliseconds),
    )
    .unwrap();
    assert!(matches!(
        ready.assess(&correlation, &at),
        BodyTimeQuality::Ready { .. }
    ));
    assert!(matches!(
        ready.assess_with_execution_bounds(
            &correlation,
            &at,
            MonotonicDuration::new(0, TemporalScale::Milliseconds),
            MonotonicDuration::new(0, TemporalScale::Milliseconds),
        ),
        BodyTimeQuality::Ready { .. }
    ));
    assert_eq!(
        ready.assess_with_execution_bounds(
            &correlation,
            &at,
            MonotonicDuration::new(4, TemporalScale::Milliseconds),
            MonotonicDuration::new(4, TemporalScale::Milliseconds),
        ),
        BodyTimeQuality::Unsupported {
            reason: BodyTimeRefusal::InsufficientQuality
        }
    );
    assert_eq!(
        ready.assess_with_execution_bounds(
            &correlation,
            &at,
            MonotonicDuration::new(1, TemporalScale::Microseconds),
            MonotonicDuration::new(1, TemporalScale::Milliseconds),
        ),
        BodyTimeQuality::Unsupported {
            reason: BodyTimeRefusal::InvalidHorizon
        }
    );
    let quality = ready.assess(&correlation, &at);
    quality.validate().unwrap();
    let encoded = serde_json::to_vec(&quality).unwrap();
    let replayed: BodyTimeQuality = serde_json::from_slice(&encoded).unwrap();
    replayed.validate().unwrap();
    assert_eq!(replayed, quality);
    let mut forged = replayed;
    if let BodyTimeQuality::Ready { horizon, .. } = &mut forged {
        horizon.generation += 1;
    }
    assert_eq!(forged.validate(), Err(BodyTimeRefusal::InvalidEstimate));
    let degrading = BodyTimeRequirement::new(
        "body/one".into(),
        BodyTimeTolerance::new(7, TemporalScale::Milliseconds),
        MonotonicDuration::new(10_000, TemporalScale::Milliseconds),
    )
    .unwrap();
    assert!(matches!(
        degrading.assess(&correlation, &at),
        BodyTimeQuality::Degrading {
            reason: BodyTimeRefusal::InsufficientQuality,
            ..
        }
    ));
    let unsupported = BodyTimeRequirement::new(
        "body/one".into(),
        BodyTimeTolerance::new(4, TemporalScale::Milliseconds),
        MonotonicDuration::new(100, TemporalScale::Milliseconds),
    )
    .unwrap();
    assert_eq!(
        unsupported.assess(&correlation, &at),
        BodyTimeQuality::Unsupported {
            reason: BodyTimeRefusal::InsufficientQuality
        }
    );
}
