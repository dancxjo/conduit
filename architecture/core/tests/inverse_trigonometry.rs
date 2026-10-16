use conduit_core::{
    ArctangentTurnsRequirements as Requirements, ExactNumericWork as Work, ExactRational as Q,
    MathematicalEnclosureRefusal as R, PreparedArctangentTurns as Prepared,
};
use core::cmp::Ordering;
fn work() -> Work {
    Work::new(100_000_000)
}
fn q(n: i128, d: i128) -> Q {
    Q::ratio(n, d, &mut work()).unwrap()
}
fn requirements() -> Requirements {
    Requirements {
        maximum_width_turns: q(1, 1_000_000_000),
        maximum_series_width: q(1, 100_000_000_000),
        maximum_terms: 32,
    }
}
#[test]
fn exact_cardinals_need_no_series_and_keep_principal_branch() {
    for (input, output) in [
        (Q::ZERO, Q::ZERO),
        (Q::ONE, q(1, 8)),
        (Q::from_i128(-1), q(-1, 8)),
    ] {
        let mut req = requirements();
        req.maximum_width_turns = Q::ZERO;
        req.maximum_series_width = Q::ZERO;
        req.maximum_terms = 0;
        let p = Prepared::prepare(input, req, &mut work()).unwrap();
        assert_eq!(p.input(), input);
        assert_eq!(p.target().lower(), output);
        assert_eq!(p.target().upper(), output);
        assert_eq!(p.maximum_used_terms(), 0);
    }
}
#[test]
fn reduced_reciprocal_and_negative_arguments_enclose_independent_brackets() {
    // Independent decimal brackets in turns, not certificates used by production.
    for (n, d, lo, hi) in [
        (1, 2, 737_918, 737_919),
        (3, 4, 1_024_163, 1_024_164),
        (2, 1, 1_762_081, 1_762_082),
    ] {
        let positive = Prepared::prepare(q(n, d), requirements(), &mut work()).unwrap();
        let lower = q(lo, 10_000_000);
        let upper = q(hi, 10_000_000);
        assert_eq!(
            positive
                .target()
                .lower()
                .checked_compare(lower, &mut work())
                .unwrap(),
            Ordering::Greater
        );
        assert_eq!(
            positive
                .target()
                .upper()
                .checked_compare(upper, &mut work())
                .unwrap(),
            Ordering::Less
        );
        let negative = Prepared::prepare(q(-n, d), requirements(), &mut work()).unwrap();
        assert_eq!(
            negative.target().lower(),
            positive.target().upper().negated()
        );
        assert_eq!(
            negative.target().upper(),
            positive.target().lower().negated()
        );
        assert!(positive.limb_work() > 0);
    }
}
#[test]
fn insufficient_terms_tolerance_and_work_refuse() {
    let mut req = requirements();
    req.maximum_terms = 0;
    assert_eq!(
        Prepared::prepare(q(3, 4), req, &mut work()),
        Err(R::ConstantTermLimit)
    );
    req = requirements();
    req.maximum_width_turns = Q::ZERO;
    assert_eq!(
        Prepared::prepare(q(3, 4), req, &mut work()),
        Err(R::ToleranceExceeded)
    );
    req = requirements();
    req.maximum_width_turns = Q::from_i128(-1);
    assert_eq!(
        Prepared::prepare(q(3, 4), req, &mut work()),
        Err(R::NegativeTolerance)
    );
    assert!(matches!(
        Prepared::prepare(q(3, 4), requirements(), &mut Work::new(0)),
        Err(R::Numeric(_))
    ));
}
