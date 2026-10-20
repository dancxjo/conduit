//! Arithmetic prerequisite for the qualified Source fold; not Source admission proof.
use conduit_core::*;
#[path = "common/numerical_allocation.rs"]
mod allocation_probe;
use allocation_probe::observe;

fn work() -> ExactNumericWork {
    ExactNumericWork::new(100_000_000)
}
fn q(value: &str) -> ExactRational {
    ExactRational::parse_decimal(value, &mut work()).unwrap()
}
fn template(tolerance: ExactRational) -> PreparedBinary64Algorithm {
    let input = Binary64ProjectedInput::new(
        ExactRational::ZERO,
        IeeeF64::from_bits(0),
        q("0.001"),
        &mut work(),
    )
    .unwrap();
    PreparedBinary64Algorithm::prepare(
        &[
            Binary64Operation::ProjectedConstant(Box::new(input)),
            Binary64Operation::Constant(Box::new(q("10000000000000000"))),
            Binary64Operation::Add { left: 0, right: 1 },
            Binary64Operation::Subtract { left: 2, right: 1 },
        ],
        Binary64Rounding::NearestTiesEven,
        tolerance,
        Binary64AlgorithmLimits {
            maximum_steps: 4,
            maximum_retained_bytes: 1_000_000,
        },
        &mut work(),
    )
    .unwrap()
}

#[test]
fn intermediate_rounding_separates_broad_and_tight_consumers_for_incoming_binary32() {
    let tenth = q("0.1");
    // Exact widening of binary32 0x3dcccccd; no host float arithmetic oracle.
    let incoming = Binary64ProjectedInput::new(
        tenth,
        IeeeF64::from_bits(0x3fb9_9999_a000_0000),
        q("0.001"),
        &mut work(),
    )
    .unwrap();
    let broad = template(q("0.2"));
    let tight = template(q("0.01"));
    let mut admitted = PreparedBinary64AlgorithmRunner::new(&broad, &[0], 3_000_000).unwrap();
    let mut refused = PreparedBinary64AlgorithmRunner::new(&tight, &[0], 3_000_000).unwrap();
    let before = refused.last_success().clone();
    let retained = admitted.retained_payload_bytes();
    let arguments = [Binary64AlgorithmInput::Projected(&incoming)];
    let (correct, allocations) = observe(|| {
        let mut charged = work();
        let accepted = admitted.execute_inputs(&arguments, &mut charged).unwrap();
        let accepted_correct = accepted.source() == tenth
            && accepted.target().bits() == 0
            && accepted.error() == Binary64ErrorClaim::ExactDistance(tenth)
            && accepted.steps()[0].semantic_value() == tenth
            && accepted.steps()[0].projection().target() == incoming.target()
            && accepted.steps()[3].projection().source() == ExactRational::ZERO
            && charged.spent() > 0;
        let mut failed_work = work();
        accepted_correct
            && matches!(
                refused.execute_inputs(&arguments, &mut failed_work),
                Err(Binary64AlgorithmRefusal::Projection(
                    Binary64Refusal::ToleranceExceeded
                ))
            )
            && failed_work.spent() > 0
            && refused.last_success() == &before
    });
    assert!(correct);
    assert_eq!(allocations, 0);
    assert_eq!(admitted.retained_payload_bytes(), retained);
    // Rounding only the exact final answer would wrongly fit the tight contract.
    let final_only = PreparedBinary64Projection::prepare(
        tenth,
        Binary64Rounding::NearestTiesEven,
        q("0.01"),
        &mut work(),
    )
    .unwrap();
    assert_ne!(
        final_only.target().bits(),
        admitted.last_success().target().bits()
    );
}
