//! Well-typed forged packets must fail numerical verification.
use super::*;
fn replace_leaf(value: &StructuredInfoValue, path: &[&str], bytes: &[u8]) -> StructuredInfoValue {
    if path.is_empty() {
        return StructuredInfoValue::leaf(value.value_type().clone(), bytes.to_vec()).unwrap();
    }
    match value.shape() {
        StructuredInfoValueShape::Record(fields) => StructuredInfoValue::record(
            value.value_type().clone(),
            fields
                .iter()
                .map(|f| {
                    StructuredFieldValue::new(
                        f.name(),
                        if f.name() == path[0] {
                            replace_leaf(f.value(), &path[1..], bytes)
                        } else {
                            f.value().clone()
                        },
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap(),
        StructuredInfoValueShape::Variant { tag, payload } if tag == path[0] => {
            StructuredInfoValue::variant(
                value.value_type().clone(),
                tag,
                replace_leaf(payload, &path[1..], bytes),
            )
            .unwrap()
        }
        _ => panic!("bad fixture path"),
    }
}
#[test]
fn canonical_types_do_not_authorize_forged_target_error_source_or_physical_identity() {
    let mut work = ExactNumericWork::new(NEAR_NUMERICAL_WORK);
    let source = RationalQuantity::new(
        ExactRational::ratio(1, 3, &mut work).unwrap(),
        Unit::resolve("m").unwrap(),
        QuantityRole::Linear,
    )
    .unwrap();
    let mut input = vec![0; QUANTITY_VALUE_MAX_ENCODED_BYTES];
    let n = encode_rational_quantity_value(source, &mut input).unwrap();
    input.truncate(n);
    let requested = Quantity::parse_plot_literal("0.0001m").unwrap();
    let tolerance = RationalQuantity::from_quantity(requested, &mut work).unwrap();
    let mut exact = NearQuantityBack::new(requested, NearQuantityAlgorithm::ExactRational).unwrap();
    exact.evaluate(&input).unwrap();
    let exact = StructuredInfoValue::from_canonical_bytes(exact.state.encoding.encoded()).unwrap();
    let StructuredInfoValueShape::Record(fields) = exact.shape() else {
        panic!("record")
    };
    let zero = fields
        .iter()
        .find(|f| f.name() == "maximum-error")
        .unwrap()
        .value();
    let StructuredInfoValueShape::Leaf(zero) = zero.shape() else {
        panic!("leaf")
    };
    let mut back = NearQuantityBack::new(requested, NearQuantityAlgorithm::Binary32).unwrap();
    back.evaluate(&input).unwrap();
    let original =
        StructuredInfoValue::from_canonical_bytes(back.state.encoding.encoded()).unwrap();
    let verifier =
        PreparedNearQuantityVerifier::new(&conduit_semantic_catalog::near_quantity_type()).unwrap();
    for (path, bytes, expected) in [
        (
            vec!["representation", "binary32", "target"],
            0x3eaa_aaaau32.to_le_bytes().to_vec(),
            NearQuantityEvidenceRefusal::InvalidClaim,
        ),
        (
            vec!["maximum-error"],
            zero.to_vec(),
            NearQuantityEvidenceRefusal::InvalidClaim,
        ),
        (
            vec!["source"],
            zero.to_vec(),
            NearQuantityEvidenceRefusal::SourceMismatch,
        ),
        (
            vec!["unit"],
            Unit::resolve("Hz").unwrap().encode().to_vec(),
            NearQuantityEvidenceRefusal::SourceMismatch,
        ),
    ] {
        let forged = replace_leaf(&original, &path, &bytes)
            .canonical_bytes()
            .unwrap();
        // Every packet remains canonical with the exact registered schema.
        assert_eq!(
            StructuredInfoValue::from_canonical_bytes(&forged)
                .unwrap()
                .value_type(),
            original.value_type()
        );
        assert_eq!(
            verifier.verify(
                &forged,
                QUANTITY_INFO_ID,
                tolerance,
                None,
                &mut ExactNumericWork::new(NEAR_NUMERICAL_WORK)
            ),
            Err(expected),
            "{path:?}"
        );
    }
    assert_eq!(
        verifier.verify(
            back.state.encoding.encoded(),
            QUANTITY_INFO_ID,
            tolerance,
            None,
            &mut ExactNumericWork::new(0)
        ),
        Err(NearQuantityEvidenceRefusal::WorkLimit)
    );
}
