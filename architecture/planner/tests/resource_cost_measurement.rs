//! Identified clock observations become typed measurement ranges, not forecasts.
use conduit_core::*;
use conduit_planner::resource_cost::*;

fn work() -> ExactNumericWork {
    ExactNumericWork::new(100_000_000)
}
fn instant(
    ticks: u64,
    scale: TemporalScale,
    resolution: u64,
    uncertainty: u64,
) -> MonotonicInstant {
    MonotonicInstant::new(
        ticks,
        MonotonicClockIdentity::new(
            "host/measurement".into(),
            "boot/measurement".into(),
            "measurement/basis".into(),
            scale,
            resolution,
            uncertainty,
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn all_clock_scales_preserve_endpoints_and_exact_time_enclosures() {
    for (scale, denominator) in [
        (TemporalScale::Seconds, 1),
        (TemporalScale::Milliseconds, 1_000),
        (TemporalScale::Microseconds, 1_000_000),
        (TemporalScale::Nanoseconds, 1_000_000_000),
    ] {
        let started = instant(50, scale, 1, 1);
        let completed = instant(150, scale, 1, 1);
        let measured = CostMeasuredDuration::measure(&started, &completed, &mut work()).unwrap();
        assert_eq!(measured.started(), &started);
        assert_eq!(measured.completed(), &completed);
        assert_eq!(measured.elapsed().ticks(), 100);
        assert_eq!(measured.elapsed().scale(), scale);
        for (duration, ticks) in [
            (measured.range().lower(), 96),
            (measured.range().upper(), 104),
        ] {
            assert_eq!(duration.original().unit(), Unit::resolve("s").unwrap());
            assert_eq!(duration.original().role(), QuantityRole::Linear);
            assert_eq!(
                duration.seconds(),
                ExactRational::ratio(ticks.into(), denominator, &mut work()).unwrap()
            );
        }
    }
}

#[test]
fn zero_reading_and_short_reading_keep_uncertainty_without_negative_time() {
    let started = instant(10, TemporalScale::Microseconds, 2, 3);
    for (ticks, upper) in [(10, 10), (14, 14)] {
        let completed = instant(ticks, TemporalScale::Microseconds, 2, 3);
        let measured = CostMeasuredDuration::measure(&started, &completed, &mut work()).unwrap();
        assert_eq!(measured.range().lower().seconds(), ExactRational::ZERO);
        assert_eq!(
            measured.range().upper().seconds(),
            ExactRational::ratio(upper.into(), 1_000_000, &mut work()).unwrap()
        );
    }
}

#[test]
fn mismatched_clock_quality_foreign_basis_and_backwards_time_refuse() {
    let started = instant(10, TemporalScale::Microseconds, 1, 1);
    let changed = instant(20, TemporalScale::Microseconds, 2, 1);
    assert_eq!(
        CostMeasuredDuration::measure(&started, &changed, &mut work()),
        Err(CostEvidenceRefusal::ClockQualityChanged)
    );
    let foreign = MonotonicInstant::new(
        20,
        MonotonicClockIdentity::new(
            "host/foreign".into(),
            "boot/foreign".into(),
            "measurement/basis".into(),
            TemporalScale::Microseconds,
            1,
            1,
        )
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(
        CostMeasuredDuration::measure(&started, &foreign, &mut work()),
        Err(CostEvidenceRefusal::InvalidClock(_))
    ));
    let backwards = instant(9, TemporalScale::Microseconds, 1, 1);
    assert!(matches!(
        CostMeasuredDuration::measure(&started, &backwards, &mut work()),
        Err(CostEvidenceRefusal::InvalidClock(_))
    ));
}

#[test]
fn overflow_and_exhausted_numeric_work_refuse_instead_of_narrowing() {
    let started = instant(0, TemporalScale::Microseconds, 1, 1);
    let completed = instant(u64::MAX, TemporalScale::Microseconds, 1, 1);
    assert_eq!(
        CostMeasuredDuration::measure(&started, &completed, &mut work()),
        Err(CostEvidenceRefusal::InvalidClock(
            MonotonicTimeRefusal::Overflow
        ))
    );
    let completed = instant(20, TemporalScale::Microseconds, 1, 1);
    assert!(matches!(
        CostMeasuredDuration::measure(&started, &completed, &mut ExactNumericWork::new(0)),
        Err(CostEvidenceRefusal::Numeric(ExactNumericRefusal::WorkLimit))
    ));
}
