use conduit_core::{
    Binary64Rounding as Mode, ExactNumericWork as Work, ExactRational as Q,
    MathematicalEnclosureRequirements as Requirements, MathematicalValue as Value,
    PreparedMathematicalEnclosure as Enclosure, PreparedSymbolicBinary64 as Prepared,
    SymbolicBinary64Refusal as Refusal,
};
fn work() -> Work {
    Work::new(100_000_000)
}
fn q(n: i128, d: i128) -> Q {
    Q::ratio(n, d, &mut work()).unwrap()
}
fn enclosure(name: &str, width: Q) -> Enclosure {
    let source = Value::parse_literal(name, &mut work()).unwrap();
    Enclosure::prepare(
        &source,
        Requirements {
            maximum_width: width,
            maximum_leaf_width: q(1, 100_000_000_000_000_000_000),
            maximum_constant_terms: 32,
            maximum_root_refinements: 96,
            maximum_scratch_payload_bytes: 131072,
            maximum_retained_payload_bytes: 131072,
        },
        &mut work(),
    )
    .unwrap()
}
#[test]
fn constants_keep_exact_source_and_prove_correct_rounding() {
    for (name, bits) in [("pi", 0x400921fb54442d18), ("tau", 0x401921fb54442d18)] {
        let bound = enclosure(name, q(1, 10_000_000_000_000_000_000));
        let source = bound.source().clone();
        let result = Prepared::prepare(
            bound,
            Mode::NearestTiesEven,
            q(1, 1_000_000_000_000_000),
            &mut work(),
        )
        .unwrap();
        assert_eq!(result.target().bits(), bits);
        assert_eq!(result.enclosure().source(), &source);
        assert!(result.spent_limb_work() > 0);
        assert!(Prepared::prepare(
            result.enclosure().clone(),
            Mode::NearestTiesEven,
            Q::ZERO,
            &mut work()
        )
        .is_err());
    }
}
#[test]
fn broad_enclosure_refuses_to_guess_rounding() {
    let source = Value::parse_literal("pi", &mut work()).unwrap();
    let requirements = Requirements {
        maximum_width: q(1, 100),
        maximum_leaf_width: q(1, 100),
        maximum_constant_terms: 32,
        maximum_root_refinements: 96,
        maximum_scratch_payload_bytes: 131072,
        maximum_retained_payload_bytes: 131072,
    };
    let broad = Enclosure::prepare(&source, requirements, &mut work()).unwrap();
    assert_eq!(
        Prepared::prepare(broad, Mode::NearestTiesEven, Q::ONE, &mut work()),
        Err(Refusal::RoundingUndetermined)
    );
}
