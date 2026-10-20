use super::*;
use alloc::collections::{BTreeMap, BTreeSet};

fn checked() -> CheckedExpression {
    let syntax = crate::parse_syntax_document(
        "plot length (\n wire: Bytes >> size: U64\n) = (bytes/length(.))\n",
    );
    let input = crate::CheckedExpressionType::semantic("value/bytes");
    let values = BTreeMap::new();
    let types = BTreeMap::new();
    let numeric = BTreeSet::new();
    let kinds = BTreeMap::new();
    crate::check_expression(
        &syntax.plots[0]
            .expression_body
            .as_ref()
            .unwrap()
            .expression
            .syntax,
        &crate::ExpressionTypeContext {
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

#[test]
fn checked_and_retained_encoders_agree_for_both_versions_and_invalid_bounds() {
    let mut checked = checked();
    for bound in [None, Some(2048)] {
        checked.input_maximum_bytes = bound;
        let program = PortableExpressionProgram::from_checked(&checked).unwrap();
        let streamed = checked_encoding::checked_canonical_hex(&checked).unwrap();
        assert_eq!(streamed, program.canonical_hex().unwrap());
        assert_eq!(
            PortableExpressionProgram::from_canonical_hex(&streamed).unwrap(),
            program
        );
        assert!(program
            .canonical_bytes()
            .unwrap()
            .starts_with(if bound.is_some() { V3 } else { V2 }));
    }
    let unbounded = conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32;
    for maximum in [unbounded + 1, u32::MAX] {
        checked.input_maximum_bytes = Some(maximum);
        assert!(PortableExpressionProgram::from_checked(&checked).is_err());
        assert!(checked_encoding::checked_canonical_hex(&checked).is_err());
        assert!(
            crate::pure_expression_definition(&checked, conduit_core::PortTemporal::Value).is_err()
        );
    }
}

#[test]
fn bounded_decoder_rejects_truncation_unknown_version_and_trailing_data() {
    let mut checked = checked();
    checked.input_maximum_bytes = Some(2048);
    let bytes = PortableExpressionProgram::from_checked(&checked)
        .unwrap()
        .canonical_bytes()
        .unwrap();
    for end in [V3.len() - 1, V3.len(), V3.len() + 3, bytes.len() - 1] {
        assert!(PortableExpressionProgram::from_canonical_bytes(&bytes[..end]).is_err());
    }
    let mut unknown = bytes.clone();
    unknown[V3.len() - 1] = b'4';
    assert!(PortableExpressionProgram::from_canonical_bytes(&unknown).is_err());
    let mut trailing = bytes;
    trailing.push(0);
    assert!(PortableExpressionProgram::from_canonical_bytes(&trailing).is_err());
}

#[test]
fn legacy_identity_program_bytes_do_not_gain_empty_predicate_envelopes() {
    // Frozen pre-qualification v2/v3 vectors for the same U64 identity.
    let ty = conduit_core::StructuredInfoType::leaf(conduit_core::kind_id("value/u64")).unwrap();
    for (bound, expected) in [
        (None, "636f6e647569742e707572652d65787072657373696f6e2e70726f6772616d2e76320e00000000000000000900000076616c75652f7536340e00000000000000000900000076616c75652f7536340e00000000000000000900000076616c75652f75363400"),
        (Some(8), "636f6e647569742e707572652d65787072657373696f6e2e70726f6772616d2e7633080000000e00000000000000000900000076616c75652f7536340e00000000000000000900000076616c75652f7536340e00000000000000000900000076616c75652f75363400"),
    ] {
        let program = PortableExpressionProgram {
            input_type: ty.clone(), input_maximum_bytes: bound, output_type: ty.clone(),
            root: PortableExpressionNode { value_type: ty.clone(), operation: PortableExpressionOperation::Input },
        };
        assert_eq!(program.canonical_hex().unwrap(), expected);
        assert_eq!(PortableExpressionProgram::from_canonical_hex(expected).unwrap(), program);
    }
}
