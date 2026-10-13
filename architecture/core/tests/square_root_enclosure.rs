use conduit_core::{
    projection::*, ExactNumericRefusal, ExactNumericWork, ExactRational as Q,
    PreparedSquareRoot as Root, SquareRootDomain as Domain, SquareRootPolicy as Policy,
    SquareRootRefusal as R,
};
use core::cmp::Ordering;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(100_000_000)
}
fn ratio(n: i128, d: i128) -> Q {
    Q::ratio(n, d, &mut work()).unwrap()
}

#[test]
fn irrational_root_has_proved_containment_and_admitted_width() {
    let mut budget = work();
    let root = Root::prepare(Q::from_i128(2), ratio(1, 1000), 32, &mut budget).unwrap();
    let bounds = root.target();
    assert_eq!(root.refinements(), 11);
    assert_ne!(bounds.lower(), bounds.upper());
    assert_eq!(
        bounds
            .lower()
            .checked_multiply(bounds.lower(), &mut budget)
            .unwrap()
            .checked_compare(root.radicand(), &mut budget)
            .unwrap(),
        Ordering::Less
    );
    assert_eq!(
        bounds
            .upper()
            .checked_multiply(bounds.upper(), &mut budget)
            .unwrap()
            .checked_compare(root.radicand(), &mut budget)
            .unwrap(),
        Ordering::Greater
    );
    assert_eq!(
        root.width(),
        bounds
            .upper()
            .checked_subtract(bounds.lower(), &mut budget)
            .unwrap()
    );
    assert_ne!(
        root.width()
            .checked_compare(root.maximum_width(), &mut budget)
            .unwrap(),
        Ordering::Greater
    );
    assert!(budget.spent() > 0);
}
#[test]
fn exact_special_cases_and_subunit_roots() {
    for (radicand, expected) in [
        (Q::ZERO, Q::ZERO),
        (Q::ONE, Q::ONE),
        (Q::from_i128(4), Q::from_i128(2)),
        (ratio(1, 4), ratio(1, 2)),
    ] {
        let root = Root::prepare(radicand, Q::ZERO, 8, &mut work()).unwrap();
        assert_eq!(root.target().lower(), expected);
        assert_eq!(root.target().upper(), expected);
        assert_eq!(root.width(), Q::ZERO);
    }
    let tiny = Root::prepare(ratio(1, 100), ratio(1, 1000), 32, &mut work()).unwrap();
    assert_ne!(
        tiny.target()
            .lower()
            .checked_compare(ratio(1, 10), &mut work())
            .unwrap(),
        Ordering::Greater
    );
    assert_ne!(
        tiny.target()
            .upper()
            .checked_compare(ratio(1, 10), &mut work())
            .unwrap(),
        Ordering::Less
    );
}
#[test]
fn domain_precision_work_and_extent_refuse_explicitly() {
    assert_eq!(
        Root::prepare(Q::from_i128(-1), Q::ONE, 32, &mut work()),
        Err(R::NegativeRadicand)
    );
    assert_eq!(
        Root::prepare(Q::ONE, Q::from_i128(-1), 32, &mut work()),
        Err(R::NegativeTolerance)
    );
    assert_eq!(
        Root::prepare(Q::from_i128(2), Q::ZERO, 32, &mut work()),
        Err(R::RefinementLimit)
    );
    assert_eq!(
        Root::prepare(Q::from_i128(2), ratio(1, 1000), 0, &mut work()),
        Err(R::RefinementLimit)
    );
    assert_eq!(
        Root::prepare(Q::from_i128(2), Q::ZERO, 32, &mut ExactNumericWork::new(0)),
        Err(R::Numeric(ExactNumericRefusal::WorkLimit))
    );
    let enormous = Q::from_i128(10).checked_power(1000, &mut work()).unwrap();
    assert_eq!(
        Root::prepare(enormous, Q::ZERO, 1, &mut work()),
        Err(R::Numeric(ExactNumericRefusal::ExtentExceeded))
    );
}
fn text(s: &str) -> ProjectionText<'_> {
    ProjectionText::new(s).unwrap()
}
fn input<'a>(
    source: &'a Root,
    target: &'a conduit_core::SquareRootEnclosure,
    facts: &'a [ProjectionFact<'a, Domain>],
) -> ProjectionInput<'a, Domain> {
    ProjectionInput {
        basis: ProjectionBasis {
            report: text("root/report@1"),
            source: text("root/source@1"),
            target: Some(text("root/target@1")),
            projector: text("root/projector@1"),
            requested_route: text("root/route@1"),
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
fn generic_report_distinguishes_exactness_and_rejects_forged_evidence() {
    for (radicand, tolerance, exact) in [(4, Q::ZERO, true), (2, ratio(1, 100), false)] {
        let source = Root::prepare(Q::from_i128(radicand), tolerance, 32, &mut work()).unwrap();
        let target = source.target();
        let facts = source.facts();
        let policy = Policy {
            maximum_width: tolerance,
        };
        let report =
            ProjectionReport::new(&Domain, &policy, input(&source, &target, &facts)).unwrap();
        assert_eq!(report.require_exact().is_ok(), exact);
        let wrong = Root::prepare(Q::from_i128(9), Q::ONE, 32, &mut work())
            .unwrap()
            .target();
        assert!(ProjectionReport::new(&Domain, &policy, input(&source, &wrong, &facts)).is_err());
        assert!(
            ProjectionReport::new(&Domain, &policy, input(&source, &target, &facts[..1])).is_err()
        );
        let changed = Policy {
            maximum_width: Q::from_i128(2),
        };
        assert!(!changed.permits(&source, &target, &facts));
        if !exact {
            assert!(
                ProjectionReport::new(&Domain, &changed, input(&source, &target, &facts))
                    .unwrap()
                    .require_policy(changed.identity())
                    .is_err()
            );
        }
        let invented = [
            ProjectionFact::Preserved {
                obligation: text("exact-denotation"),
            },
            ProjectionFact::Transformed {
                obligation: text("numeric-representation"),
                law: text("float@1"),
            },
        ];
        assert!(
            ProjectionReport::new(&Domain, &policy, input(&source, &target, &invented)).is_err()
        );
    }
}

#[test]
fn rational_grid_has_outward_bounds_without_floating_point_oracle() {
    for numerator in 0..25 {
        for denominator in 1..10 {
            let mut budget = work();
            let value = Q::ratio(numerator, denominator, &mut budget).unwrap();
            let prepared = Root::prepare(value, ratio(1, 1000), 32, &mut budget).unwrap();
            let target = prepared.target();
            // Cross-check containment by multiplying in the independent small
            // i128 domain. All dyadic endpoint parts in this grid fit i128.
            fn parts(q: Q) -> (i128, i128) {
                let mut bytes = [0; conduit_core::EXACT_RATIONAL_MAX_ENCODED_BYTES];
                let length = q.encode(&mut bytes).unwrap();
                let n = usize::from(u16::from_le_bytes([bytes[2], bytes[3]]));
                let d = usize::from(u16::from_le_bytes([bytes[4], bytes[5]]));
                assert_eq!(length, 6 + n + d);
                assert!(n <= 16 && d <= 16 && bytes[1] == 0);
                let mut nb = [0; 16];
                let mut db = [0; 16];
                nb[..n].copy_from_slice(&bytes[6..6 + n]);
                db[..d].copy_from_slice(&bytes[6 + n..length]);
                (i128::from_le_bytes(nb), i128::from_le_bytes(db))
            }
            let (ln, ld) = parts(target.lower());
            let (un, ud) = parts(target.upper());
            assert!(ln * ln * denominator <= numerator * ld * ld);
            assert!(un * un * denominator >= numerator * ud * ud);
            assert!((un * ld - ln * ud) * 1000 <= ud * ld);
        }
    }
}
