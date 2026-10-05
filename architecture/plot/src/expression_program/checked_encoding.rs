//! Serialize the checked tree without retaining a second fully typed tree.
use super::*;

pub(crate) fn checked_canonical_hex(
    checked: &CheckedExpression,
) -> Result<String, PortableExpressionProgramRefusal> {
    let mut counted = ProgramSink {
        storage: ProgramStorage::Count,
        length: 0,
    };
    encode(checked, &mut counted)?;
    let mut encoded = ProgramSink {
        storage: ProgramStorage::Hex(String::with_capacity(counted.length * 2)),
        length: 0,
    };
    encode(checked, &mut encoded)?;
    let ProgramStorage::Hex(text) = encoded.storage else {
        unreachable!("hex storage")
    };
    Ok(text)
}

fn encode(
    checked: &CheckedExpression,
    sink: &mut ProgramSink,
) -> Result<(), PortableExpressionProgramRefusal> {
    sink.extend_from_slice(b"conduit.pure-expression.program.v2");
    for ty in [&checked.input_type, &checked.value_type] {
        push_type(
            sink,
            &ty.structured_info_type_with(&checked.semantic_structures)?,
        )?;
    }
    push_checked(sink, &checked.syntax, checked)?;
    if sink.length > MAXIMUM_PURE_EXPRESSION_PROGRAM_BYTES {
        return Err(PortableExpressionProgramRefusal::TooLarge);
    }
    Ok(())
}

fn push_checked(
    sink: &mut ProgramSink,
    syntax: &ExpressionSyntax,
    checked: &CheckedExpression,
) -> Result<(), PortableExpressionProgramRefusal> {
    let variant = {
        let ty = checked
            .node_types
            .iter()
            .find(|node| same_span(node.span, syntax.span()))
            .ok_or(PortableExpressionProgramRefusal::MissingCheckedNodeType)?
            .value_type
            .structured_info_type_with(&checked.semantic_structures)?;
        push_type(sink, &ty)?;
        matches!(
            ty.shape(),
            conduit_core::StructuredInfoTypeShape::Variant { .. }
        )
    };
    match syntax {
        ExpressionSyntax::Input(_) => sink.push(0),
        ExpressionSyntax::Atomic(value) => {
            sink.push(1);
            push_text(sink, &value.text);
        }
        ExpressionSyntax::Projection { value, member, .. }
            if variant
                && matches!(value.as_ref(), ExpressionSyntax::Atomic(_))
                && !checked
                    .node_types
                    .iter()
                    .any(|node| same_span(node.span, value.span())) =>
        {
            let ExpressionProjection::Field(case) = member else {
                return Err(PortableExpressionProgramRefusal::InvalidTupleIndex);
            };
            sink.push(1);
            push_text(sink, &case.text);
        }
        ExpressionSyntax::Projection { value, member, .. } => {
            sink.push(2);
            push_checked(sink, value, checked)?;
            match member {
                ExpressionProjection::Field(field) => {
                    sink.push(0);
                    push_text(sink, &field.text);
                }
                ExpressionProjection::TupleIndex(index) => {
                    let index: u16 = index
                        .text
                        .parse()
                        .map_err(|_| PortableExpressionProgramRefusal::InvalidTupleIndex)?;
                    sink.push(1);
                    sink.extend_from_slice(&index.to_le_bytes());
                }
            }
        }
        ExpressionSyntax::Unary {
            operator, operand, ..
        } => {
            sink.push(3);
            sink.push(unary_tag(*operator));
            push_checked(sink, operand, checked)?;
        }
        ExpressionSyntax::Binary {
            operator,
            left,
            right,
            ..
        } => {
            sink.push(4);
            sink.push(binary_tag(*operator));
            sink.push(u8::from(
                checked
                    .proven_arithmetic
                    .contains(&(syntax.span().start, syntax.span().end)),
            ));
            push_checked(sink, left, checked)?;
            push_checked(sink, right, checked)?;
        }
        ExpressionSyntax::Conditional {
            condition,
            when_true,
            when_false,
            ..
        } => {
            sink.push(5);
            for value in [condition, when_true, when_false] {
                push_checked(sink, value, checked)?;
            }
        }
        ExpressionSyntax::Tuple { values, .. } => {
            sink.push(6);
            push_values(sink, values, checked)?;
        }
        ExpressionSyntax::Record { fields, .. } => {
            sink.push(7);
            push_len(sink, fields.len());
            for field in fields {
                push_text(sink, &field.name.text);
                push_checked(sink, &field.value, checked)?;
            }
        }
        ExpressionSyntax::Collection { values, .. } => {
            sink.push(8);
            push_values(sink, values, checked)?;
        }
        ExpressionSyntax::Variant { tag, payload, .. } => {
            sink.push(9);
            push_text(sink, tag.text.rsplit('.').next().expect("checked tag"));
            push_checked(sink, payload, checked)?;
        }
        ExpressionSyntax::SemanticCall {
            kind, arguments, ..
        } => {
            sink.push(10);
            push_text(sink, &kind.text);
            push_values(sink, arguments, checked)?;
        }
    }
    Ok(())
}

fn push_values(
    sink: &mut ProgramSink,
    values: &[ExpressionSyntax],
    checked: &CheckedExpression,
) -> Result<(), PortableExpressionProgramRefusal> {
    push_len(sink, values.len());
    for value in values {
        push_checked(sink, value, checked)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::collections::{BTreeMap, BTreeSet};

    #[test]
    fn streamed_checked_programs_match_the_retained_v2_tree_encoding() {
        let input = crate::CheckedExpressionType::semantic("value/u8");
        let values = BTreeMap::new();
        let structures = BTreeMap::new();
        let numeric = BTreeSet::new();
        let kinds = BTreeMap::new();
        let context = crate::ExpressionTypeContext {
            input: &input,
            immutable_values: &values,
            structured_types: &structures,
            literal_types: &values,
            numeric_types: &numeric,
            semantic_kinds: &kinds,
        };
        for expression in [
            ".",
            ". + 1",
            "(., . * 2)",
            "{ first: ., second: . ^ 3 }",
            "[., . + 1]",
            ". == 0 ? . : . + 1",
        ] {
            let source = alloc::format!(
                "plot encoding (\n    >> value: U8\n    result: U8 >>\n) = ({expression})\n"
            );
            let syntax = crate::parse_syntax_document(&source);
            assert!(
                syntax.diagnostics.is_empty(),
                "{expression}: {:?}",
                syntax.diagnostics
            );
            let checked = crate::check_expression(
                &syntax.plots[0]
                    .expression_body
                    .as_ref()
                    .unwrap()
                    .expression
                    .syntax,
                &context,
            )
            .unwrap_or_else(|error| panic!("{expression}: {error:?}"));
            let retained = PortableExpressionProgram::from_checked(&checked).unwrap();
            let streamed = checked_canonical_hex(&checked).unwrap();
            assert_eq!(streamed, retained.canonical_hex().unwrap(), "{expression}");
            assert_eq!(
                PortableExpressionProgram::from_canonical_hex(&streamed).unwrap(),
                retained
            );
        }
    }
}
