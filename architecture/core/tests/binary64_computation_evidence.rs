//! Shared CPE transport retains the complete ordered computation, not only its answer.
use conduit_core::{projection::*, *};
#[path = "common/numerical_allocation.rs"]
mod allocation_probe;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(200_000_000)
}
fn q(n: i128, d: i128) -> ExactRational {
    ExactRational::ratio(n, d, &mut work()).unwrap()
}
fn input<'a>(
    s: &'a PreparedBinary64Algorithm,
    t: &'a IeeeF64,
    f: &'a [ProjectionFact<'a, Binary64AlgorithmDomain>],
) -> ProjectionInput<'a, Binary64AlgorithmDomain> {
    let text = |s| ProjectionText::new(s).unwrap();
    ProjectionInput {
        basis: ProjectionBasis {
            report: text("fixture/ordered-report"),
            source: text("fixture/retained-computation"),
            target: Some(text("fixture/target")),
            projector: text("fixture/binary64"),
            requested_route: text("fixture/route"),
            boundary: None,
        },
        source: s,
        target: Some(t),
        facts: f,
        native: &[],
        scores: &[],
        admitted: &[],
        attempts: &[],
        selected_attempt: None,
        mechanism: ProjectionMechanism::Completed,
        diagnostics: &[],
    }
}
fn program() -> Vec<Binary64Operation> {
    vec![
        Binary64Operation::ProjectedConstant(Box::new(
            Binary64ProjectedInput::new(
                q(1, 10),
                IeeeF64::from_bits(0x3fb9_9999_a000_0000),
                q(1, 1000),
                &mut work(),
            )
            .unwrap(),
        )),
        Binary64Operation::Constant(Box::new(q(10_000_000_000_000_000, 1))),
        Binary64Operation::Add { left: 0, right: 1 },
        Binary64Operation::Subtract { left: 2, right: 1 },
    ]
}
fn prepare(ops: &[Binary64Operation], mode: Binary64Rounding) -> PreparedBinary64Algorithm {
    PreparedBinary64Algorithm::prepare(
        ops,
        mode,
        q(3, 1),
        Binary64AlgorithmLimits {
            maximum_steps: 32,
            maximum_retained_bytes: 2_000_000,
        },
        &mut work(),
    )
    .unwrap()
}
#[test]
fn all_modes_replay_complete_ordered_receipts_without_transport_allocation() {
    for mode in [
        Binary64Rounding::NearestTiesEven,
        Binary64Rounding::TowardZero,
        Binary64Rounding::TowardPositive,
        Binary64Rounding::TowardNegative,
    ] {
        let ops = program();
        let source = prepare(&ops, mode);
        let replay = prepare(&ops, mode);
        let domain = Binary64AlgorithmDomain;
        let policy = Binary64AlgorithmPolicy {
            maximum_error: q(3, 1),
            rounding: mode,
        };
        let codec = Binary64AlgorithmEvidenceCodec::new(policy);
        let target = source.target();
        let facts = source.projection_facts();
        let report =
            ProjectionReport::new(&domain, &policy, input(&source, &target, &facts)).unwrap();
        let target2 = replay.target();
        let facts2 = replay.projection_facts();
        let report2 =
            ProjectionReport::new(&domain, &policy, input(&replay, &target2, &facts2)).unwrap();
        let mut bytes = vec![0; BINARY64_ALGORITHM_EVIDENCE_LIMITS.total_bytes];
        let (length, allocations) = allocation_probe::observe(|| {
            let n = encode_projection_evidence(
                &report,
                &codec,
                BINARY64_ALGORITHM_EVIDENCE_LIMITS,
                &mut bytes,
            )
            .unwrap();
            verify_projection_evidence(
                &report2,
                &codec,
                BINARY64_ALGORITHM_EVIDENCE_LIMITS,
                &bytes[..n],
            )
            .unwrap();
            n
        });
        assert_eq!(allocations, 0);
        assert_eq!(&bytes[..4], b"CPE\x01");
        assert!(length < bytes.len());
        // Every canonical byte participates in verification, including the ordered steps.
        for offset in 0..length {
            bytes[offset] ^= 1;
            assert!(
                verify_projection_evidence(
                    &report2,
                    &codec,
                    BINARY64_ALGORITHM_EVIDENCE_LIMITS,
                    &bytes[..length]
                )
                .is_err(),
                "offset {offset}"
            );
            bytes[offset] ^= 1;
        }
        assert!(verify_projection_evidence(
            &report2,
            &codec,
            BINARY64_ALGORITHM_EVIDENCE_LIMITS,
            &bytes[..length - 1]
        )
        .is_err());
        bytes[length] = 0;
        assert!(verify_projection_evidence(
            &report2,
            &codec,
            BINARY64_ALGORITHM_EVIDENCE_LIMITS,
            &bytes[..length + 1]
        )
        .is_err());
        let changed = Binary64AlgorithmEvidenceCodec::new(Binary64AlgorithmPolicy {
            maximum_error: q(4, 1),
            rounding: mode,
        });
        assert_eq!(
            projection_evidence_len(&report, &changed, BINARY64_ALGORITHM_EVIDENCE_LIMITS),
            Err(ProjectionEvidenceRefusal::PolicyConfigurationMismatch)
        );
    }
}
#[test]
fn final_only_rounding_cannot_verify_the_ordered_computation_receipt() {
    let mode = Binary64Rounding::NearestTiesEven;
    let ordered = prepare(&program(), mode);
    let final_only = prepare(&[Binary64Operation::Constant(Box::new(q(1, 10)))], mode);
    assert_ne!(ordered.target(), final_only.target());
    let d = Binary64AlgorithmDomain;
    let p = Binary64AlgorithmPolicy {
        maximum_error: q(3, 1),
        rounding: mode,
    };
    let c = Binary64AlgorithmEvidenceCodec::new(p);
    let a = ordered.target();
    let f = ordered.projection_facts();
    let r = ProjectionReport::new(&d, &p, input(&ordered, &a, &f)).unwrap();
    let b = final_only.target();
    let g = final_only.projection_facts();
    let other = ProjectionReport::new(&d, &p, input(&final_only, &b, &g)).unwrap();
    let mut bytes = vec![0; BINARY64_ALGORITHM_EVIDENCE_LIMITS.total_bytes];
    let n =
        encode_projection_evidence(&r, &c, BINARY64_ALGORITHM_EVIDENCE_LIMITS, &mut bytes).unwrap();
    assert!(verify_projection_evidence(
        &other,
        &c,
        BINARY64_ALGORITHM_EVIDENCE_LIMITS,
        &bytes[..n]
    )
    .is_err());
    let mut short = vec![0xA5; n - 1];
    assert_eq!(
        encode_projection_evidence(&r, &c, BINARY64_ALGORITHM_EVIDENCE_LIMITS, &mut short),
        Err(ProjectionEvidenceRefusal::BufferTooSmall)
    );
    assert!(short.iter().all(|b| *b == 0xA5));
}

#[test]
fn maximum_step_inventory_preserves_every_operation_and_bound_policy() {
    let mut ops = program();
    ops.push(Binary64Operation::IeeeConstant(IeeeF64::from_bits(1 << 63)));
    ops.push(Binary64Operation::Negate { input: 4 });
    ops.push(Binary64Operation::Multiply { left: 0, right: 5 });
    ops.push(Binary64Operation::Divide { left: 0, right: 1 });
    while ops.len() < BINARY64_ALGORITHM_MAX_STEPS {
        ops.push(Binary64Operation::Negate {
            input: (ops.len() - 1) as u8,
        });
    }
    let value = prepare(&ops, Binary64Rounding::TowardNegative);
    let replay = prepare(&ops, Binary64Rounding::TowardNegative);
    assert_eq!(value.steps().len(), 32);
    let domain = Binary64AlgorithmDomain;
    let policy = Binary64AlgorithmPolicy {
        maximum_error: q(3, 1),
        rounding: value.rounding(),
    };
    let codec = Binary64AlgorithmEvidenceCodec::new(policy);
    let facts = value.projection_facts();
    let target = value.target();
    let report = ProjectionReport::new(&domain, &policy, input(&value, &target, &facts)).unwrap();
    let facts2 = replay.projection_facts();
    let target2 = replay.target();
    let report2 =
        ProjectionReport::new(&domain, &policy, input(&replay, &target2, &facts2)).unwrap();
    assert_eq!(
        report2.require_bound_policy(policy.identity(), policy.configuration_digest().unwrap()),
        Ok(&target2)
    );
    let mut bytes = vec![0; BINARY64_ALGORITHM_EVIDENCE_LIMITS.total_bytes];
    let (n, allocations) = allocation_probe::observe(|| {
        let n = encode_projection_evidence(
            &report,
            &codec,
            BINARY64_ALGORITHM_EVIDENCE_LIMITS,
            &mut bytes,
        )
        .unwrap();
        verify_projection_evidence(
            &report2,
            &codec,
            BINARY64_ALGORITHM_EVIDENCE_LIMITS,
            &bytes[..n],
        )
        .unwrap();
        n
    });
    assert_eq!(allocations, 0);
    assert!(n < bytes.len());
}
