//! Observing nominal collections retains exact input and selected element meaning.
#[path = "prepared_structured_payload/allocation.rs"]
mod allocation;
use conduit_core::{
    ConfigurationValue, StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape as Shape,
    StructuredInfoValue,
};
use conduit_plot::*;
const SOURCE: &str = "type Item = {\n number: U64\n}\ntype OtherItem = {\n number: U64\n}\n\
type Vector<N: U16> = collection Item = N\ntype Tier<N: U16> = sequence Item <= N\n\
type Fixed = Vector<2>\ntype Variable = Tier<4>\n\
type Request = {\n fixed: Fixed\n variable: Variable\n index: U64\n}\n\
type ForeignRequest = {\n fixed: Fixed\n variable: Variable\n index: U64\n}\n\
plot length (\n value: Request >> result: U64\n) = (sequence/length(.variable))\n\
plot select (\n value: Request >> result: Item\n) = (sequence/at(.variable, .index))\n\
plot exact (\n value: Request >> result: Item\n) = (sequence/at(.fixed, .index))\n";
fn checked() -> CheckedSyntaxDocument {
    check_syntax_document(&parse_syntax_document(SOURCE), &StartupCatalog::new()).unwrap()
}
fn program(entry: &str) -> PortableExpressionProgram {
    let expanded = expand_canonical_plot_for_authoring(&checked(), entry, &ProfileCatalog::new())
        .unwrap()
        .expanded;
    let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        panic!()
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
fn field_type(ty: &StructuredInfoType, name: &str) -> StructuredInfoType {
    let Shape::Record { fields, .. } = ty.shape() else {
        panic!()
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value_type()
        .clone()
}
fn items(ty: StructuredInfoType, count: usize) -> StructuredInfoValue {
    match ty.shape() {
        Shape::Nominal { representation, .. } => {
            let inner = items(representation.clone(), count);
            StructuredInfoValue::nominal(ty, inner).unwrap()
        }
        Shape::Collection { element, .. } | Shape::Sequence { element, .. } => {
            let values = (0..count)
                .map(|index| {
                    StructuredInfoValue::record(
                        element.clone(),
                        vec![StructuredFieldValue::new(
                            "number",
                            StructuredInfoValue::leaf(
                                field_type(element, "number"),
                                (index as u64 + 10).to_le_bytes().to_vec(),
                            )
                            .unwrap(),
                        )
                        .unwrap()],
                    )
                    .unwrap()
                })
                .collect();
            if matches!(ty.shape(), Shape::Collection { .. }) {
                StructuredInfoValue::collection(ty, values).unwrap()
            } else {
                StructuredInfoValue::sequence(ty, values).unwrap()
            }
        }
        _ => panic!(),
    }
}
fn input(ty: StructuredInfoType, count: usize, index: u64) -> Vec<u8> {
    let values = vec![
        StructuredFieldValue::new("fixed", items(field_type(&ty, "fixed"), 2)).unwrap(),
        StructuredFieldValue::new("variable", items(field_type(&ty, "variable"), count)).unwrap(),
        StructuredFieldValue::new(
            "index",
            StructuredInfoValue::leaf(field_type(&ty, "index"), index.to_le_bytes().to_vec())
                .unwrap(),
        )
        .unwrap(),
    ];
    StructuredInfoValue::record(ty, values)
        .unwrap()
        .canonical_bytes()
        .unwrap()
}
#[test]
fn actual_nominal_counts_and_selected_elements_match_reference_without_runtime_growth() {
    for entry in ["length", "select", "exact"] {
        let program = program(entry);
        let encoded = input(program.input_type.clone(), 2, 1);
        let expected = program.evaluate(&encoded).unwrap();
        let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
        assert_eq!(
            allocation::allocations(|| {
                for _ in 0..1000 {
                    assert_eq!(prepared.evaluate(&encoded).unwrap(), expected);
                }
            }),
            0
        );
        if entry == "length" {
            assert_eq!(expected, 2_u64.to_le_bytes());
        } else {
            let selected = StructuredInfoValue::from_canonical_bytes(&expected).unwrap();
            assert_eq!(selected.value_type(), &program.output_type);
        }
    }
    let program = program("length");
    let empty = input(program.input_type.clone(), 0, 0);
    assert_eq!(program.evaluate(&empty).unwrap(), 0_u64.to_le_bytes());
    assert_eq!(
        PreparedPortableExpressionEvaluator::new(&program)
            .unwrap()
            .evaluate(&empty)
            .unwrap(),
        0_u64.to_le_bytes()
    );
}
#[test]
fn actual_bounds_foreign_input_and_forged_element_identity_refuse() {
    let checked = checked();
    let foreign = checked
        .native_types
        .iter()
        .find(|ty| ty.name == "ForeignRequest")
        .unwrap();
    let other_item = checked
        .native_types
        .iter()
        .find(|ty| ty.name == "OtherItem")
        .unwrap();
    for entry in ["select", "exact"] {
        let mut program = program(entry);
        let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
        let short = input(program.input_type.clone(), 2, 2);
        assert!(program.evaluate(&short).is_err());
        assert!(prepared.evaluate(&short).is_err());
        let foreign = input(foreign.value_type.clone(), 2, 0);
        assert!(program.evaluate(&foreign).is_err());
        assert!(prepared.evaluate(&foreign).is_err());
        program.output_type = other_item.value_type.clone();
        program.root.value_type = other_item.value_type.clone();
        assert!(PreparedPortableExpressionEvaluator::new(&program).is_err());
    }
}
