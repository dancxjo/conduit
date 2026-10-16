use conduit_core::*;
use core::cmp::Ordering;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(1_000_000_000)
}
fn q(n: i128, d: i128) -> ExactRational {
    ExactRational::ratio(n, d, &mut work()).unwrap()
}
fn requirements() -> InverseCircularRequirements {
    InverseCircularRequirements {
        branch: InverseCircularBranch::Principal,
        angle: ArctangentTurnsRequirements {
            maximum_width_turns: q(1, 100_000_000),
            maximum_series_width: q(1, 100_000_000_000),
            maximum_terms: 32,
        },
        maximum_root_width: q(1, 1_000_000_000_000),
        maximum_root_refinements: 48,
    }
}
#[test]
fn exact_endpoints_and_zero_respect_each_principal_branch() {
    for (function, input, expected) in [
        (InverseCircularFunction::Arcsine, q(-1, 1), q(-1, 4)),
        (InverseCircularFunction::Arcsine, q(0, 1), q(0, 1)),
        (InverseCircularFunction::Arcsine, q(1, 1), q(1, 4)),
        (InverseCircularFunction::Arccosine, q(-1, 1), q(1, 2)),
        (InverseCircularFunction::Arccosine, q(0, 1), q(1, 4)),
        (InverseCircularFunction::Arccosine, q(1, 1), q(0, 1)),
    ] {
        let mut req = requirements();
        req.angle.maximum_width_turns = ExactRational::ZERO;
        req.angle.maximum_series_width = ExactRational::ZERO;
        req.angle.maximum_terms = 0;
        req.maximum_root_refinements = 0;
        let p = PreparedInverseCircular::prepare(function, input, req, &mut work()).unwrap();
        assert_eq!(p.target().lower(), expected);
        assert_eq!(p.target().upper(), expected);
        assert!(p.root().is_none());
        assert!(p.arctangent_bounds().is_none());
    }
}
#[test]
fn irrational_root_uncertainty_is_retained_and_contains_known_half_input_angles() {
    for (function, input, expected) in [
        (InverseCircularFunction::Arcsine, q(1, 2), q(1, 12)),
        (InverseCircularFunction::Arcsine, q(-1, 2), q(-1, 12)),
        (InverseCircularFunction::Arccosine, q(1, 2), q(1, 6)),
        (InverseCircularFunction::Arccosine, q(-1, 2), q(1, 3)),
    ] {
        let p =
            PreparedInverseCircular::prepare(function, input, requirements(), &mut work()).unwrap();
        println!(
            "INVERSE_CIRCULAR function={function:?} negative={} limb_work={} root_refinements={}",
            input.is_negative(),
            p.limb_work(),
            p.root().unwrap().refinements()
        );
        assert_eq!(p.input(), input);
        assert_eq!(p.function(), function);
        assert_ne!(
            p.target()
                .lower()
                .checked_compare(expected, &mut work())
                .unwrap(),
            Ordering::Greater
        );
        assert_ne!(
            p.target()
                .upper()
                .checked_compare(expected, &mut work())
                .unwrap(),
            Ordering::Less
        );
        assert_eq!(p.root().unwrap().radicand(), q(3, 4));
        assert!(p.arctangent_bounds().is_some());
        assert_ne!(p.width_turns(), ExactRational::ZERO);
        assert_ne!(
            p.width_turns()
                .checked_compare(requirements().angle.maximum_width_turns, &mut work())
                .unwrap(),
            Ordering::Greater
        );
    }
}
#[test]
fn domain_and_inadequate_root_or_total_error_profiles_refuse() {
    use InverseCircularFunction::Arcsine as F;
    use InverseCircularRefusal as R;
    for input in [q(1001, 1000), q(-1001, 1000)] {
        assert_eq!(
            PreparedInverseCircular::prepare(F, input, requirements(), &mut work()),
            Err(R::OutsideUnitDomain)
        );
    }
    let mut req = requirements();
    req.maximum_root_refinements = 0;
    assert!(matches!(
        PreparedInverseCircular::prepare(F, q(1, 2), req, &mut work()),
        Err(R::Enclosure(MathematicalEnclosureRefusal::Root(
            SquareRootRefusal::RefinementLimit
        )))
    ));
    req = requirements();
    req.maximum_root_width = q(1, 2);
    assert_eq!(
        PreparedInverseCircular::prepare(F, q(999, 1000), req, &mut work()),
        Err(R::UnresolvedPositiveRoot)
    );
    req = requirements();
    req.maximum_root_width = q(1, 10);
    assert_eq!(
        PreparedInverseCircular::prepare(F, q(1, 2), req, &mut work()),
        Err(R::Enclosure(
            MathematicalEnclosureRefusal::ToleranceExceeded
        ))
    );
}
