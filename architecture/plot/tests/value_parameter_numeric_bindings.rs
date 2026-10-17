//! Execute generated bindings from the real migrated Numeric Source.
use conduit_plot::rust_binding::{generate_rust_bindings, RustBindingOptions};
#[path = "common/generated_rust.rs"]
mod generated_rust;

#[test]
fn migrated_numeric_bindings_retain_exact_resource_and_shape_admission() {
    let mut catalog = conduit_plot::StartupCatalog::new();
    catalog
        .insert_value_kind_alias(
            "ResourceRef",
            conduit_core::kind_id(conduit_core::RESOURCE_REFERENCE_INFO_ID),
        )
        .unwrap();
    let source = include_str!("../../../semantics/ai/fixed_numeric.conduit");
    let syntax = conduit_plot::parse_syntax_document(source);
    let checked = conduit_plot::check_syntax_document(&syntax, &catalog).unwrap();
    let public = syntax
        .types
        .iter()
        .filter(|ty| ty.parameters.is_empty())
        .collect::<Vec<_>>();
    assert_eq!(public.len(), 23);
    for declaration in public {
        assert!(checked
            .native_types
            .iter()
            .any(|ty| ty.name == declaration.name.text));
    }
    let generated =
        generate_rust_bindings(&checked.native_types, &RustBindingOptions::default()).unwrap();
    for native in &checked.native_types {
        assert_eq!(
            generated.semantic_type_bytes[native.identity.as_str()],
            native.value_type.canonical_bytes().unwrap()
        );
    }
    generated_rust::exercise(&generated.source, EXERCISE, 3, "numeric-value-binding");
}

const EXERCISE: &str = r#"
#[cfg(test)]
mod acceptance {
    use super::*;
    use conduit_core::{
        BoundedResourceRef, ResourceClassId, ResourceExtent, ResourceLifetime,
        ResourceSemanticIdentity, ResourceVersionIdentity, KindId, IeeeF32,
        StructuredInfoTypeShape, StructuredInfoValueShape,
    };

    // This immutable reference is encoded data, not a resolved resource or a
    // claim that a ModelArtifact has been prepared or authority granted.
    fn resource() -> BoundedResourceRef {
        BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest([11; 32]),
            content_profile: KindId::from("tensor/f32@1"),
            access_class: ResourceClassId::from("test/immutable-numeric@1"),
            extent: ResourceExtent { bytes: 4096, items: Some(1024) },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest([12; 32]),
                expires_at: None,
            },
        }
    }
    fn digest() -> NumericTensorDigest { NumericTensorDigest::new([7; 32]).unwrap() }
    fn finite() -> NumericFiniteF32 { NumericFiniteF32::new(IeeeF32::from_bits(0)).unwrap() }

    fn replace_integer(value: StructuredInfoValue, name: &str, number: u16) -> Vec<u8> {
        let semantic = value.value_type().clone();
        let StructuredInfoValueShape::Record(fields) = value.shape() else { panic!() };
        let values = fields.iter().map(|field| {
            let original = field.value();
            let next = if field.name() == name {
                StructuredInfoValue::leaf(original.value_type().clone(), number.to_le_bytes().to_vec()).unwrap()
            } else { original.clone() };
            StructuredFieldValue::new(field.name(), next).unwrap()
        }).collect();
        StructuredInfoValue::record(semantic, values).unwrap().canonical_bytes().unwrap()
    }

    #[test]
    fn every_matrix_and_bias_preserves_reference_digest_element_and_exact_dimensions() {
        macro_rules! matrix {
            ($ty:ident, $columns:literal, $rows:literal) => {{
                let value = $ty::new($columns, digest(), NumericTensorElement::f32(), resource(), $rows).unwrap();
                let encoded = value.clone().encode().unwrap();
                assert_eq!($ty::decode(&encoded).unwrap(), value);
                assert_eq!(value.resource(), &resource());
                assert_eq!(value.content_digest().clone().encode().unwrap(), digest().encode().unwrap());
                assert_eq!(value.element(), &NumericTensorElement::f32());
                assert!($ty::new($columns + 1, digest(), NumericTensorElement::f32(), resource(), $rows).is_err());
                assert!($ty::new($columns, digest(), NumericTensorElement::f32(), resource(), $rows + 1).is_err());
                assert!($ty::decode(&replace_integer(value.clone().into_structured().unwrap(), "columns", $columns + 1)).is_err());
                assert!($ty::decode(&replace_integer(value.into_structured().unwrap(), "rows", $rows + 1)).is_err());
                assert!($ty::decode(&encoded[..encoded.len() - 1]).is_err());
            }};
        }
        matrix!(NumericEmbedding224x12, 224, 12);
        matrix!(NumericF32MatrixRef32x64, 32, 64);
        matrix!(NumericF32MatrixRef192x128, 192, 128);
        matrix!(NumericF32MatrixRef128x320, 128, 320);
        matrix!(NumericF32MatrixRef3x2, 3, 2);
        macro_rules! bias {
            ($ty:ident, $length:literal) => {{
                let value = $ty::new(digest(), NumericTensorElement::f32(), $length, resource()).unwrap();
                assert_eq!($ty::decode(&value.clone().encode().unwrap()).unwrap(), value);
                assert_eq!(value.resource(), &resource());
                assert!($ty::new(digest(), NumericTensorElement::f32(), $length + 1, resource()).is_err());
                assert!($ty::decode(&replace_integer(value.into_structured().unwrap(), "length", $length + 1)).is_err());
            }};
        }
        bias!(NumericF32BiasRef64, 64);
        bias!(NumericF32BiasRef128, 128);
        bias!(NumericF32BiasRef320, 320);
        bias!(NumericF32BiasRef2, 2);
    }

    fn poison_first(value: StructuredInfoValue) -> StructuredInfoValue {
        poison_representation(value.value_type().clone(), &value)
    }

    fn poison_representation(semantic: StructuredInfoType, value: &StructuredInfoValue) -> StructuredInfoValue {
        match semantic.shape() {
            StructuredInfoTypeShape::Nominal { representation, .. } => {
                let poisoned = poison_representation(representation.clone(), value);
                StructuredInfoValue::nominal(semantic, poisoned).unwrap()
            }
            StructuredInfoTypeShape::Collection { .. } => {
                let StructuredInfoValueShape::Collection(values) = value.shape() else { panic!() };
                let mut values = values.to_vec();
                values[0] = poison_first(values[0].clone());
                StructuredInfoValue::collection(semantic, values).unwrap()
            }
            StructuredInfoTypeShape::Leaf(_) => StructuredInfoValue::leaf(semantic, IeeeF32::from_bits(0x7fc00000).encode().to_vec()).unwrap(),
            _ => panic!("unexpected finite vector shape"),
        }
    }

    #[test]
    fn all_nine_vectors_retain_finite_elements_and_nominal_decode() {
        macro_rules! vector {
            ($ty:ident, $length:literal) => {{
                let value = $ty::new(core::array::from_fn::<_, $length, _>(|_| finite())).unwrap();
                assert_eq!($ty::decode(&value.clone().encode().unwrap()).unwrap(), value);
                let poisoned = poison_first(value.into_structured().unwrap()).canonical_bytes().unwrap();
                assert!($ty::decode(&poisoned).is_err());
            }};
        }
        vector!(NumericF32Vector2, 2);
        vector!(NumericF32Vector3, 3);
        vector!(NumericF32Vector12, 12);
        vector!(NumericF32Vector20, 20);
        vector!(NumericF32Vector32, 32);
        vector!(NumericF32Vector64, 64);
        vector!(NumericF32Vector128, 128);
        vector!(NumericF32Vector192, 192);
        vector!(NumericF32Vector320, 320);
        assert!(NumericFiniteF32::new(IeeeF32::from_bits(0x7fc00000)).is_err());
        assert!(NumericFiniteF32::new(IeeeF32::from_bits(0x7f800000)).is_err());
        let foreign = NumericF32Vector128::new(core::array::from_fn(|_| finite())).unwrap().encode().unwrap();
        assert!(NumericF32Vector64::decode(&foreign).is_err());
    }

    fn zero(semantic: StructuredInfoType) -> StructuredInfoValue {
        match semantic.shape() {
            StructuredInfoTypeShape::Nominal { representation, .. } => StructuredInfoValue::nominal(semantic.clone(), zero(representation.clone())).unwrap(),
            StructuredInfoTypeShape::Collection { element, length } => {
                let values = (0..length).map(|_| zero(element.clone())).collect();
                StructuredInfoValue::collection(semantic, values).unwrap()
            }
            StructuredInfoTypeShape::Record { fields, .. } => {
                let values = fields.iter().map(|field| StructuredFieldValue::new(field.name(), zero(field.value_type().clone())).unwrap()).collect();
                StructuredInfoValue::record(semantic, values).unwrap()
            }
            StructuredInfoTypeShape::Leaf(kind) if conduit_core::primitive_info_kind(kind.as_str()) == Some(conduit_core::PrimitiveInfoKind::F32) => StructuredInfoValue::leaf(semantic, IeeeF32::from_bits(0).encode().to_vec()).unwrap(),
            _ => panic!("unexpected Numeric Window representation"),
        }
    }

    #[test]
    fn derived_window_and_history_use_the_same_generated_admission() {
        let raw = zero(NumericWindow2x64::semantic_type().unwrap());
        let value = NumericWindow2x64::decode(&raw.canonical_bytes().unwrap()).unwrap();
        let constructed = NumericWindow2x64::new(value.next_history().clone(), value.window().clone()).unwrap();
        assert_eq!(constructed, value);
        for name in ["window", "next_history"] {
            let original = value.clone().into_structured().unwrap();
            let StructuredInfoValueShape::Record(fields) = original.shape() else { panic!() };
            let values = fields.iter().map(|field| {
                let next = if field.name() == name { poison_first(field.value().clone()) } else { field.value().clone() };
                StructuredFieldValue::new(field.name(), next).unwrap()
            }).collect();
            let poisoned = StructuredInfoValue::record(original.value_type().clone(), values).unwrap().canonical_bytes().unwrap();
            assert!(NumericWindow2x64::decode(&poisoned).is_err());
        }
    }
}
"#;
