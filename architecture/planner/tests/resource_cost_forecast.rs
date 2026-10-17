//! Exact, unit-normalized fulfillment intervals; no hard-deadline claim.
use conduit_core::*;
use conduit_planner::resource_cost::*;

fn work() -> ExactNumericWork {
    ExactNumericWork::new(100_000_000)
}
fn duration(n: i128, d: i128, unit: &str) -> CostDuration {
    CostDuration::admit(
        RationalQuantity::new(
            ExactRational::ratio(n, d, &mut work()).unwrap(),
            Unit::resolve(unit).unwrap(),
            QuantityRole::Linear,
        )
        .unwrap(),
        &mut work(),
    )
    .unwrap()
}
fn range(lower: CostDuration, upper: CostDuration) -> CostDurationRange {
    CostDurationRange::admit(lower, upper, &mut work()).unwrap()
}
fn point(n: i128, d: i128, unit: &str) -> CostDurationRange {
    let value = duration(n, d, unit);
    range(value, value)
}
fn stages(setup: CostDurationRange, service: CostDurationRange) -> CostForecastStages {
    CostForecastStages {
        setup: Some(setup),
        queue: Some(point(0, 1, "s")),
        service: Some(service),
        transfer: Some(point(0, 1, "s")),
    }
}
#[test]
fn fractional_stage_bounds_sum_exactly_without_native_rounding() {
    let inputs = CostForecastStages {
        setup: Some(point(1, 3, "ms")),
        queue: Some(point(1, 7, "ms")),
        service: Some(range(duration(1, 11, "ms"), duration(1, 5, "ms"))),
        transfer: Some(point(1, 2, "s")),
    };
    let forecast = FulfillmentCostForecast::sequential(inputs, &mut work()).unwrap();
    // Independently reduced sums: 1/3000 + 1/7000 + 1/11000 + 1/2.
    assert_eq!(
        forecast.lower_seconds(),
        ExactRational::ratio(115631, 231000, &mut work()).unwrap()
    );
    assert_eq!(
        forecast.upper_seconds(),
        ExactRational::ratio(262855, 525000, &mut work()).unwrap()
    );
    assert_eq!(forecast.stages(), inputs);
}
#[test]
fn setup_can_reverse_service_preference_and_overlap_stays_unordered() {
    let cpu_short = FulfillmentCostForecast::sequential(
        stages(point(0, 1, "s"), point(10, 1, "ms")),
        &mut work(),
    )
    .unwrap();
    let gpu_short = FulfillmentCostForecast::sequential(
        stages(point(20, 1, "ms"), point(1, 1, "ms")),
        &mut work(),
    )
    .unwrap();
    assert!(cpu_short.strictly_precedes(gpu_short, &mut work()).unwrap());
    let cpu_long = FulfillmentCostForecast::sequential(
        stages(point(0, 1, "s"), point(1000, 1, "ms")),
        &mut work(),
    )
    .unwrap();
    let gpu_long = FulfillmentCostForecast::sequential(
        stages(point(20, 1, "ms"), point(100, 1, "ms")),
        &mut work(),
    )
    .unwrap();
    assert!(gpu_long.strictly_precedes(cpu_long, &mut work()).unwrap());
    let uncertain = FulfillmentCostForecast::sequential(
        stages(
            point(0, 1, "s"),
            range(duration(5, 1, "ms"), duration(15, 1, "ms")),
        ),
        &mut work(),
    )
    .unwrap();
    assert!(!uncertain.strictly_precedes(cpu_short, &mut work()).unwrap());
    assert!(!cpu_short.strictly_precedes(uncertain, &mut work()).unwrap());
}
#[test]
fn unknown_inverted_and_budget_exhausted_models_refuse() {
    assert_eq!(
        CostDurationRange::admit(duration(2, 1, "ms"), duration(1, 1, "ms"), &mut work()),
        Err(CostEvidenceRefusal::InvertedDurationRange)
    );
    let mut inputs = stages(point(1, 1, "ms"), point(1, 3, "ms"));
    inputs.queue = None;
    let mut no_work = ExactNumericWork::new(0);
    assert_eq!(
        FulfillmentCostForecast::sequential(inputs, &mut no_work),
        Err(CostEvidenceRefusal::MissingForecastStage)
    );
    assert_eq!(no_work.spent(), 0);
    inputs.queue = Some(point(0, 1, "s"));
    assert_eq!(
        FulfillmentCostForecast::sequential(inputs, &mut no_work),
        Err(CostEvidenceRefusal::Numeric(ExactNumericRefusal::WorkLimit))
    );
}
