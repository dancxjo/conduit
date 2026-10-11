//! Time enclosures keep physical custody and distinguish arithmetic error.
use conduit_core::{projection::*, *};
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
fn text(s: &str) -> ProjectionText<'_> {
    ProjectionText::new(s).unwrap()
}
fn input<'a>(
    source: &'a PreparedQuantityIntegerEnclosure,
    target: &'a QuantityIntegerEnclosure,
    facts: &'a [ProjectionFact<'a, QuantityIntegerEnclosureDomain>],
) -> ProjectionInput<'a, QuantityIntegerEnclosureDomain> {
    ProjectionInput {
        basis: ProjectionBasis {
            report: text("fixture/time/report"),
            source: text("fixture/time/source"),
            target: Some(text("fixture/time/bounds")),
            projector: text("numeric/quantity-integer-enclosure@1"),
            requested_route: text("fixture/time-enclosure"),
            boundary: None,
        },
        source,
        target: Some(target),
        facts,
        native: &[],
        scores: &[],
        admitted: &[],
        attempts: &[],
        selected_attempt: None,
        mechanism: ProjectionMechanism::Completed,
        diagnostics: &[],
    }
}
#[test]
fn fractional_time_has_admitted_outward_ticks_and_explicit_precision_loss() {
    let source = quantity(1, 3, "ms", QuantityRole::Linear);
    let tolerance = quantity(1, 1, "ms", QuantityRole::Linear);
    let prepared = PreparedQuantityIntegerEnclosure::prepare(
        source,
        PrimitiveInfoKind::U64,
        tolerance,
        &mut work(),
    )
    .unwrap();
    let target = prepared.target();
    assert_eq!(target.coordinate().lower().unsigned().unwrap(), 0);
    assert_eq!(target.coordinate().upper().unsigned().unwrap(), 1);
    assert_eq!(target.unit(), source.unit());
    assert_eq!(target.unit().dimension(), source.dimension());
    assert_eq!(target.unit().family(), source.family());
    assert_eq!(target.role(), source.role());
    assert_eq!(prepared.source(), source);
    let facts = prepared.facts();
    let policy = QuantityIntegerEnclosurePolicy {
        maximum_outward_error: tolerance,
    };
    let report = ProjectionReport::new(
        &QuantityIntegerEnclosureDomain,
        &policy,
        input(&prepared, &target, &facts),
    )
    .unwrap();
    assert_eq!(
        report.require_exact(),
        Err(ProjectionRefusal::ConsumerRequiresExact)
    );
    assert_eq!(report.require_policy(policy.identity()), Ok(&target));
    let foreign = PreparedQuantityIntegerEnclosure::prepare(
        quantity(1, 3, "m", QuantityRole::Linear),
        PrimitiveInfoKind::U64,
        quantity(1, 1, "m", QuantityRole::Linear),
        &mut work(),
    )
    .unwrap()
    .target();
    assert!(matches!(
        ProjectionReport::new(
            &QuantityIntegerEnclosureDomain,
            &policy,
            input(&prepared, &foreign, &facts)
        ),
        Err(ProjectionRefusal::Domain)
    ));
    assert!(matches!(
        ProjectionReport::new(
            &QuantityIntegerEnclosureDomain,
            &policy,
            input(&prepared, &target, &facts[..2])
        ),
        Err(ProjectionRefusal::Domain)
    ));
}
#[test]
fn tolerance_requires_the_exact_unit_and_difference_role() {
    let time = quantity(1, 3, "ms", QuantityRole::Linear);
    for tolerance in [
        quantity(1, 1, "s", QuantityRole::Linear),
        quantity(1, 1, "m", QuantityRole::Linear),
    ] {
        let mut budget = ExactNumericWork::new(0);
        assert_eq!(
            PreparedQuantityIntegerEnclosure::prepare(
                time,
                PrimitiveInfoKind::U64,
                tolerance,
                &mut budget
            ),
            Err(QuantityIntegerEnclosureRefusal::PhysicalToleranceMismatch)
        );
        assert_eq!(budget.spent(), 0);
    }
    let point = quantity(211, 10, "°C", QuantityRole::Point);
    let tolerance = quantity(1, 1, "°C", QuantityRole::Delta);
    let p = PreparedQuantityIntegerEnclosure::prepare(
        point,
        PrimitiveInfoKind::I16,
        tolerance,
        &mut work(),
    )
    .unwrap();
    assert_eq!(p.target().coordinate().lower().signed().unwrap(), 21);
    assert_eq!(p.target().coordinate().upper().signed().unwrap(), 22);
    assert_eq!(p.target().role(), QuantityRole::Point);
    assert_eq!(
        PreparedQuantityIntegerEnclosure::prepare(
            point,
            PrimitiveInfoKind::I16,
            quantity(1, 1, "°C", QuantityRole::Point),
            &mut work()
        ),
        Err(QuantityIntegerEnclosureRefusal::PhysicalToleranceMismatch)
    );
}
