use crate::prelude::*;
use crate::{
    SpannedText, SyntaxCheckDiagnostic, TypeDefinitionSyntax, TypeExpressionSyntax, TypeSyntax,
    TypeVariantPayloadSyntax,
};
use alloc::collections::{BTreeMap, BTreeSet};

mod canonical;
mod substitution;
use canonical::{application_key, expression as canonical};

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
