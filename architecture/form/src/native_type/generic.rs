use crate::prelude::*;
use crate::{
    SpannedText, SyntaxCheckDiagnostic, TypeDefinitionSyntax, TypeExpressionSyntax,
    TypeFieldSyntax, TypeSyntax, TypeVariantCaseSyntax, TypeVariantPayloadSyntax,
};
use alloc::collections::{BTreeMap, BTreeSet};

mod canonical;
use canonical::{application_key, expression as canonical, instantiated_name};

const MAXIMUM_GENERIC_INSTANTIATION_DEPTH: usize = 32;

/// Resolves authored generic declarations into finite concrete declarations.
///
/// Generic declarations are compile-time templates. Only ordinary declarations
/// are returned as public bindings; private instantiated declarations are
/// retained long enough for the native checker to give every use an exact
/// nominal identity.
pub(super) fn instantiate(
    declarations: &[TypeSyntax],
) -> Result<(Vec<TypeSyntax>, BTreeSet<String>), SyntaxCheckDiagnostic> {
    let mut names = BTreeSet::new();
    let mut generics = BTreeMap::new();
    let mut public = BTreeSet::new();
    for declaration in declarations {
        if !names.insert(declaration.name.text.as_str()) {
            return Err(error(
                declaration.name.span,
                alloc::format!(
                    "semantic Type '{}' is declared more than once",
                    declaration.name.text
                ),
            ));
        }
        let mut parameters = BTreeSet::new();
        for parameter in &declaration.parameters {
            if !parameters.insert(parameter.text.as_str()) {
                return Err(error(
                    parameter.span,
                    alloc::format!("generic Type parameter '{}' is duplicated", parameter.text),
                ));
            }
        }
        if declaration.parameters.is_empty() {
            public.insert(declaration.name.text.clone());
        } else {
            for parameter in &declaration.parameters {
                if !definition_uses(&declaration.definition, &parameter.text) {
                    return Err(error(
                        parameter.span,
                        alloc::format!("generic Type parameter '{}' is unused", parameter.text),
                    ));
                }
            }
            generics.insert(declaration.name.text.as_str(), declaration);
        }
    }

    let aliases = declarations
        .iter()
        .filter_map(|declaration| {
            let TypeDefinitionSyntax::Scalar(TypeExpressionSyntax::Reference {
                value_type,
                arguments,
                maximum_bytes: None,
                refinements,
                ..
            }) = &declaration.definition
            else {
                return None;
            };
            (!arguments.is_empty() && refinements.is_empty()).then(|| {
                (
                    application_key(&value_type.text, arguments),
                    declaration.name.clone(),
                )
            })
        })
        .collect();
    let mut context = Context {
        declarations,
        generics,
        aliases,
        generated: BTreeMap::new(),
        active: Vec::new(),
    };
    let mut concrete = Vec::new();
    for declaration in declarations
        .iter()
        .filter(|declaration| declaration.parameters.is_empty())
    {
        let mut declaration = declaration.clone();
        let (definition, generic_context) = context.public_definition(&declaration.definition)?;
        declaration.definition = definition;
        declaration.generic_context = generic_context;
        concrete.push(declaration);
    }
    public.extend(
        context
            .generated
            .values()
            .map(|declaration| declaration.name.text.clone()),
    );
    concrete.extend(context.generated.into_values());
    Ok((concrete, public))
}

struct Context<'a> {
    declarations: &'a [TypeSyntax],
    generics: BTreeMap<&'a str, &'a TypeSyntax>,
    aliases: BTreeMap<String, SpannedText>,
    generated: BTreeMap<String, TypeSyntax>,
    active: Vec<String>,
}

impl Context<'_> {
    fn public_definition(
        &mut self,
        definition: &TypeDefinitionSyntax,
    ) -> Result<(TypeDefinitionSyntax, Option<String>), SyntaxCheckDiagnostic> {
        let TypeDefinitionSyntax::Scalar(TypeExpressionSyntax::Reference {
            value_type,
            arguments,
            maximum_bytes: None,
            refinements,
            span,
        }) = definition
        else {
            return self
                .definition(definition, &BTreeMap::new())
                .map(|definition| (definition, None));
        };
        if arguments.is_empty() || !refinements.is_empty() {
            return self
                .definition(definition, &BTreeMap::new())
                .map(|definition| (definition, None));
        }
        let Some(template) = self.generics.get(value_type.text.as_str()).copied() else {
            return self
                .definition(definition, &BTreeMap::new())
                .map(|definition| (definition, None));
        };
        if template.parameters.len() != arguments.len() {
            return Err(error(
                *span,
                alloc::format!(
                    "generic semantic Type '{}' expects {} arguments but received {}",
                    value_type.text,
                    template.parameters.len(),
                    arguments.len()
                ),
            ));
        }
        let resolved = arguments
            .iter()
            .map(|argument| self.expression(argument, &BTreeMap::new()))
            .collect::<Result<Vec<_>, _>>()?;
        let key = alloc::format!(
            "{}<{}>",
            value_type.text,
            resolved.iter().map(canonical).collect::<Vec<_>>().join(",")
        );
        self.active.push(key.clone());
        let bindings = template
            .parameters
            .iter()
            .map(|parameter| parameter.text.clone())
            .zip(resolved)
            .collect::<BTreeMap<_, _>>();
        let result = self.definition(&template.definition, &bindings);
        self.active.pop();
        result.map(|definition| (definition, Some(key)))
    }

    fn definition(
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

    fn expression(
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

fn definition_uses(definition: &TypeDefinitionSyntax, parameter: &str) -> bool {
    match definition {
        TypeDefinitionSyntax::Scalar(value) => expression_uses(value, parameter),
        TypeDefinitionSyntax::Record(fields) => fields
            .iter()
            .any(|field| expression_uses(&field.value_type, parameter)),
        TypeDefinitionSyntax::Variant(cases) => cases.iter().any(|case| match &case.payload {
            TypeVariantPayloadSyntax::Unit => false,
            TypeVariantPayloadSyntax::Type(value) => expression_uses(value, parameter),
            TypeVariantPayloadSyntax::Record(fields) => fields
                .iter()
                .any(|field| expression_uses(&field.value_type, parameter)),
        }),
    }
}

fn expression_uses(expression: &TypeExpressionSyntax, parameter: &str) -> bool {
    match expression {
        TypeExpressionSyntax::Reference {
            value_type,
            arguments,
            ..
        } => {
            value_type.text == parameter
                || arguments
                    .iter()
                    .any(|argument| expression_uses(argument, parameter))
        }
        TypeExpressionSyntax::Optional { value, .. }
        | TypeExpressionSyntax::DataReference { value, .. } => expression_uses(value, parameter),
        TypeExpressionSyntax::Collection { element, .. }
        | TypeExpressionSyntax::Sequence { element, .. } => expression_uses(element, parameter),
    }
}

fn error(span: crate::Span, message: String) -> SyntaxCheckDiagnostic {
    SyntaxCheckDiagnostic {
        code: "CND-FRM-058",
        span,
        message,
    }
}
