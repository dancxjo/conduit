//! Version4 is a canonical metadata encoding, not a change to the typed AST.
use conduit_core::{
    kind_id, ConfigurationValue, PortTemporal, StructuredFieldType, StructuredFieldValue,
    StructuredInfoType, StructuredInfoValue,
};
use conduit_plot::*;
use std::collections::{BTreeMap, BTreeSet};
const V4: &[u8] = b"conduit.pure-expression.program.v4";
fn definition(bytes: &mut Vec<u8>, schema: &[u8]) {
    bytes.push(0);
    bytes.extend_from_slice(&(schema.len() as u64).to_le_bytes());
    bytes.extend_from_slice(schema);
}
fn reference(bytes: &mut Vec<u8>, index: u16) {
    bytes.push(1);
    bytes.extend_from_slice(&index.to_le_bytes());
}
fn identity(ty: StructuredInfoType, bound: Option<u32>) -> PortableExpressionProgram {
    PortableExpressionProgram {
        input_type: ty.clone(),
        input_maximum_bytes: bound,
        output_type: ty.clone(),
        root: PortableExpressionNode {
            value_type: ty,
            operation: PortableExpressionOperation::Input,
        },
    }
}
fn identity_wire(schema: &[u8], bound: Option<u32>) -> Vec<u8> {
    let mut bytes = V4.to_vec();
    bytes.push(u8::from(bound.is_some()));
    if let Some(bound) = bound {
        bytes.extend_from_slice(&bound.to_le_bytes());
    }
    definition(&mut bytes, schema);
    reference(&mut bytes, 0);
    reference(&mut bytes, 0);
    bytes.push(0);
    bytes
}
#[test]
fn first_seen_table_and_optional_input_bound_are_exact_and_deterministic() {
    let ty = StructuredInfoType::leaf(kind_id("value/u64")).unwrap();
    let schema = ty.canonical_bytes().unwrap();
    for bound in [None, Some(8)] {
        let program = identity(ty.clone(), bound);
        let bytes = program.canonical_bytes().unwrap();
        assert_eq!(bytes, identity_wire(&schema, bound));
        assert_eq!(program.canonical_bytes().unwrap(), bytes);
        let decoded = PortableExpressionProgram::from_canonical_bytes(&bytes).unwrap();
        assert_eq!(decoded, program);
        assert_eq!(decoded.input_maximum_bytes, bound);
        assert_eq!(
            decoded.evaluate(&17u64.to_le_bytes()).unwrap(),
            17u64.to_le_bytes()
        );
    }
}
fn checked() -> CheckedExpression {
    let syntax =
        parse_syntax_document("plot arithmetic (\n input: U64 >> output: U64\n) = ((. + 1) * 2)\n");
    assert!(syntax.diagnostics.is_empty());
    let input = CheckedExpressionType::semantic("value/u64");
    let values = BTreeMap::new();
    let types = BTreeMap::new();
    let numeric = BTreeSet::new();
    let kinds = BTreeMap::new();
    check_expression(
        &syntax.plots[0]
            .expression_body
            .as_ref()
            .unwrap()
            .expression
            .syntax,
        &ExpressionTypeContext {
            glyph_values: None,
            input: &input,
            immutable_values: &values,
            structured_types: &types,
            literal_types: &values,
            numeric_types: &numeric,
            semantic_kinds: &kinds,
        },
    )
    .unwrap()
}
fn arithmetic_equivalence(program: &PortableExpressionProgram) {
    let mut prepared = PreparedPortableExpressionEvaluator::new(program).unwrap();
    for input in [0u64, 1, 17, u64::MAX / 2 - 1] {
        let expected = ((input + 1) * 2).to_le_bytes();
        assert_eq!(program.evaluate(&input.to_le_bytes()).unwrap(), expected);
        assert_eq!(prepared.evaluate(&input.to_le_bytes()).unwrap(), expected);
    }
    assert!(program.evaluate(&u64::MAX.to_le_bytes()).is_err());
    assert!(prepared.evaluate(&u64::MAX.to_le_bytes()).is_err());
}
#[test]
fn checked_streaming_and_owned_encoding_have_identical_ast_bytes_and_semantics() {
    let checked = checked();
    let owned = PortableExpressionProgram::from_checked(&checked).unwrap();
    let streamed = pure_expression_definition(&checked, PortTemporal::Value).unwrap();
    let ConfigurationValue::Text(hex) = &streamed.configuration[0].default_value else {
        panic!("program");
    };
    assert_eq!(hex, &owned.canonical_hex().unwrap());
    let decoded = PortableExpressionProgram::from_canonical_hex(hex).unwrap();
    assert_eq!(decoded, owned);
    let projected = portable_expression_definition(&decoded, PortTemporal::Value).unwrap();
    assert_eq!(projected, streamed);
    arithmetic_equivalence(&decoded);
}
#[test]
fn authored_bound_survives_checked_expansion_and_owned_reencoding() {
    let syntax = parse_syntax_document(
        "plot arithmetic (\n input: U64 <= 8B >> output: U64\n) = ((. + 1) * 2)\n",
    );
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "arithmetic", &ProfileCatalog::new())
            .unwrap();
    let gears = &expanded.expanded.gears;
    assert_eq!(gears.len(), 1);
    let ConfigurationValue::Text(hex) = &gears[0]
        .configuration
        .iter()
        .find(|entry| entry.key == "program")
        .unwrap()
        .value
    else {
        panic!("program");
    };
    let owned = PortableExpressionProgram::from_canonical_hex(hex).unwrap();
    assert_eq!(owned.input_maximum_bytes, Some(8));
    assert_eq!(owned.canonical_hex().unwrap(), *hex);
    assert!(owned.canonical_bytes().unwrap().starts_with(V4));
    arithmetic_equivalence(&owned);
    assert!(owned.evaluate(&[0; 9]).is_err());
}
fn record_type() -> StructuredInfoType {
    let leaf = StructuredInfoType::leaf(kind_id("value/u64")).unwrap();
    StructuredInfoType::record(
        kind_id("fixture/repeated-record"),
        (0..64)
            .map(|i| StructuredFieldType::new(format!("channel_{i:03}"), leaf.clone()).unwrap())
            .collect(),
    )
    .unwrap()
}
fn conditional(ty: &StructuredInfoType, depth: u8) -> PortableExpressionNode {
    let operation = if depth == 0 {
        PortableExpressionOperation::Input
    } else {
        PortableExpressionOperation::Conditional {
            condition: Box::new(PortableExpressionNode {
                value_type: StructuredInfoType::leaf(kind_id("value/bool")).unwrap(),
                operation: PortableExpressionOperation::Literal("true".into()),
            }),
            when_true: Box::new(conditional(ty, depth - 1)),
            when_false: Box::new(conditional(ty, depth - 1)),
        }
    };
    PortableExpressionNode {
        value_type: ty.clone(),
        operation,
    }
}
#[test]
fn repeated_large_record_is_interned_without_losing_types_or_evaluation() {
    let ty = record_type();
    let schema = ty.canonical_bytes().unwrap();
    let program = PortableExpressionProgram {
        input_type: ty.clone(),
        input_maximum_bytes: None,
        output_type: ty.clone(),
        root: conditional(&ty, 6),
    };
    let bytes = program.canonical_bytes().unwrap();
    //127 AST result types plus input/output repeated this exact record in v2.
    let legacy_type_bytes = 129 * schema.len();
    assert!(
        bytes.len() * 8 < legacy_type_bytes,
        "{} vs{legacy_type_bytes}",
        bytes.len()
    );
    assert_eq!(
        bytes
            .windows(schema.len())
            .filter(|window| *window == schema)
            .count(),
        1
    );
    let decoded = PortableExpressionProgram::from_canonical_bytes(&bytes).unwrap();
    assert_eq!(decoded, program);
    let leaf = StructuredInfoType::leaf(kind_id("value/u64")).unwrap();
    let input = StructuredInfoValue::record(
        ty,
        (0..64)
            .map(|i| {
                StructuredFieldValue::new(
                    format!("channel_{i:03}"),
                    StructuredInfoValue::leaf(leaf.clone(), (i as u64).to_le_bytes().to_vec())
                        .unwrap(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    assert_eq!(decoded.evaluate(&input).unwrap(), input);
    assert_eq!(
        PreparedPortableExpressionEvaluator::new(&decoded)
            .unwrap()
            .evaluate(&input)
            .unwrap(),
        input
    );
}
#[test]
fn malformed_interning_is_refused_instead_of_repaired() {
    let ty = StructuredInfoType::leaf(kind_id("value/u64")).unwrap();
    let schema = ty.canonical_bytes().unwrap();
    let valid = identity_wire(&schema, None);
    let output = V4.len() + 1 + 1 + 8 + schema.len();
    let root = output + 3;
    let mut cases = Vec::new();
    // No definition exists yet: reference0 is a forward reference, not a seed.
    let mut forward = V4.to_vec();
    forward.push(0);
    reference(&mut forward, 0);
    reference(&mut forward, 0);
    reference(&mut forward, 0);
    forward.push(0);
    cases.push(forward);
    for (offset, index) in [(output, 1u16), (root, u16::MAX)] {
        let mut bytes = valid.clone();
        bytes[offset + 1..offset + 3].copy_from_slice(&index.to_le_bytes());
        cases.push(bytes);
    }
    // A previously defined schema must be referenced, never defined twice.
    let mut duplicate = valid[..output].to_vec();
    definition(&mut duplicate, &schema);
    reference(&mut duplicate, 0);
    duplicate.push(0);
    cases.push(duplicate);
    let mut flags = valid.clone();
    flags[V4.len()] = 2;
    cases.push(flags);
    let mut tag = valid.clone();
    tag[output] = 2;
    cases.push(tag);
    let mut trailing = valid.clone();
    trailing.push(0);
    cases.push(trailing);
    let mut wrong_output = valid[..output].to_vec();
    definition(
        &mut wrong_output,
        &StructuredInfoType::leaf(kind_id("value/bool"))
            .unwrap()
            .canonical_bytes()
            .unwrap(),
    );
    reference(&mut wrong_output, 0);
    wrong_output.push(0);
    cases.push(wrong_output);
    for (index, bytes) in cases.iter().enumerate() {
        assert!(
            PortableExpressionProgram::from_canonical_bytes(bytes).is_err(),
            "accepted malformed case{index}"
        );
    }
    for end in [
        0,
        V4.len(),
        V4.len() + 1,
        output + 1,
        output + 2,
        valid.len() - 1,
    ] {
        assert!(PortableExpressionProgram::from_canonical_bytes(&valid[..end]).is_err());
    }
}
#[test]
fn frozen_v2_and_v3_decode_without_being_misvalidated_as_v4() {
    let ty = StructuredInfoType::leaf(kind_id("value/u64")).unwrap();
    for (bound,hex) in [
        (None,"636f6e647569742e707572652d65787072657373696f6e2e70726f6772616d2e76320e00000000000000000900000076616c75652f7536340e00000000000000000900000076616c75652f7536340e00000000000000000900000076616c75652f75363400"),
        (Some(8),"636f6e647569742e707572652d65787072657373696f6e2e70726f6772616d2e7633080000000e00000000000000000900000076616c75652f7536340e00000000000000000900000076616c75652f7536340e00000000000000000900000076616c75652f75363400"),
    ] {
        let decoded=PortableExpressionProgram::from_canonical_hex(hex).unwrap();
        assert_eq!(decoded,identity(ty.clone(),bound));
        assert!(decoded.canonical_bytes().unwrap().starts_with(V4));
        assert_eq!(decoded.evaluate(&23u64.to_le_bytes()).unwrap(),23u64.to_le_bytes());
        let mut trailing=hex.to_string();trailing.push_str("00");
        assert!(PortableExpressionProgram::from_canonical_hex(&trailing).is_err());
    }
}
fn expansion_type() -> StructuredInfoType {
    let leaf = StructuredInfoType::leaf(kind_id(&"L".repeat(96))).unwrap();
    let inner = StructuredInfoType::record(
        kind_id("fixture/large-member"),
        (0..16)
            .map(|i| {
                StructuredFieldType::new(format!("{i:03}{}", "x".repeat(93)), leaf.clone()).unwrap()
            })
            .collect(),
    )
    .unwrap();
    StructuredInfoType::record(
        kind_id("fixture/expansion-budget"),
        (0..64)
            .map(|i| {
                StructuredFieldType::new(format!("{i:03}{}", "y".repeat(93)), inner.clone())
                    .unwrap()
            })
            .collect(),
    )
    .unwrap()
}
fn wire_branch(bytes: &mut Vec<u8>, depth: u8, boolean: &[u8], defined: &mut bool) {
    reference(bytes, 0);
    if depth == 0 {
        bytes.push(0);
        return;
    }
    bytes.push(5); // Conditional, followed by a bool literal and two branches.
    if *defined {
        reference(bytes, 1);
    } else {
        definition(bytes, boolean);
        *defined = true;
    }
    bytes.push(1);
    bytes.extend_from_slice(&4u64.to_le_bytes());
    bytes.extend_from_slice(b"true");
    wire_branch(bytes, depth - 1, boolean, defined);
    wire_branch(bytes, depth - 1, boolean, defined);
}
#[test]
fn small_interned_input_cannot_expand_past_the_original_type_budget() {
    let schema = expansion_type().canonical_bytes().unwrap();
    let expanded_type_bytes = (511 + 2) * schema.len();
    assert!(expanded_type_bytes > MAXIMUM_PURE_EXPRESSION_PROGRAM_BYTES);
    let mut wire = V4.to_vec();
    wire.push(0);
    definition(&mut wire, &schema);
    reference(&mut wire, 0);
    let boolean = StructuredInfoType::leaf(kind_id("value/bool"))
        .unwrap()
        .canonical_bytes()
        .unwrap();
    wire_branch(&mut wire, 8, &boolean, &mut false);
    assert!(wire.len() < MAXIMUM_PURE_EXPRESSION_PROGRAM_BYTES / 32);
    assert!(matches!(
        PortableExpressionProgram::from_canonical_bytes(&wire),
        Err(PortableExpressionProgramRefusal::TooLarge)
    ));
}
