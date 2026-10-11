//! Type-safe exact projection and general projection-report forgery refusal.
use conduit_core::{
    projection::*, ExactIntegerProjectionDomain as Domain, ExactIntegerProjectionPolicy as Policy,
    ExactIntegerProjectionRefusal as R, ExactIntegerProjectionRequest as Request,
    ExactNumericRefusal as N, ExactNumericWork, ExactRational, FixedInteger,
    PrimitiveInfoKind as K,
};
fn text(s: &str) -> ProjectionText<'_> {
    ProjectionText::new(s).unwrap()
}
fn request(value: i128, kind: K) -> Request {
    Request {
        value: ExactRational::from_i128(value),
        target_kind: kind,
    }
}
fn input<'a>(
    source: &'a Request,
    target: &'a FixedInteger,
    facts: &'a [ProjectionFact<'a, Domain>],
) -> ProjectionInput<'a, Domain> {
    ProjectionInput {
        basis: ProjectionBasis {
            report: text("numeric/report@1"),
            source: text("numeric/source@1"),
            target: Some(text("numeric/target@1")),
            projector: text("numeric/fixed-integer@1"),
            requested_route: text("exact@1"),
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
fn fixed_integer_projection_checks_range_signedness_and_integrality() {
    for kind in [K::I8, K::I16, K::I32, K::I64, K::I128] {
        let value = request(-12, kind).project().unwrap();
        assert_eq!(value.kind(), kind);
        assert_eq!(value.signed().unwrap(), -12);
    }
    for kind in [K::U8, K::U16, K::U32, K::U64, K::U128] {
        let value = request(12, kind).project().unwrap();
        assert_eq!(value.kind(), kind);
        assert_eq!(value.unsigned().unwrap(), 12);
        assert_eq!(request(-1, kind).project(), Err(R::Numeric(N::OutOfRange)));
    }
    assert!(matches!(request(128, K::I8).project(), Err(R::Target(_))));
    assert!(matches!(request(256, K::U8).project(), Err(R::Target(_))));
    let mut work = ExactNumericWork::new(1_000_000);
    let fractional = Request {
        value: ExactRational::ratio(1, 3, &mut work).unwrap(),
        target_kind: K::I64,
    };
    assert_eq!(fractional.project(), Err(R::Numeric(N::InexactProjection)));
    for kind in [K::F32, K::F64, K::Scalar, K::Count, K::Bool] {
        assert_eq!(request(1, kind).project(), Err(R::UnsupportedTarget));
    }
    let value =
        ExactRational::parse_decimal("340282366920938463463374607431768211455", &mut work).unwrap();
    let projected = Request {
        value,
        target_kind: K::U128,
    }
    .project()
    .unwrap();
    assert_eq!(projected.unsigned().unwrap(), u128::MAX);
    let enormous =
        ExactRational::parse_decimal(&format!("1{}1", "0".repeat(999)), &mut work).unwrap();
    assert_eq!(
        Request {
            value: enormous,
            target_kind: K::I64
        }
        .project(),
        Err(R::Numeric(N::OutOfRange))
    );
}
#[test]
fn generic_report_rechecks_exact_value_kind_and_complete_obligation_inventory() {
    let source = request(42, K::U16);
    let target = source.project().unwrap();
    let facts = Domain.facts();
    let report = ProjectionReport::new(&Domain, &Policy, input(&source, &target, &facts)).unwrap();
    assert_eq!(report.require_exact(), Ok(&target));
    assert_eq!(
        report.summary().disposition,
        ProjectionDisposition::Completed(ProjectionFidelity::Exact)
    );
    let wrong = request(41, K::U16).project().unwrap();
    let wrong_kind = request(42, K::U64).project().unwrap();
    for wrong in [wrong, wrong_kind] {
        assert!(matches!(
            ProjectionReport::new(&Domain, &Policy, input(&source, &wrong, &facts)),
            Err(ProjectionRefusal::Domain)
        ));
    }
    assert!(matches!(
        ProjectionReport::new(&Domain, &Policy, input(&source, &target, &facts[..1])),
        Err(ProjectionRefusal::Domain)
    ));
    let wrong_law = [
        ProjectionFact::Preserved {
            obligation: text("exact-denotation"),
        },
        ProjectionFact::Transformed {
            obligation: text("numeric-representation"),
            law: text("round@1"),
        },
    ];
    assert!(matches!(
        ProjectionReport::new(&Domain, &Policy, input(&source, &target, &wrong_law)),
        Err(ProjectionRefusal::Domain)
    ));
}
