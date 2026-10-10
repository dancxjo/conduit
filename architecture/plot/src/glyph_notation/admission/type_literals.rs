//! Bounded traversal of literal-bearing Native Type declarations.
use crate::*;

pub(super) fn roots<'a>(
    document: &'a SyntaxDocument,
    expressions: &mut Vec<&'a ExpressionSyntax>,
    visited: &mut usize,
) -> Result<(), Span> {
    let mut types = Vec::new();
    for declaration in &document.types {
        *visited += 1;
        if *visited > 4096 {
            return Err(declaration.span);
        }
        expressions.extend(declaration.invariants.iter().map(|value| &value.syntax));
        types.extend(
            declaration
                .parameters
                .iter()
                .filter_map(|parameter| parameter.value_type.as_deref()),
        );
        match &declaration.definition {
            TypeDefinitionSyntax::Scalar(value) => types.push(value),
            TypeDefinitionSyntax::Record(fields) => {
                types.extend(fields.iter().map(|field| &field.value_type));
            }
            TypeDefinitionSyntax::Variant(cases) => {
                for case in cases {
                    match &case.payload {
                        TypeVariantPayloadSyntax::Unit => {}
                        TypeVariantPayloadSyntax::Type(value) => types.push(value),
                        TypeVariantPayloadSyntax::Record(fields) => {
                            types.extend(fields.iter().map(|field| &field.value_type));
                        }
                    }
                }
            }
        }
        if types.len() + expressions.len() > 4096 {
            return Err(declaration.span);
        }
    }
    while let Some(value) = types.pop() {
        let span = match value {
            TypeExpressionSyntax::Reference { span, .. }
            | TypeExpressionSyntax::Optional { span, .. }
            | TypeExpressionSyntax::DataReference { span, .. }
            | TypeExpressionSyntax::Collection { span, .. }
            | TypeExpressionSyntax::Sequence { span, .. } => *span,
        };
        *visited += 1;
        if *visited > 4096 {
            return Err(span);
        }
        match value {
            TypeExpressionSyntax::Reference {
                refinements,
                arguments,
                ..
            } => {
                for refinement in refinements {
                    if let ValueRefinement::TextPattern {
                        glyph: Some(value), ..
                    } = refinement
                    {
                        expressions.push(&value.syntax);
                    }
                }
                types.extend(arguments.iter().filter_map(|argument| match argument {
                    NativeTypeArgumentSyntax::Type(value) => Some(value.as_ref()),
                    NativeTypeArgumentSyntax::Value(_) => None,
                }));
            }
            TypeExpressionSyntax::Optional { value, .. }
            | TypeExpressionSyntax::DataReference { value, .. } => types.push(value),
            TypeExpressionSyntax::Collection { element, .. }
            | TypeExpressionSyntax::Sequence { element, .. } => types.push(element),
        }
        if types.len() + expressions.len() > 4096 {
            return Err(span);
        }
    }
    Ok(())
}
