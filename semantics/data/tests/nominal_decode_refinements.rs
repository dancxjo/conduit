//! Generated nominal decoding must keep checked laws without a second encoding.
use conduit_core::{kind_id, StructuredInfoType, StructuredInfoValue};
use conduit_data::CoordinateAxisName;
use conduit_plot::rust_binding::{nominal_representation_type, NativeRustBinding};

fn encoded_text(text: &str, foreign: bool) -> StructuredInfoValue {
    let semantic = CoordinateAxisName::semantic_type().unwrap();
    let representation = nominal_representation_type(&semantic).unwrap();
    let value =
        StructuredInfoValue::leaf(representation.clone(), text.as_bytes().to_vec()).unwrap();
    let semantic = if foreign {
        StructuredInfoType::nominal(kind_id("test/foreign-axis@1"), representation).unwrap()
    } else {
        semantic
    };
    StructuredInfoValue::nominal(semantic, value).unwrap()
}
#[test]
fn nominal_decoder_keeps_exact_type_empty_and_byte_bound_refusals() {
    for (text, foreign) in [
        ("".to_string(), false),
        ("x".repeat(129), false),
        ("x".to_string(), true),
    ] {
        let value = encoded_text(&text, foreign);
        let encoded = value.canonical_bytes().unwrap();
        assert!(CoordinateAxisName::from_structured(value).is_err());
        assert!(CoordinateAxisName::decode(&encoded).is_err());
    }
    for text in ["x".to_string(), "x".repeat(128), "é".repeat(64)] {
        let value = encoded_text(&text, false);
        let encoded = value.canonical_bytes().unwrap();
        assert_eq!(CoordinateAxisName::decode(&encoded).unwrap().get(), &text);
        assert_eq!(
            CoordinateAxisName::from_structured(value).unwrap().get(),
            &text
        );
    }
}
