//! Exact native member laws survive typed joins without shape-based inference.
use conduit_core::*;
use conduit_plot::*;

const TYPES: &str =
    "type Strong = {\n address: U8 in 8..=119\n}\ntype Weak = {\n address: U8 in 0..=255\n}\n";

fn joined_catalog(types: &str, member: &str) -> StartupCatalog {
    let checked =
        check_syntax_document(&parse_syntax_document(types), &StartupCatalog::new()).unwrap();
    let ty = checked
        .native_types
        .iter()
        .find(|ty| ty.name == member)
        .unwrap()
        .value_type
        .clone();
    let pair = tuple_info_type(vec![
        ty,
        StructuredInfoType::leaf(kind_id("value/u64")).unwrap(),
    ])
    .unwrap();
    let mut startup = StartupCatalog::new();
    startup.insert_structured_type("Joined", pair).unwrap();
    startup
}

fn expand(
    types: &str,
    startup: &StartupCatalog,
    output: &str,
    expression: &str,
) -> Result<ExpandedAuthoringPlot, CanonicalExpansionDiagnostic> {
    let source =
        format!("{types}\nplot choose (\n input: Joined >> output: {output}\n) = ({expression})\n");
    let checked = check_syntax_document(&parse_syntax_document(&source), startup).unwrap();
    expand_canonical_plot_for_authoring(&checked, "choose", &ProfileCatalog::new())
}

#[test]
fn exact_native_refinements_forward_through_tuple_projection() {
    let startup = joined_catalog(TYPES, "Strong");
    let expanded = expand(TYPES, &startup, "Strong", "{address: .0.address}").unwrap();
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("expression");
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    let StructuredInfoTypeShape::Record { fields, .. } = program.input_type.shape() else {
        panic!("tuple");
    };
    let member = fields[0].value_type().clone();
    let StructuredInfoTypeShape::Record { fields, .. } = member.shape() else {
        panic!("native record");
    };
    let value = StructuredInfoValue::record(
        member.clone(),
        vec![StructuredFieldValue::new(
            "address",
            StructuredInfoValue::leaf(fields[0].value_type().clone(), vec![0x76]).unwrap(),
        )
        .unwrap()],
    )
    .unwrap();
    let mut pair = PreparedTypedTuplePairEncoder::new(
        member,
        value.canonical_bytes().unwrap().len() as u32,
        StructuredInfoType::leaf(kind_id("value/u64")).unwrap(),
        8,
    )
    .unwrap();
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let input = value.canonical_bytes().unwrap();
    assert_eq!(
        evaluator
            .evaluate(pair.encode(&input, &0_u64.to_le_bytes()).unwrap())
            .unwrap(),
        input
    );
}

#[test]
fn weaker_member_contract_cannot_authorize_a_stronger_constructed_value() {
    let startup = joined_catalog(TYPES, "Weak");
    assert!(expand(TYPES, &startup, "Strong", "{address: .0.address}")
        .unwrap_err()
        .message
        .contains("law validator"));
}

#[test]
fn imported_member_shape_without_retained_laws_cannot_authorize_construction() {
    let native =
        check_syntax_document(&parse_syntax_document(TYPES), &StartupCatalog::new()).unwrap();
    let strong = native
        .native_types
        .iter()
        .find(|ty| ty.name == "Strong")
        .unwrap();
    let mut startup = joined_catalog(TYPES, "Strong");
    startup
        .insert_structured_type("External", strong.value_type.clone())
        .unwrap();
    assert!(expand("", &startup, "External", "{address: .0.address}")
        .unwrap_err()
        .message
        .contains("law validator"));
}

#[test]
fn cross_field_laws_still_require_a_validator_when_reconstructing_a_tuple_member() {
    let types = "type Strong = {\n address: U8\n where .address >= 8\n}\n";
    let startup = joined_catalog(types, "Strong");
    assert!(expand(types, &startup, "Strong", "{address: .0.address}")
        .unwrap_err()
        .message
        .contains("law validator"));
    assert!(expand(types, &startup, "Strong", ".0").is_ok());
}
