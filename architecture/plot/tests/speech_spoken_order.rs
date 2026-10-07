//! Standalone Source law tests; no generated Language or Speech build required.
use conduit_core::*;
use conduit_plot::rust_binding::{validate_native_contracts, validate_native_invariants};
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};

fn admitted(token_count: u64, spoken: [bool; 4], count: u64, order: [u64; 4]) -> bool {
    let source = include_str!("../../../semantics/speech/spoken_order.conduit");
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let ty = &checked.native_types[0];
    let StructuredInfoTypeShape::Record { fields, .. } = ty.value_type.shape() else {
        panic!("Source record")
    };
    fn scalar(ty: &StructuredInfoType, bytes: Vec<u8>) -> StructuredInfoValue {
        match ty.shape() {
            StructuredInfoTypeShape::Nominal { representation, .. } => {
                StructuredInfoValue::nominal(ty.clone(), scalar(representation, bytes)).unwrap()
            }
            StructuredInfoTypeShape::Leaf { .. } => {
                StructuredInfoValue::leaf(ty.clone(), bytes).unwrap()
            }
            _ => panic!("scalar"),
        }
    }
    let values = fields
        .iter()
        .map(|field| {
            let ty = field.value_type();
            let value = match field.name() {
                "count" => scalar(ty, count.to_le_bytes().to_vec()),
                "token_count" => scalar(ty, token_count.to_le_bytes().to_vec()),
                "spoken" | "ordered" => {
                    let StructuredInfoTypeShape::Collection { element, length } = ty.shape() else {
                        panic!("collection")
                    };
                    assert_eq!(length, 4);
                    let items = (0..4)
                        .map(|i| {
                            scalar(
                                element,
                                if field.name() == "spoken" {
                                    vec![u8::from(spoken[i])]
                                } else {
                                    order[i].to_le_bytes().to_vec()
                                },
                            )
                        })
                        .collect();
                    StructuredInfoValue::collection(ty.clone(), items).unwrap()
                }
                _ => panic!("field"),
            };
            StructuredFieldValue::new(field.name(), value).unwrap()
        })
        .collect();
    let candidate = StructuredInfoValue::record(ty.value_type.clone(), values).unwrap();
    validate_native_contracts(&candidate, &ty.value_contracts).is_ok()
        && validate_native_invariants(&candidate, &ty.invariants).is_ok()
}

#[test]
fn complete_spoken_order_is_source_owned_and_cannot_omit_a_word() {
    assert!(admitted(4, [true, false, true, false], 2, [0, 2, 0, 0]));
    assert!(!admitted(4, [true, false, true, false], 2, [2, 0, 0, 0]));
    assert!(!admitted(4, [true, false, true, false], 1, [0, 0, 0, 0]));
    assert!(!admitted(4, [true, false, true, false], 2, [0, 0, 0, 0]));
    assert!(!admitted(4, [true, false, true, false], 2, [0, 1, 0, 0]));
    assert!(!admitted(4, [true, false, true, false], 2, [0, 4, 0, 0]));
    assert!(!admitted(2, [true, false, false, true], 2, [0, 3, 0, 0]));
    assert!(admitted(2, [false; 4], 0, [0; 4]));
}

#[test]
#[ignore = "requires the exact retained checked Language/Speech Source union"]
fn complete_phone_composition_source_checks_against_retained_native_types() {
    let language =
        std::fs::read_to_string(std::env::var("CONDUIT_COVERAGE_LANGUAGE_SOURCE").unwrap())
            .unwrap();
    let checked_language =
        check_syntax_document(&parse_syntax_document(&language), &StartupCatalog::new()).unwrap();
    let mut imports = StartupCatalog::new();
    for ty in &checked_language.native_types {
        imports
            .insert_structured_type(&ty.name, ty.value_type.clone())
            .unwrap();
    }
    let mut source =
        std::fs::read_to_string(std::env::var("CONDUIT_COVERAGE_SPEECH_SOURCE").unwrap()).unwrap();
    source.push_str(include_str!(
        "../../../semantics/speech/spoken_order.conduit"
    ));
    source.push_str(include_str!(
        "../../../semantics/speech/phone_layout.conduit"
    ));
    source.push_str(include_str!(
        "../../../semantics/speech/phone_composition.conduit"
    ));
    let checked = check_syntax_document(&parse_syntax_document(&source), &imports).unwrap();
    assert!(checked
        .native_types
        .iter()
        .any(|ty| ty.name == "SpeechPhoneCompositionWitness"));
}
