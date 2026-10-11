//! Independent coordinate oracle and custody/freshness substitution checks.
use super::{fragment, member_offer, pool};
use conduit_core::*;

fn work() -> ExactNumericWork {
    ExactNumericWork::new(1_000_000_000)
}
fn quantity(n: i128, d: i128, unit: &str) -> RationalQuantity {
    RationalQuantity::new(
        ExactRational::ratio(n, d, &mut work()).unwrap(),
        Unit::resolve(unit).unwrap(),
        QuantityRole::Linear,
    )
    .unwrap()
}
fn fixture() -> (Plan, HostAdvertisement) {
    let plan = seal_plan(
        PlotIdentity {
            source_document_id: "cost/source".into(),
            checked_plot_id: "cost/checked".into(),
            expanded_plot_id: "cost/expanded".into(),
        },
        vec![fragment(pool())],
    );
    let r = &plan.fragments[0].shared_pools[0].realization_envelope[0];
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: r.host_id.clone(),
        boot_id: r.boot_id.clone(),
        offer_generation: r.offer_generation,
        profile: "cost/test-host@1".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![member_offer("chat/peer", "chat/peer@1")],
        planner_capabilities: vec![],
    };
    assert!(verify_plan(&plan));
    (plan, host)
}
fn instant(host: &HostAdvertisement, ticks: u64) -> MonotonicInstant {
    MonotonicInstant::new(
        ticks,
        MonotonicClockIdentity::new(
            host.host_id.clone(),
            host.boot_id.clone(),
            "pool-cost/test-clock".into(),
            TemporalScale::Milliseconds,
            1,
            0,
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn fractional_completion_keeps_original_time_typed_tolerance_and_outward_witness() {
    let source = quantity(1, 3, "s");
    let tolerance = quantity(1, 1_000_000_000, "s");
    let prepared = PreparedPoolDuration::prepare(source, tolerance, &mut work()).unwrap();
    assert_eq!(prepared.source(), source);
    assert_eq!(prepared.tolerance(), tolerance);
    assert_eq!(prepared.upper_nanoseconds(), 333_333_334);
    let witness = prepared.projection();
    assert_eq!(witness.source().unit(), Unit::resolve("ns").unwrap());
    assert_eq!(witness.source().role(), QuantityRole::Linear);
    assert_eq!(
        witness.source().coordinate(),
        ExactRational::ratio(1_000_000_000, 3, &mut work()).unwrap()
    );
    assert_eq!(
        witness.target().coordinate().lower().unsigned().unwrap(),
        333_333_333
    );
    assert_eq!(
        witness.error().upper_distance(),
        ExactRational::ratio(2, 3, &mut work()).unwrap()
    );
    assert!(witness
        .facts()
        .iter()
        .any(|f| matches!(f, projection::ProjectionFact::Lost { .. })));
    assert_eq!(
        PreparedPoolDuration::prepare(quantity(1, 1, "ms"), quantity(0, 1, "ms"), &mut work())
            .unwrap()
            .upper_nanoseconds(),
        1_000_000
    );
    assert_eq!(
        PreparedPoolDuration::prepare(quantity(0, 1, "s"), quantity(0, 1, "s"), &mut work())
            .unwrap()
            .upper_nanoseconds(),
        0
    );
}

#[test]
fn wrong_dimensions_negative_values_precision_and_work_limits_refuse() {
    use PoolDurationPreparationRefusal as R;
    assert_eq!(
        PreparedPoolDuration::prepare(quantity(1, 1, "m"), quantity(0, 1, "m"), &mut work()),
        Err(R::InvalidDuration)
    );
    assert_eq!(
        PreparedPoolDuration::prepare(quantity(-1, 1, "s"), quantity(0, 1, "s"), &mut work()),
        Err(R::InvalidDuration)
    );
    assert_eq!(
        PreparedPoolDuration::prepare(quantity(1, 1, "s"), quantity(-1, 1, "s"), &mut work()),
        Err(R::InvalidTolerance)
    );
    assert_eq!(
        PreparedPoolDuration::prepare(quantity(1, 1, "s"), quantity(1, 1, "ns"), &mut work()),
        Err(R::InvalidTolerance)
    );
    assert_eq!(
        PreparedPoolDuration::prepare(quantity(1, 3, "ns"), quantity(0, 1, "ns"), &mut work()),
        Err(R::Projection(QuantityIntegerEnclosureRefusal::Numeric(
            IntegerEnclosureRefusal::ToleranceExceeded
        )))
    );
    assert!(matches!(
        PreparedPoolDuration::prepare(
            quantity(i128::from(u64::MAX) + 1, 1, "ns"),
            quantity(0, 1, "ns"),
            &mut work()
        ),
        Err(R::Projection(_))
    ));
    assert_eq!(
        PreparedPoolDuration::prepare(
            quantity(1, 1, "ms"),
            quantity(0, 1, "ms"),
            &mut ExactNumericWork::new(0)
        ),
        Err(R::Quantity(RationalQuantityRefusal::Numeric(
            ExactNumericRefusal::WorkLimit
        )))
    );
}

#[test]
fn prepared_cost_binds_one_plan_operation_and_full_owner_snapshot() {
    let (plan, host) = fixture();
    let pool_id = &plan.fragments[0].shared_pools[0].pool_id;
    let operation = PoolOperationId::from("cost/operation-1");
    let sign = SignId::from("cost/forecast-1");
    let (observed, expires, now) = (instant(&host, 10), instant(&host, 20), instant(&host, 15));
    let prepared = PreparedPoolCost::prepare(
        PoolCostPreparation {
            plan: &plan,
            fragment_index: 0,
            pool: pool_id,
            operation: &operation,
            demand: [7; 32],
            realization: 0,
            host: &host,
            offer: &host.capabilities[0],
            resource: None,
            sign: &sign,
            observed: &observed,
            expires: &expires,
            evaluated_at: &now,
            duration: quantity(1, 3, "s"),
            tolerance: quantity(1, 1_000_000_000, "s"),
        },
        &mut work(),
    )
    .ok()
    .unwrap();
    assert_eq!(prepared.plan_id(), &plan.plan_id);
    assert_eq!(prepared.operation_id(), &operation);
    assert_eq!(prepared.sign(), &sign);
    assert_eq!(prepared.demand(), [7; 32]);
    assert_eq!(prepared.duration().upper_nanoseconds(), 333_333_334);
    assert_eq!(prepared.validate_at(&host, &now), Ok(()));
    assert_eq!(
        prepared.validate_at(&host, &instant(&host, 9)),
        Err(PoolCostPreparationRefusal::Future)
    );
    assert_eq!(
        prepared.validate_at(&host, &instant(&host, 20)),
        Err(PoolCostPreparationRefusal::Stale)
    );
    for change in 0..5 {
        let mut changed = host.clone();
        match change {
            0 => changed.boot_id = "other/boot".into(),
            1 => changed.offer_generation.0 += 1,
            2 => changed.profile = "other/profile".into(),
            3 => changed.capabilities[0].limits.max_queue_items += 1,
            _ => changed.protocol_version += 1,
        }
        assert_eq!(
            prepared.validate_at(&changed, &now),
            Err(PoolCostPreparationRefusal::SourceChanged)
        );
    }
    let changed_quality = MonotonicInstant::new(
        15,
        MonotonicClockIdentity::new(
            host.host_id.clone(),
            host.boot_id.clone(),
            "pool-cost/test-clock".into(),
            TemporalScale::Milliseconds,
            2,
            0,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        prepared.validate_at(&host, &changed_quality),
        Err(PoolCostPreparationRefusal::ClockChanged)
    );
}
