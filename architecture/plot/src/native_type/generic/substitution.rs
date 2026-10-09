//! Recursive substitution of finite native Type expressions.
use super::canonical::{application_key, instantiated_name};
use super::{error, Context, MAXIMUM_GENERIC_INSTANTIATION_DEPTH};
use crate::prelude::*;
use crate::{
    SpannedText, SyntaxCheckDiagnostic, TypeDefinitionSyntax, TypeExpressionSyntax,
    TypeFieldSyntax, TypeSyntax, TypeVariantCaseSyntax, TypeVariantPayloadSyntax,
};
use alloc::collections::BTreeMap;

impl Context<'_> {
    pub(super) fn definition(
        &mut self,
        definition: &TypeDefinitionSyntax,
        arguments: &BTreeMap<String, TypeExpressionSyntax>,
    ) -> Result<TypeDefinitionSyntax, SyntaxCheckDiagnostic> {
        Ok(match definition {
            TypeDefinitionSyntax::Scalar(expression) => {
                TypeDefinitionSyntax::Scalar(self.expression(expression, arguments)?)
            }
            TypeDefinitionSyntax::Record(fields) => TypeDefinitionSyntax::Record(
                fields
                    .iter()
                    .map(|field| self.field(field, arguments))
                    .collect::<Result<_, _>>()?,
            ),
            TypeDefinitionSyntax::Variant(cases) => TypeDefinitionSyntax::Variant(
                cases
                    .iter()
                    .map(|case| {
                        Ok(TypeVariantCaseSyntax {
                            tag: case.tag.clone(),
                            payload: match &case.payload {
                                TypeVariantPayloadSyntax::Unit => TypeVariantPayloadSyntax::Unit,
                                TypeVariantPayloadSyntax::Type(value) => {
                                    TypeVariantPayloadSyntax::Type(
                                        self.expression(value, arguments)?,
                                    )
                                }
                                TypeVariantPayloadSyntax::Record(fields) => {
                                    TypeVariantPayloadSyntax::Record(
                                        fields
                                            .iter()
                                            .map(|field| self.field(field, arguments))
                                            .collect::<Result<_, _>>()?,
                                    )
                                }
                            },
                            span: case.span,
                        })
                    })
                    .collect::<Result<_, SyntaxCheckDiagnostic>>()?,
            ),
        })
    }

    fn field(
        &mut self,
        field: &TypeFieldSyntax,
        arguments: &BTreeMap<String, TypeExpressionSyntax>,
    ) -> Result<TypeFieldSyntax, SyntaxCheckDiagnostic> {
        Ok(TypeFieldSyntax {
            name: field.name.clone(),
            value_type: self.expression(&field.value_type, arguments)?,
            span: field.span,
        })
    }

    pub(super) fn expression(
        &mut self,
        expression: &TypeExpressionSyntax,
        substitutions: &BTreeMap<String, TypeExpressionSyntax>,
    ) -> Result<TypeExpressionSyntax, SyntaxCheckDiagnostic> {
        match expression {
            TypeExpressionSyntax::Reference {
                value_type,
                arguments,
                maximum_bytes,
                refinements,
                span,
            } => {
                if arguments.is_empty() {
                    if let Some(substitution) = substitutions.get(&value_type.text) {
                        if maximum_bytes.is_some() || !refinements.is_empty() {
                            return Err(error(
                                *span,
                                "a generic Type parameter cannot be directly refined".into(),
                            ));
                        }
                        return Ok(substitution.clone());
                    }
                    if self.generics.contains_key(value_type.text.as_str()) {
                        return Err(error(
                            *span,
                            alloc::format!(
                                "generic semantic Type '{}' requires arguments",
                                value_type.text
                            ),
                        ));
                    }
                    return Ok(expression.clone());
                }
                if maximum_bytes.is_some() || !refinements.is_empty() {
                    return Err(error(
                        *span,
                        "a generic Type application cannot be directly refined".into(),
                    ));
                }
                let resolved = arguments
                    .iter()
                    .map(|argument| self.expression(argument, substitutions))
                    .collect::<Result<Vec<_>, _>>()?;
                let Some(template) = self.generics.get(value_type.text.as_str()).copied() else {
                    if self
                        .declarations
                        .iter()
                        .any(|candidate| candidate.name.text == value_type.text)
                    {
                        return Err(error(
                            *span,
                            alloc::format!(
                                "semantic Type '{}' does not accept generic arguments",
                                value_type.text
                            ),
                        ));
                    }
                    return Err(error(
                        *span,
                        alloc::format!(
                            "generic semantic Type '{}' is not in scope",
                            value_type.text
                        ),
                    ));
                };
                if template.parameters.len() != resolved.len() {
                    return Err(error(
                        *span,
                        alloc::format!(
                            "generic semantic Type '{}' expects {} arguments but received {}",
                            value_type.text,
                            template.parameters.len(),
                            resolved.len()
                        ),
                    ));
                }
                let key = application_key(&value_type.text, &resolved);
                if self.active.contains(&key)
                    || self.active.len() >= MAXIMUM_GENERIC_INSTANTIATION_DEPTH
                {
                    return Err(error(
                        *span,
                        alloc::format!(
                            "recursive or unbounded generic semantic Type instantiation: {key}"
                        ),
                    ));
                }
                if let Some(alias) = self.aliases.get(&key) {
                    return Ok(TypeExpressionSyntax::Reference {
                        value_type: alias.clone(),
                        arguments: Vec::new(),
                        maximum_bytes: None,
                        refinements: Vec::new(),
                        span: *span,
                    });
                }
                if !self.generated.contains_key(&key) {
                    self.active.push(key.clone());
                    let bindings = template
                        .parameters
                        .iter()
                        .map(|parameter| parameter.text.clone())
                        .zip(resolved)
                        .collect::<BTreeMap<_, _>>();
                    let definition = self.definition(&template.definition, &bindings)?;
                    self.active.pop();
                    let generated_name = instantiated_name(&value_type.text, &key);
                    self.generated.insert(
                        key.clone(),
                        TypeSyntax {
                            name: SpannedText {
                                text: generated_name,
                                span: template.name.span,
                            },
                            parameters: Vec::new(),
                            generic_context: Some(key.clone()),
                            definition,
                            invariants: template.invariants.clone(),
                            span: template.span,
                        },
                    );
                }
                Ok(TypeExpressionSyntax::Reference {
                    value_type: SpannedText {
                        text: self.generated[&key].name.text.clone(),
                        span: value_type.span,
                    },
                    arguments: Vec::new(),
                    maximum_bytes: None,
                    refinements: Vec::new(),
                    span: *span,
                })
            }
            TypeExpressionSyntax::Optional { value, span } => Ok(TypeExpressionSyntax::Optional {
                value: Box::new(self.expression(value, substitutions)?),
                span: *span,
            }),
            TypeExpressionSyntax::DataReference { value, span } => {
                Ok(TypeExpressionSyntax::DataReference {
                    value: Box::new(self.expression(value, substitutions)?),
                    span: *span,
                })
            }
            TypeExpressionSyntax::Collection {
                element,
                length,
                span,
            } => Ok(TypeExpressionSyntax::Collection {
                element: Box::new(self.expression(element, substitutions)?),
                length: *length,
                span: *span,
            }),
            TypeExpressionSyntax::Sequence {
                element,
                minimum_items,
                maximum_items,
                span,
            } => Ok(TypeExpressionSyntax::Sequence {
                element: Box::new(self.expression(element, substitutions)?),
                minimum_items: *minimum_items,
                maximum_items: *maximum_items,
                span: *span,
            }),
        }
    }
}
