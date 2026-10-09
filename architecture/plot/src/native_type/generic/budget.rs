//! Validate finite native syntax before recursive cloning or canonicalization.
use super::{binding::type_span, error};
use crate::{
    ExpressionSyntax as Law, NativeIntegerExpressionSyntax as Integer,
    NativeTypeArgumentSyntax as Argument, Span, SpannedText, SyntaxCheckDiagnostic,
    TypeDefinitionSyntax, TypeExpressionSyntax as Type, TypeFieldSyntax, TypeSyntax,
    TypeVariantPayloadSyntax, ValueRefinement,
};

pub(super) fn validate(declarations: &[TypeSyntax]) -> Result<(), SyntaxCheckDiagnostic> {
    let mut budget = Budget { nodes: 0, bytes: 0 };
    for declaration in declarations {
        budget.node(declaration.span, 0)?;
        budget.text(&declaration.name)?;
        if declaration.parameters.len() > 16 {
            return Err(error(
                declaration.name.span,
                "native Type family exceeds its 16-parameter profile".into(),
            ));
        }
        for parameter in &declaration.parameters {
            budget.text(&parameter.name)?;
            if let Some(annotation) = &parameter.value_type {
                budget.expression(annotation, 0)?;
            }
        }
        match &declaration.definition {
            TypeDefinitionSyntax::Scalar(value) => budget.expression(value, 0)?,
            TypeDefinitionSyntax::Record(fields) => budget.fields(fields)?,
            TypeDefinitionSyntax::Variant(cases) => {
                for case in cases {
                    budget.text(&case.tag)?;
                    match &case.payload {
                        TypeVariantPayloadSyntax::Unit => {}
                        TypeVariantPayloadSyntax::Type(value) => budget.expression(value, 0)?,
                        TypeVariantPayloadSyntax::Record(fields) => budget.fields(fields)?,
                    }
                }
            }
        }
        for law in &declaration.invariants {
            budget.bytes(&law.text, law.span)?;
            budget.law(&law.syntax, 0)?;
        }
    }
    Ok(())
}

struct Budget {
    nodes: usize,
    bytes: usize,
}
impl Budget {
    fn node(&mut self, span: Span, depth: usize) -> Result<(), SyntaxCheckDiagnostic> {
        self.nodes = self.nodes.saturating_add(1);
        if depth >= 32 || self.nodes > 16384 {
            return Err(error(
                span,
                "native Type syntax exceeds its finite depth/node budget".into(),
            ));
        }
        Ok(())
    }
    fn bytes(&mut self, text: &str, span: Span) -> Result<(), SyntaxCheckDiagnostic> {
        self.bytes = self.bytes.saturating_add(text.len());
        if self.bytes > 1024 * 1024 {
            return Err(error(
                span,
                "native Type syntax exceeds its finite text budget".into(),
            ));
        }
        Ok(())
    }
    fn text(&mut self, value: &SpannedText) -> Result<(), SyntaxCheckDiagnostic> {
        self.node(value.span, 0)?;
        self.bytes(&value.text, value.span)
    }
    fn fields(&mut self, fields: &[TypeFieldSyntax]) -> Result<(), SyntaxCheckDiagnostic> {
        for field in fields {
            self.text(&field.name)?;
            self.expression(&field.value_type, 0)?;
        }
        Ok(())
    }
    fn expression(&mut self, value: &Type, depth: usize) -> Result<(), SyntaxCheckDiagnostic> {
        self.node(type_span(value), depth)?;
        match value {
            Type::Reference {
                value_type,
                arguments,
                refinements,
                ..
            } => {
                self.text(value_type)?;
                if arguments.len() > 16 {
                    return Err(error(
                        type_span(value),
                        "native Type family exceeds its 16-argument profile".into(),
                    ));
                }
                for argument in arguments {
                    match argument {
                        Argument::Type(value) => self.expression(value, depth + 1)?,
                        Argument::Value(value) => self.integer(value, 0)?,
                    }
                }
                for refinement in refinements {
                    match refinement {
                        ValueRefinement::Finite { .. } => {}
                        ValueRefinement::TextPattern { source, .. } => self.text(source)?,
                        ValueRefinement::Range {
                            minimum, maximum, ..
                        } => {
                            for value in minimum.iter().chain(maximum) {
                                self.text(value)?;
                            }
                        }
                        ValueRefinement::Membership { members, .. } => {
                            for value in members {
                                self.text(value)?;
                            }
                        }
                    }
                }
            }
            Type::Optional { value, .. } | Type::DataReference { value, .. } => {
                self.expression(value, depth + 1)?
            }
            Type::Collection {
                element, length, ..
            } => {
                self.expression(element, depth + 1)?;
                self.integer(length, 0)?;
            }
            Type::Sequence {
                element,
                minimum_items,
                maximum_items,
                ..
            } => {
                self.expression(element, depth + 1)?;
                self.integer(minimum_items, 0)?;
                self.integer(maximum_items, 0)?;
            }
        }
        Ok(())
    }
    fn integer(&mut self, value: &Integer, depth: usize) -> Result<(), SyntaxCheckDiagnostic> {
        if depth >= 64 {
            return Err(error(
                value.span(),
                "native integer syntax exceeds its finite depth budget".into(),
            ));
        }
        self.node(value.span(), 0)?;
        match value {
            Integer::Literal { .. } => {}
            Integer::Parameter(name) => self.text(name)?,
            Integer::Group { value, .. } => self.integer(value, depth + 1)?,
            Integer::Binary { left, right, .. } => {
                self.integer(left, depth + 1)?;
                self.integer(right, depth + 1)?;
            }
        }
        Ok(())
    }
    fn law(&mut self, value: &Law, depth: usize) -> Result<(), SyntaxCheckDiagnostic> {
        self.node(value.span(), depth)?;
        match value {
            Law::Atomic(value) => self.text(value)?,
            Law::Input(_) => {}
            Law::Projection { value, member, .. } => {
                self.law(value, depth + 1)?;
                match member {
                    crate::ExpressionProjection::Field(name)
                    | crate::ExpressionProjection::TupleIndex(name) => self.text(name)?,
                }
            }
            Law::Unary { operand, .. } => self.law(operand, depth + 1)?,
            Law::Binary { left, right, .. } => {
                self.law(left, depth + 1)?;
                self.law(right, depth + 1)?;
            }
            Law::Conditional {
                condition,
                when_true,
                when_false,
                ..
            } => {
                self.law(condition, depth + 1)?;
                self.law(when_true, depth + 1)?;
                self.law(when_false, depth + 1)?;
            }
            Law::Tuple { values, .. } | Law::Collection { values, .. } => {
                for value in values {
                    self.law(value, depth + 1)?;
                }
            }
            Law::Record { fields, .. } => {
                for field in fields {
                    self.text(&field.name)?;
                    self.law(&field.value, depth + 1)?;
                }
            }
            Law::Variant { tag, payload, .. } => {
                self.text(tag)?;
                self.law(payload, depth + 1)?;
            }
            Law::SemanticCall {
                kind, arguments, ..
            } => {
                self.text(kind)?;
                for value in arguments {
                    self.law(value, depth + 1)?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;

    #[test]
    fn preflight_refuses_deep_public_ast_before_cloning() {
        let mut parsed = crate::parse_syntax_document("type Value = U16\n");
        let TypeDefinitionSyntax::Scalar(value) = &mut parsed.types[0].definition else {
            panic!("scalar expected")
        };
        let span = type_span(value);
        for _ in 0..40 {
            *value = Type::Optional {
                value: Box::new(value.clone()),
                span,
            };
        }
        let failure = validate(&parsed.types).unwrap_err();
        assert!(failure.message.contains("depth/node budget"));
    }

    #[test]
    fn source_parser_refuses_deep_type_nesting_before_specialization() {
        let source = alloc::format!("type Value = U16{}\n", "?".repeat(40));
        let accepted = crate::parse_syntax_document("type Value = U16???\n");
        assert!(accepted.diagnostics.is_empty());
        let parsed = crate::parse_syntax_document(&source);
        assert!(!parsed.diagnostics.is_empty());
        let application = alloc::format!(
            "type Value = {}U16{}\n",
            "Family<".repeat(40),
            ">".repeat(40)
        );
        assert!(!crate::parse_syntax_document(&application)
            .diagnostics
            .is_empty());
    }

    #[test]
    fn empty_public_names_still_consume_the_node_budget() {
        let mut parsed = crate::parse_syntax_document("type Value = U16\n");
        let mut declaration = parsed.types.remove(0);
        declaration.name.text.clear();
        let declarations = alloc::vec![declaration; 16385];
        assert!(validate(&declarations)
            .unwrap_err()
            .message
            .contains("node budget"));
    }

    #[test]
    fn preflight_preserves_the_integer_expression_profile() {
        let expression = alloc::vec!["1"; 32].join(" + ");
        let parsed = crate::parse_syntax_document(&alloc::format!(
            "type Value = collection U8 = {expression}\n"
        ));
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        validate(&parsed.types).unwrap();
        crate::check_syntax_document(&parsed, &crate::StartupCatalog::new()).unwrap();
    }
}
