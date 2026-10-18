//! A valid number remains valid when its consumer exhausts admitted work.
use super::*;
#[test]
fn rational_extraction_exhaustion_is_work_refusal_not_malformed_input() {
    let mut preparation = ExactNumericWork::new(1_000_000);
    let q = ExactRational::ratio(1, 3, &mut preparation).unwrap();
    let mut payload = [0; 2 + crate::EXACT_RATIONAL_MAX_ENCODED_BYTES];
    let n = crate::MathematicalValue::rational(q)
        .encode(&mut payload)
        .unwrap();
    let payload = &payload[..n];
    let mut measured = ExactNumericWork::new(1_000_000);
    crate::MathematicalValueView::decode(payload, &mut measured).unwrap();
    assert!(measured.spent() > 0);
    let canonical = crate::StructuredInfoValue::leaf(
        StructuredInfoType::leaf(crate::kind_id(crate::MATHEMATICAL_VALUE_ID)).unwrap(),
        payload.to_vec(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let value = validate_canonical_structured_value(&canonical).unwrap();
    // This permits validated decoding, then exhausts the second metered read.
    assert_eq!(
        number(value, &mut ExactNumericWork::new(measured.spent())),
        Err(Refusal::WorkLimit)
    );
    assert_eq!(number(value, &mut ExactNumericWork::new(1_000_000)), Ok(q));
}
