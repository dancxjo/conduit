use conduit_form::rust_binding::NativeRustBinding;
use conduit_web::{JsonCollectionRefusal, JsonRefusal, JsonSummaryRefusal};

fn round_trip<T>(value: T)
where
    T: NativeRustBinding + Copy + core::fmt::Debug + PartialEq,
{
    let structured = value.into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn json_refusal_families_have_native_identity_and_exact_round_trips() {
    round_trip(JsonRefusal::DuplicateKey);
    round_trip(JsonCollectionRefusal::InvalidValue(
        JsonRefusal::StringByteOverflow,
    ));
    round_trip(JsonSummaryRefusal::InvalidValue(
        JsonRefusal::NumericOverflow,
    ));
}

#[test]
fn json_refusal_details_remain_exact() {
    assert_eq!(JsonRefusal::MalformedSyntax.detail(), 1);
    assert_eq!(JsonRefusal::NonCanonicalValue.detail(), 13);
    assert_eq!(
        JsonSummaryRefusal::InvalidValue(JsonRefusal::DuplicateKey).detail(),
        12
    );
}
