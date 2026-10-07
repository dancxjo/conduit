//! Constructor fallback keeps reference refusal order; prepared Play owns storage.
#[path = "prepared_structured_payload/allocation.rs"]
mod allocation;
use conduit_core::{InfoBool, StructuredFieldValue, StructuredInfoTypeShape, StructuredInfoValue};
use conduit_plot::rust_binding::{
    validate_native_invariants, NativeBindingRefusal, PreparedNativeInvariantAdmission,
};
use conduit_plot::{
    check_syntax_document, parse_syntax_document, PortableExpressionOperation,
    PortableExpressionProgram, StartupCatalog,
};

fn fixture() -> (
    conduit_core::StructuredInfoType,
    Vec<PortableExpressionProgram>,
) {
    let source =
        "type Interval = {\n start: U32\n end: U32\n where .start <= .end\n where .end < 10\n}\n";
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    (
        checked.native_types[0].value_type.clone(),
        checked.native_types[0].invariants.clone(),
    )
}
fn value(ty: &conduit_core::StructuredInfoType, start: u32, end: u32) -> StructuredInfoValue {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!()
    };
    StructuredInfoValue::record(
        ty.clone(),
        fields
            .iter()
            .map(|field| {
                let number = if field.name() == "start" { start } else { end };
                StructuredFieldValue::new(
                    field.name(),
                    conduit_plot::rust_binding::primitive_into_structured(
                        field.value_type().clone(),
                        &number,
                    )
                    .unwrap(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}
fn reference(
    value: &StructuredInfoValue,
    laws: &[PortableExpressionProgram],
) -> Result<(), NativeBindingRefusal> {
    let input = value
        .canonical_bytes()
        .map_err(NativeBindingRefusal::InvalidValue)?;
    for (index, law) in laws.iter().enumerate() {
        let result = law
            .evaluate(&input)
            .map_err(NativeBindingRefusal::InvalidInvariant)?;
        let accepted = InfoBool::decode(&result).map(InfoBool::get).map_err(|_| {
            NativeBindingRefusal::InvalidInvariant(
                conduit_plot::PortableExpressionEvaluationRefusal::InvalidProgram,
            )
        })?;
        if !accepted {
            return Err(NativeBindingRefusal::ViolatedInvariant { index });
        }
    }
    Ok(())
}
#[test]
fn malformed_later_law_does_not_replace_the_original_first_failure() {
    let (ty, mut laws) = fixture();
    laws[1].root.operation =
        PortableExpressionOperation::Literal("malformed Boolean literal".into());
    for (start, end) in [(5, 4), (4, 4)] {
        let value = value(&ty, start, end);
        assert_eq!(
            validate_native_invariants(&value, &laws),
            reference(&value, &laws)
        );
    }
    assert_eq!(
        validate_native_invariants(&value(&ty, 5, 4), &laws),
        Err(NativeBindingRefusal::ViolatedInvariant { index: 0 })
    );
    assert!(
        PreparedNativeInvariantAdmission::new(&ty, &laws, 2, 65536).is_err(),
        "Play admission refuses a complete unsupported bank"
    );
}
#[test]
fn prepared_complete_native_bank_never_allocates_during_reuse() {
    let (ty, laws) = fixture();
    let values = [value(&ty, 4, 4), value(&ty, 5, 4), value(&ty, 4, 10)];
    let inputs = values
        .iter()
        .map(|value| value.canonical_bytes().unwrap())
        .collect::<Vec<_>>();
    let expected = values
        .iter()
        .map(|value| reference(value, &laws))
        .collect::<Vec<_>>();
    let mut bank = PreparedNativeInvariantAdmission::new(&ty, &laws, 2, 65536).unwrap();
    assert_eq!(
        allocation::allocations(|| {
            for _ in 0..1000 {
                for (input, expected) in inputs.iter().zip(&expected) {
                    assert_eq!(&bank.validate(input), expected);
                }
            }
        }),
        0
    );
}
