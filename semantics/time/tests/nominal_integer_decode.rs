//! A generated integer nominal keeps the constructor's exact range refusal.
use conduit_core::StructuredInfoValue;
use conduit_plot::rust_binding::{nominal_representation_type, NativeRustBinding};
use conduit_time::WeekdaySet;

fn structured(value: u8) -> StructuredInfoValue {
    let semantic = WeekdaySet::semantic_type().unwrap();
    let representation = nominal_representation_type(&semantic).unwrap();
    let leaf = StructuredInfoValue::leaf(representation, vec![value]).unwrap();
    StructuredInfoValue::nominal(semantic, leaf).unwrap()
}
#[test]
fn nominal_integer_decode_preserves_constructor_range_and_refusal_identity() {
    for value in 0..=u8::MAX {
        let expected = WeekdaySet::new(value);
        assert_eq!(expected.is_ok(), (1..=127).contains(&value));
        let raw = structured(value);
        assert_eq!(WeekdaySet::from_structured(raw.clone()), expected.clone());
        assert_eq!(
            WeekdaySet::decode(&raw.canonical_bytes().unwrap()),
            expected
        );
    }
}
