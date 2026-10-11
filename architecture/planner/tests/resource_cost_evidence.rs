//! Physical and clock admission contracts; no predictor/selection claim.
use conduit_core::*;
use conduit_planner::resource_cost::*;
use core::cmp::Ordering;

fn work() -> ExactNumericWork {
    ExactNumericWork::new(100_000_000)
}
fn quantity(n: i128, d: i128, unit: &str, role: QuantityRole) -> RationalQuantity {
    RationalQuantity::new(
        ExactRational::ratio(n, d, &mut work()).unwrap(),
        Unit::resolve(unit).unwrap(),
        role,
    )
    .unwrap()
}
#[test]
fn rational_time_coordinates_are_exact_and_keep_original_physical_custody() {
    let original = quantity(1, 3, "ms", QuantityRole::Linear);
    let mut normalization = work();
    let value = CostDuration::admit(original, &mut normalization).unwrap();
    assert!(normalization.spent() > 0);
    assert_eq!(
        normalization.spent() + normalization.remaining(),
        100_000_000
    );
    eprintln!(
        "exact 1/3 ms normalization conservative limb work: {}",
        normalization.spent()
    );
    assert_eq!(value.original(), original);
    assert_eq!(
        value.seconds(),
        ExactRational::ratio(1, 3000, &mut work()).unwrap()
    );
    let equivalent =
        CostDuration::admit(quantity(1, 3000, "s", QuantityRole::Linear), &mut work()).unwrap();
    assert_ne!(value.original(), equivalent.original());
    assert_eq!(value.compare(equivalent, &mut work()), Ok(Ordering::Equal));
    assert!(matches!(
        CostDuration::admit(quantity(1, 1, "m", QuantityRole::Linear), &mut work()),
        Err(CostEvidenceRefusal::Physical(_))
    ));
    assert!(matches!(
        CostDuration::admit(quantity(20, 1, "°C", QuantityRole::Point), &mut work()),
        Err(CostEvidenceRefusal::Physical(_))
    ));
    assert_eq!(
        CostDuration::admit(quantity(-1, 1, "s", QuantityRole::Linear), &mut work()),
        Err(CostEvidenceRefusal::NegativeDuration)
    );
    assert_eq!(
        CostDuration::admit(original, &mut ExactNumericWork::new(0)),
        Err(CostEvidenceRefusal::Numeric(ExactNumericRefusal::WorkLimit))
    );
}
fn clock(boot: &str, scale: TemporalScale, resolution: u64) -> MonotonicClockIdentity {
    MonotonicClockIdentity::new(
        "host/cost".into(),
        boot.into(),
        "cost-clock".into(),
        scale,
        resolution,
        0,
    )
    .unwrap()
}
fn instant(ticks: u64, clock: &MonotonicClockIdentity) -> MonotonicInstant {
    MonotonicInstant::new(ticks, clock.clone()).unwrap()
}
#[test]
fn freshness_fences_clock_boot_scale_quality_and_the_entire_valid_window() {
    let basis = clock("boot/cost", TemporalScale::Milliseconds, 1);
    let window = CostObservationWindow::admit(instant(100, &basis), instant(110, &basis)).unwrap();
    for ticks in [100, 109] {
        assert_eq!(window.validate_at(&instant(ticks, &basis)), Ok(()));
    }
    assert_eq!(
        window.validate_at(&instant(99, &basis)),
        Err(CostEvidenceRefusal::FutureObservation)
    );
    for ticks in [110, 111] {
        assert_eq!(
            window.validate_at(&instant(ticks, &basis)),
            Err(CostEvidenceRefusal::Stale)
        );
    }
    assert_eq!(
        window.validate_at(&instant(
            105,
            &clock("boot/new", TemporalScale::Milliseconds, 1)
        )),
        Err(CostEvidenceRefusal::InvalidClock(
            MonotonicTimeRefusal::DifferentClock
        ))
    );
    assert_eq!(
        window.validate_at(&instant(
            105,
            &clock("boot/cost", TemporalScale::Microseconds, 1)
        )),
        Err(CostEvidenceRefusal::InvalidClock(
            MonotonicTimeRefusal::DifferentScale
        ))
    );
    let changed = clock("boot/cost", TemporalScale::Milliseconds, 2);
    assert_eq!(
        window.validate_at(&instant(105, &changed)),
        Err(CostEvidenceRefusal::ClockQualityChanged)
    );
    assert_eq!(
        CostObservationWindow::admit(instant(100, &basis), instant(110, &changed)),
        Err(CostEvidenceRefusal::ClockQualityChanged)
    );
    assert_eq!(
        CostObservationWindow::admit(instant(100, &basis), instant(100, &basis)),
        Err(CostEvidenceRefusal::InvalidWindow)
    );
    let mut forged = serde_json::to_value(instant(105, &basis)).unwrap();
    forged["clock"]["resolution_ticks"] = 0.into();
    let forged: MonotonicInstant = serde_json::from_value(forged).unwrap();
    assert_eq!(
        window.validate_at(&forged),
        Err(CostEvidenceRefusal::InvalidClock(
            MonotonicTimeRefusal::InvalidResolution
        ))
    );
}
