//! Physical exact and single-round receipts use the same bounded report transport.
use conduit_core::{projection::*, *};
#[path = "common/numerical_allocation.rs"]
mod allocation_probe;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(200_000_000)
}
fn text(s: &str) -> ProjectionText<'_> {
    ProjectionText::new(s).unwrap()
}
fn input<'a, D: ProjectionDomain>(
    s: &'a D::Source,
    t: &'a D::Target,
    f: &'a [ProjectionFact<'a, D>],
) -> ProjectionInput<'a, D> {
    ProjectionInput {
        basis: ProjectionBasis {
            report: text("fixture/physical-report"),
            source: text("fixture/retained-source-wire"),
            target: Some(text("fixture/physical-target")),
            projector: text("fixture/physical-projector"),
            requested_route: text("fixture/physical-route"),
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
fn quantity(n: i128, d: i128, unit: &str, role: QuantityRole) -> RationalQuantity {
    RationalQuantity::new(
        ExactRational::ratio(n, d, &mut work()).unwrap(),
        Unit::resolve(unit).unwrap(),
        role,
    )
    .unwrap()
}
struct Reader<'a>(&'a [u8]);
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> &'a [u8] {
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        a
    }
    fn byte(&mut self) -> u8 {
        self.take(1)[0]
    }
    fn u32(&mut self) -> u32 {
        u32::from_le_bytes(self.take(4).try_into().unwrap())
    }
    fn u64(&mut self) -> u64 {
        u64::from_le_bytes(self.take(8).try_into().unwrap())
    }
    fn blob(&mut self) -> &'a [u8] {
        let n = self.u32() as usize;
        self.take(n)
    }
    fn rational(&mut self) -> ExactRational {
        ExactRational::decode(self.blob(), &mut work()).unwrap()
    }
    fn physical(&mut self) -> (Unit, QuantityRole) {
        let u = Unit::decode(self.blob()).unwrap();
        let r = match self.byte() {
            0 => QuantityRole::Linear,
            1 => QuantityRole::Point,
            2 => QuantityRole::Delta,
            _ => panic!("invalid role"),
        };
        (u, r)
    }
    fn quantity(&mut self) -> RationalQuantity {
        let q = self.rational();
        let (u, r) = self.physical();
        RationalQuantity::new(q, u, r).unwrap()
    }
    fn target(&mut self) -> (u32, Unit, QuantityRole) {
        let b = self.u32();
        let (u, r) = self.physical();
        (b, u, r)
    }
    fn error(&mut self) -> Binary32ErrorClaim {
        let t = self.byte();
        let q = self.rational();
        match t {
            0 => Binary32ErrorClaim::ExactDistance(q),
            1 => Binary32ErrorClaim::CertifiedBound(q),
            _ => panic!("invalid error"),
        }
    }
}
fn round_tag(r: Binary32Rounding) -> u8 {
    match r {
        Binary32Rounding::NearestTiesEven => 0,
        Binary32Rounding::TowardZero => 1,
        Binary32Rounding::TowardPositive => 2,
        Binary32Rounding::TowardNegative => 3,
    }
}
#[test]
fn full_physical_receipt_roundtrips_all_rounding_modes_and_wide_denotation_without_allocation() {
    let high = ExactRational::from_i128(2)
        .checked_power(4095, &mut work())
        .unwrap();
    let wide = high
        .checked_divide(
            high.checked_subtract(ExactRational::ONE, &mut work())
                .unwrap(),
            &mut work(),
        )
        .unwrap();
    let sources = [
        quantity(1, 3, "m", QuantityRole::Linear),
        quantity(-1, 3, "°C", QuantityRole::Point),
        quantity(2, 1, "°", QuantityRole::Linear),
        RationalQuantity::new(wide, Unit::Meter, QuantityRole::Linear).unwrap(),
    ];
    for (case, source) in sources.into_iter().enumerate() {
        let role = if source.role() == QuantityRole::Point {
            QuantityRole::Delta
        } else {
            source.role()
        };
        let original = RationalQuantity::new(
            ExactRational::ratio(1, 1_000_000, &mut work()).unwrap(),
            source.unit(),
            role,
        )
        .unwrap();
        let allowance = RationalQuantity::new(
            ExactRational::ratio(2, 1_000_000, &mut work()).unwrap(),
            source.unit(),
            role,
        )
        .unwrap();
        for rounding in [
            Binary32Rounding::NearestTiesEven,
            Binary32Rounding::TowardZero,
            Binary32Rounding::TowardPositive,
            Binary32Rounding::TowardNegative,
        ] {
            let mut preparation = ExactNumericWork::new(
                200_000_000 - case as u64 * 101 - u64::from(round_tag(rounding)) * 137,
            );
            ExactRational::ONE
                .checked_add(ExactRational::ONE, &mut preparation)
                .unwrap();
            let admitted = preparation.remaining();
            let receipt = PreparedQuantityBinary32Projection::prepare(
                source,
                rounding,
                original,
                &mut preparation,
            )
            .unwrap();
            let target = receipt.target();
            let facts = receipt.facts();
            let policy = QuantityBinary32ProjectionPolicy {
                maximum_error: allowance,
                rounding,
            };
            let codec = QuantityBinary32EvidenceCodec::new(policy);
            let report = ProjectionReport::new(
                &QuantityBinary32ProjectionDomain,
                &policy,
                input(&receipt, &target, &facts),
            )
            .unwrap();
            report
                .require_bound_policy(policy.identity(), policy.configuration_digest().unwrap())
                .unwrap();
            let mut bytes = [0; RATIONAL_NEAR_PROJECTION_EVIDENCE_MAX_BYTES];
            let (n, allocations) = allocation_probe::observe(|| {
                let n = encode_projection_evidence(
                    &report,
                    &codec,
                    RATIONAL_NEAR_PROJECTION_EVIDENCE_LIMITS,
                    &mut bytes,
                )
                .unwrap();
                verify_projection_evidence(
                    &report,
                    &codec,
                    RATIONAL_NEAR_PROJECTION_EVIDENCE_LIMITS,
                    &bytes[..n],
                )
                .unwrap();
                n
            });
            assert_eq!(allocations, 0);
            let view = inspect_projection_evidence(&bytes[..n]).unwrap();
            assert!(view.source_bytes.len() <= QUANTITY_BINARY32_EVIDENCE_SOURCE_MAX_BYTES);
            let mut r = Reader(view.source_bytes);
            assert_eq!(r.quantity(), source);
            let numeric = receipt.numeric();
            assert_eq!(r.rational(), numeric.source());
            assert_eq!(r.u32(), numeric.target().bits());
            assert_eq!(r.rational(), numeric.represented());
            assert_eq!(r.byte(), round_tag(rounding));
            assert_eq!(r.byte(), u8::from(numeric.inexact()));
            assert_eq!(r.error(), numeric.error());
            assert_eq!(r.rational(), numeric.maximum_error());
            assert_eq!(r.u64(), numeric.spent_limb_work());
            assert_eq!(r.u64(), admitted);
            assert_eq!(
                r.target(),
                (target.coordinate().bits(), target.unit(), target.role())
            );
            assert_eq!(r.quantity(), original);
            assert!(r.0.is_empty());
            let mut r = Reader(view.target_bytes.unwrap());
            assert_eq!(
                r.target(),
                (target.coordinate().bits(), target.unit(), target.role())
            );
            assert!(r.0.is_empty());
            let mut r = Reader(view.policy_bytes);
            assert_eq!(r.quantity(), allowance);
            assert_eq!(r.byte(), round_tag(rounding));
            assert!(r.0.is_empty());
            for f in view.facts() {
                if let ProjectionEvidenceFact::Lost { detail, .. } = f {
                    let mut r = Reader(detail);
                    assert_eq!(r.error(), receipt.error());
                    assert!(r.0.is_empty());
                }
            }
            if case == 0 && rounding == Binary32Rounding::NearestTiesEven {
                for i in 0..n {
                    bytes[i] ^= 1;
                    assert!(verify_projection_evidence(
                        &report,
                        &codec,
                        RATIONAL_NEAR_PROJECTION_EVIDENCE_LIMITS,
                        &bytes[..n]
                    )
                    .is_err());
                    bytes[i] ^= 1;
                }
            }
            let changed = QuantityBinary32EvidenceCodec::new(QuantityBinary32ProjectionPolicy {
                maximum_error: original,
                rounding,
            });
            assert_eq!(
                projection_evidence_len(
                    &report,
                    &changed,
                    RATIONAL_NEAR_PROJECTION_EVIDENCE_LIMITS
                ),
                Err(ProjectionEvidenceRefusal::PolicyConfigurationMismatch)
            );
        }
    }
}
#[test]
fn exact_coordinate_policy_is_concrete_physical_and_fully_transportable() {
    let source = quantity(-7, 3, "°C", QuantityRole::Point);
    let facts = ExactNearProjectionDomain.facts();
    let good = ExactNearProjectionPolicy {
        maximum_error: quantity(0, 1, "°C", QuantityRole::Delta),
    };
    for (policy, allowed) in [
        (good, true),
        (
            ExactNearProjectionPolicy {
                maximum_error: quantity(0, 1, "°C", QuantityRole::Point),
            },
            false,
        ),
        (
            ExactNearProjectionPolicy {
                maximum_error: quantity(0, 1, "s", QuantityRole::Linear),
            },
            false,
        ),
        (
            ExactNearProjectionPolicy {
                maximum_error: quantity(-1, 1, "°C", QuantityRole::Delta),
            },
            false,
        ),
    ] {
        let report = ProjectionReport::new(
            &ExactNearProjectionDomain,
            &policy,
            input(&source, &source, &facts),
        )
        .unwrap();
        assert_eq!(report.require_exact(), Ok(&source));
        assert_eq!(
            report
                .require_bound_policy(policy.identity(), policy.configuration_digest().unwrap())
                .is_ok(),
            allowed
        );
        let codec = ExactNearEvidenceCodec::new(policy);
        let mut bytes = [0; RATIONAL_NEAR_PROJECTION_EVIDENCE_MAX_BYTES];
        let (n, allocations) = allocation_probe::observe(|| {
            let n = encode_projection_evidence(
                &report,
                &codec,
                RATIONAL_NEAR_PROJECTION_EVIDENCE_LIMITS,
                &mut bytes,
            )
            .unwrap();
            verify_projection_evidence(
                &report,
                &codec,
                RATIONAL_NEAR_PROJECTION_EVIDENCE_LIMITS,
                &bytes[..n],
            )
            .unwrap();
            n
        });
        assert_eq!(allocations, 0);
        let view = inspect_projection_evidence(&bytes[..n]).unwrap();
        let mut r = Reader(view.source_bytes);
        assert_eq!(r.quantity(), source);
        assert!(r.0.is_empty());
        let mut r = Reader(view.target_bytes.unwrap());
        assert_eq!(r.quantity(), source);
        assert!(r.0.is_empty());
        let mut r = Reader(view.policy_bytes);
        assert_eq!(r.quantity(), policy.maximum_error);
        assert!(r.0.is_empty());
        assert_eq!(view.facts().count(), 3);
    }
}
