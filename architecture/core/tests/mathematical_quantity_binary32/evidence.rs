//! Independently reconstructed domain receipts retain full numerical metadata.
use super::*;
const LIMITS: ProjectionEvidenceLimits = ProjectionEvidenceLimits {
    source_bytes: 131072,
    target_bytes: 4096,
    detail_bytes: 2048,
    policy_bytes: 4096,
    total_bytes: 262144,
};
#[test]
fn general_evidence_replays_full_physical_receipts_without_allocation() {
    for (symbolic, unit, role, error_role) in [
        (
            true,
            Unit::Radian,
            QuantityRole::Linear,
            QuantityRole::Linear,
        ),
        (
            false,
            Unit::Radian,
            QuantityRole::Linear,
            QuantityRole::Linear,
        ),
        (
            true,
            Unit::Fahrenheit,
            QuantityRole::Point,
            QuantityRole::Delta,
        ),
        (
            true,
            Unit::Fahrenheit,
            QuantityRole::Delta,
            QuantityRole::Delta,
        ),
    ] {
        for rounding in [
            Binary32Rounding::NearestTiesEven,
            Binary32Rounding::TowardZero,
            Binary32Rounding::TowardPositive,
            Binary32Rounding::TowardNegative,
        ] {
            let maximum = limit(unit, error_role);
            let policy = MathematicalQuantityBinary32Policy {
                maximum_error: maximum,
                rounding,
            };
            let codec = MathematicalQuantityBinary32EvidenceCodec::new(policy);
            let mut producer = PreparedMathematicalQuantityBinary32::new(
                requirements(),
                rounding,
                maximum,
                200_000_000,
            )
            .unwrap();
            let mut reader = PreparedMathematicalQuantityBinary32::new(
                requirements(),
                rounding,
                maximum,
                200_000_000,
            )
            .unwrap();
            let wire = frame(symbolic, unit, role);
            let mut bytes = vec![0; LIMITS.total_bytes];
            let (_, allocations) = observe(|| {
                let sent = producer.execute(&wire, &mut work()).unwrap();
                let received = reader.execute(&wire, &mut work()).unwrap();
                let sent_target = sent.target();
                let received_target = received.target();
                let sent_facts = sent.facts();
                let received_facts = received.facts();
                let domain = MathematicalQuantityBinary32Domain::new();
                let sent_report = ProjectionReport::new(
                    &domain,
                    &policy,
                    input(&sent, &sent_target, &sent_facts),
                )
                .unwrap();
                let received_report = ProjectionReport::new(
                    &domain,
                    &policy,
                    input(&received, &received_target, &received_facts),
                )
                .unwrap();
                let n =
                    encode_projection_evidence(&sent_report, &codec, LIMITS, &mut bytes).unwrap();
                verify_projection_evidence(&received_report, &codec, LIMITS, &bytes[..n]).unwrap();
                received_report
                    .require_bound_policy(policy.identity(), policy.configuration_digest().unwrap())
                    .unwrap();
                assert!(bytes[..n].windows(wire.len()).any(|part| part == wire));
                for index in 0..n {
                    bytes[index] ^= 1;
                    assert!(verify_projection_evidence(
                        &received_report,
                        &codec,
                        LIMITS,
                        &bytes[..n]
                    )
                    .is_err());
                    bytes[index] ^= 1;
                }
                // Equal target/error is insufficient when the admitted work
                // differs: the complete source receipt must survive transport.
                let mut different_work = ExactNumericWork::new(199_999_999);
                let changed = reader.execute(&wire, &mut different_work).unwrap();
                assert_eq!(changed.target(), received_target);
                assert_eq!(changed.error(), received.error());
                let changed_target = changed.target();
                let changed_facts = changed.facts();
                let changed_report = ProjectionReport::new(
                    &domain,
                    &policy,
                    input(&changed, &changed_target, &changed_facts),
                )
                .unwrap();
                assert_eq!(
                    verify_projection_evidence(&changed_report, &codec, LIMITS, &bytes[..n]),
                    Err(ProjectionEvidenceRefusal::EvidenceMismatch)
                );
                let wrong = MathematicalQuantityBinary32EvidenceCodec::new(
                    MathematicalQuantityBinary32Policy {
                        maximum_error: RationalQuantity::new(q(2, 1_000_000), unit, error_role)
                            .unwrap(),
                        rounding,
                    },
                );
                assert_eq!(
                    verify_projection_evidence(&received_report, &wrong, LIMITS, &bytes[..n]),
                    Err(ProjectionEvidenceRefusal::PolicyConfigurationMismatch)
                );
            });
            assert_eq!(allocations, 0);
        }
    }
}
