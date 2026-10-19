//! Independent receiver recomputes a typed interval carrier under admitted bounds.
use conduit_core::*;
#[path = "common/numerical_allocation.rs"]
mod allocation_probe;
use allocation_probe::observe;
type R = MathematicalEnclosureEvidenceRefusal;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(800_000_000)
}
fn q(n: i128, d: i128) -> ExactRational {
    ExactRational::ratio(n, d, &mut work()).unwrap()
}
fn req() -> MathematicalEnclosureRequirements {
    MathematicalEnclosureRequirements {
        maximum_width: q(1, 100_000_000_000_000_000),
        maximum_leaf_width: q(1, 100_000_000_000_000_000_000),
        maximum_constant_terms: 32,
        maximum_root_refinements: 64,
        maximum_scratch_payload_bytes: 131072,
        maximum_retained_payload_bytes: 131072,
    }
}
fn schema(name: &str) -> StructuredInfoType {
    let ty = StructuredInfoType::leaf(kind_id(MATHEMATICAL_VALUE_ID)).unwrap();
    StructuredInfoType::record(
        kind_id(name),
        ["lower", "maximum-width", "source", "upper", "width"]
            .into_iter()
            .map(|n| StructuredFieldType::new(n, ty.clone()).unwrap())
            .collect(),
    )
    .unwrap()
}
fn bytes(value: &MathematicalValue) -> Vec<u8> {
    let mut out = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let n = value.encode(&mut out).unwrap();
    out.truncate(n);
    out
}
fn pi() -> MathematicalValue {
    MathematicalValue::parse_literal("pi", &mut work()).unwrap()
}
fn envelope(
    ty: &StructuredInfoType,
    source: &MathematicalValue,
    override_number: Option<(&str, ExactRational)>,
) -> Vec<u8> {
    let p = PreparedMathematicalEnclosure::prepare(source, req(), &mut work()).unwrap();
    let leaf = StructuredInfoType::leaf(kind_id(MATHEMATICAL_VALUE_ID)).unwrap();
    let values = [
        ("lower", p.target().lower()),
        ("maximum-width", req().maximum_width),
        ("upper", p.target().upper()),
        ("width", p.width()),
    ];
    let mut fields = values
        .into_iter()
        .map(|(n, v)| {
            let v = override_number
                .filter(|(name, _)| *name == n)
                .map_or(v, |(_, value)| value);
            StructuredFieldValue::new(
                n,
                StructuredInfoValue::leaf(leaf.clone(), bytes(&MathematicalValue::rational(v)))
                    .unwrap(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    fields.push(
        StructuredFieldValue::new(
            "source",
            StructuredInfoValue::leaf(leaf, bytes(source)).unwrap(),
        )
        .unwrap(),
    );
    StructuredInfoValue::record(ty.clone(), fields)
        .unwrap()
        .canonical_bytes()
        .unwrap()
}
fn verifier(ty: &StructuredInfoType) -> PreparedMathematicalEnclosureVerifier {
    PreparedMathematicalEnclosureVerifier::new(ty, req(), 800_000_000, 131072).unwrap()
}
#[test]
fn independent_receiver_admits_weaker_width_but_refuses_tightening_and_changed_claims() {
    let ty = schema("test/enclosure@1");
    let source = pi();
    let source_bytes = bytes(&source);
    let encoded = envelope(&ty, &source, None);
    let (mut v, preparation_allocations) = observe(|| verifier(&ty));
    println!("ENCLOSURE_EVIDENCE_PREPARATION allocations={preparation_allocations} retained_payload_bytes={}", v.retained_payload_bytes());
    let capacity = v.retained_payload_bytes();
    let wrong_source = bytes(&MathematicalValue::rational(ExactRational::ONE));
    let lower = envelope(&ty, &source, Some(("lower", ExactRational::ZERO)));
    let upper = envelope(&ty, &source, Some(("upper", ExactRational::ZERO)));
    let false_exact = envelope(&ty, &source, Some(("width", ExactRational::ZERO)));
    let wrong_policy = envelope(&ty, &source, Some(("maximum-width", ExactRational::ONE)));
    let (_, allocations) = observe(|| {
        for _ in 0..2 {
            let mut w = work();
            let checked = v
                .verify(&encoded, q(1, 1000), Some(&source_bytes), &mut w)
                .unwrap();
            assert!(checked.width() != ExactRational::ZERO);
            assert!(w.spent() >= MATHEMATICAL_ENCLOSURE_EVIDENCE_VALIDATION_WORK);
            assert!(matches!(
                v.verify(&encoded, q(1, 1000), Some(&wrong_source), &mut work()),
                Err(R::Enclosure(MathematicalEnclosureRefusal::SourceMismatch))
            ));
            for forged in [&lower, &upper, &false_exact] {
                assert!(matches!(
                    v.verify(forged, q(1, 1000), None, &mut work()),
                    Err(R::Enclosure(MathematicalEnclosureRefusal::InvalidClaim))
                ));
            }
            assert!(matches!(
                v.verify(&encoded, ExactRational::ZERO, None, &mut work()),
                Err(R::InsufficientTolerance)
            ));
            assert!(matches!(
                v.verify(&wrong_policy, ExactRational::ONE, None, &mut work()),
                Err(R::PolicyMismatch)
            ));
        }
    });
    assert_eq!(allocations, 0);
    assert_eq!(v.retained_payload_bytes(), capacity);
}
#[test]
fn schema_extent_storage_and_work_limits_are_independent_and_recoverable() {
    let ty = schema("test/enclosure@1");
    let source = pi();
    let encoded = envelope(&ty, &source, None);
    let mut v = verifier(&ty);
    let foreign = envelope(&schema("foreign/enclosure@1"), &source, None);
    let oversized = vec![0; MATHEMATICAL_ENCLOSURE_EVIDENCE_MAX_BYTES + 1];
    assert!(matches!(
        v.verify(&foreign, ExactRational::ONE, None, &mut work()),
        Err(R::Schema)
    ));
    // A foreign malformed body must not be generically parsed using this
    // profile's allowance for only five mathematical fields.
    assert!(matches!(
        v.verify(
            &foreign[..foreign.len() - 1],
            ExactRational::ONE,
            None,
            &mut work()
        ),
        Err(R::Schema)
    ));
    assert!(matches!(
        v.verify(&oversized, ExactRational::ONE, None, &mut work()),
        Err(R::Extent)
    ));
    assert!(matches!(
        v.verify(
            &encoded[..encoded.len() - 1],
            ExactRational::ONE,
            None,
            &mut work()
        ),
        Err(R::Malformed)
    ));
    assert!(matches!(
        v.verify(&encoded, ExactRational::from_i128(-1), None, &mut work()),
        Err(R::NegativeTolerance)
    ));
    assert!(matches!(
        v.verify(
            &encoded,
            ExactRational::ONE,
            None,
            &mut ExactNumericWork::new(0)
        ),
        Err(R::Numeric(ExactNumericRefusal::WorkLimit))
    ));
    assert!(v
        .verify(&encoded, ExactRational::ONE, None, &mut work())
        .is_ok());
    let exact = v.retained_payload_bytes() as u32;
    assert!(PreparedMathematicalEnclosureVerifier::new(&ty, req(), 800_000_000, exact).is_ok());
    assert!(matches!(
        PreparedMathematicalEnclosureVerifier::new(&ty, req(), 800_000_000, exact - 1),
        Err(R::StorageLimit)
    ));
    assert!(PreparedMathematicalEnclosureVerifier::new(
        &StructuredInfoType::leaf(kind_id(MATHEMATICAL_VALUE_ID)).unwrap(),
        req(),
        800_000_000,
        131072
    )
    .is_err());
    assert!(matches!(
        PreparedMathematicalEnclosureVerifier::new(
            &ty,
            req(),
            MATHEMATICAL_ENCLOSURE_EVIDENCE_VALIDATION_WORK,
            131072
        ),
        Err(R::Numeric(ExactNumericRefusal::WorkLimit))
    ));
    let mut bounded = PreparedMathematicalEnclosureVerifier::new(
        &ty,
        req(),
        MATHEMATICAL_ENCLOSURE_EVIDENCE_VALIDATION_WORK + 1,
        131072,
    )
    .unwrap();
    assert!(matches!(
        bounded.verify(
            &encoded,
            ExactRational::ONE,
            None,
            &mut ExactNumericWork::new(u64::MAX)
        ),
        Err(R::Numeric(ExactNumericRefusal::WorkLimit))
    ));
}

#[test]
fn exact_consumer_accepts_only_recomputed_zero_width() {
    let ty = schema("test/enclosure@1");
    let point = MathematicalValue::rational(q(3, 10));
    let encoded = envelope(&ty, &point, None);
    let mut verifier = verifier(&ty);
    let checked = verifier
        .verify(
            &encoded,
            ExactRational::ZERO,
            Some(&bytes(&point)),
            &mut work(),
        )
        .unwrap();
    assert_eq!(checked.target().lower(), q(3, 10));
    assert_eq!(checked.width(), ExactRational::ZERO);
}
