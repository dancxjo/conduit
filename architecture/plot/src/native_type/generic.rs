use crate::prelude::*;
use crate::{
    SpannedText, SyntaxCheckDiagnostic, TypeDefinitionSyntax, TypeExpressionSyntax, TypeSyntax,
    TypeVariantPayloadSyntax,
};
use alloc::collections::{BTreeMap, BTreeSet};

mod argument;
mod binding;
pub(super) mod budget;
mod canonical;
pub(super) mod imports;
mod integer;
mod law;
mod origin;
mod parameter;
mod refinement;
mod substitution;
use binding::Bindings;
use canonical::application_key;

const MAXIMUM_GENERIC_INSTANTIATION_DEPTH: usize = 32;

/// Resolves authored generic declarations into finite concrete declarations.
///
/// Generic declarations are compile-time templates. Only ordinary declarations
/// are returned as public bindings; private instantiated declarations are
/// retained long enough for the native checker to give every use an exact
/// nominal identity.
pub(super) fn instantiate(
    declarations: &[TypeSyntax],
    catalog: &crate::StartupCatalog,
) -> Result<(Vec<TypeSyntax>, BTreeSet<String>, crate::StartupCatalog), SyntaxCheckDiagnostic> {
    budget::validate(declarations)?;
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
            if !parameters.insert(parameter.name.text.as_str()) {
                return Err(error(
                    parameter.name.span,
                    alloc::format!(
                        "generic Type parameter '{}' is duplicated",
                        parameter.name.text
                    ),
                ));
            }
        }
        if declaration.parameters.is_empty() {
            public.insert(declaration.name.text.clone());
        } else {
            for parameter in &declaration.parameters {
                if !definition_uses(&declaration.definition, &parameter.name.text)
                    && !law::uses(&declaration.invariants, &parameter.name.text)
                {
                    return Err(error(
                        parameter.name.span,
                        alloc::format!(
                            "generic Type parameter '{}' is unused",
                            parameter.name.text
                        ),
                    ));
                }
            }
            generics.insert(declaration.name.text.as_str(), declaration);
        }
    }

    let imports = imports::Imports::prepare(catalog)?;
    for template in &imports.templates {
        generics.insert(template.name.text.as_str(), template);
    }
    for (alias, name) in &imports.aliases {
        if names.contains(alias.as_str()) {
            return Err(error(
                declarations[0].name.span,
                alloc::format!("imported Type family '{alias}' conflicts with a local declaration"),
            ));
        }
        let template = imports
            .templates
            .iter()
            .find(|template| template.name.text == *name)
            .expect("captured family root");
        generics.insert(alias.as_str(), template);
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
            (!arguments.is_empty()
                && refinements.is_empty()
                && declaration.parameters.is_empty()
                && !generics
                    .get(value_type.text.as_str())
                    .is_some_and(|template: &&TypeSyntax| {
                        template
                            .parameters
                            .iter()
                            .any(|parameter| parameter.value_type.is_some())
                    }))
            .then(|| {
                (
                    application_key(&value_type.text, arguments),
                    declaration.name.clone(),
                )
            })
        })
        .collect();
    let parameter_catalog = parameter::prepare(declarations, &imports.catalog)?;
    let mut context = Context {
        declarations,
        catalog: &parameter_catalog,
        generics,
        origins: &imports.origins,
        aliases,
        generated: BTreeMap::new(),
        active: Vec::new(),
        generated_instances: 0,
        expression_steps: 0,
    };
    let mut concrete = Vec::new();
    for declaration in declarations
        .iter()
        .filter(|declaration| declaration.parameters.is_empty())
    {
        let mut declaration = declaration.clone();
        let (definition, generic_context, bindings) =
            context.public_definition(&declaration.definition)?;
        let mut invariants = declaration.invariants.clone();
        if generic_context.is_some() {
            if let TypeDefinitionSyntax::Scalar(TypeExpressionSyntax::Reference {
                value_type,
                ..
            }) = &declaration.definition
            {
                if let Some(template) = context.generics.get(value_type.text.as_str()) {
                    invariants.extend(template.invariants.clone());
                }
            }
        }
        declaration.invariants = law::substitute(&invariants, &bindings.values)?;
        declaration.definition = definition;
        declaration.generic_context = generic_context.or(declaration.generic_context);
        concrete.push(declaration);
    }
    public.extend(
        context
            .generated
            .values()
            .map(|declaration| declaration.name.text.clone()),
    );
    concrete.extend(context.generated.into_values());
    Ok((concrete, public, imports.catalog))
}

struct Context<'a> {
    declarations: &'a [TypeSyntax],
    catalog: &'a crate::StartupCatalog,
    generics: BTreeMap<&'a str, &'a TypeSyntax>,
    origins: &'a BTreeMap<String, TypeSyntax>,
    aliases: BTreeMap<String, SpannedText>,
    generated: BTreeMap<String, TypeSyntax>,
    active: Vec<String>,
    generated_instances: usize,
    expression_steps: usize,
}

impl Context<'_> {
    fn origin<'a>(&'a self, template: &'a TypeSyntax) -> &'a TypeSyntax {
        self.origins.get(&template.name.text).unwrap_or(template)
    }

    fn public_definition(
        &mut self,
        definition: &TypeDefinitionSyntax,
    ) -> Result<(TypeDefinitionSyntax, Option<String>, Bindings), SyntaxCheckDiagnostic> {
        let TypeDefinitionSyntax::Scalar(TypeExpressionSyntax::Reference {
            value_type,
            arguments,
            maximum_bytes: None,
            refinements,
            span,
        }) = definition
        else {
            return self
                .definition(definition, &Bindings::default())
                .map(|definition| (definition, None, Bindings::default()));
        };
        if arguments.is_empty() || !refinements.is_empty() {
            return self
                .definition(definition, &Bindings::default())
                .map(|definition| (definition, None, Bindings::default()));
        }
        let Some(template) = self.generics.get(value_type.text.as_str()).copied() else {
            return self
                .definition(definition, &Bindings::default())
                .map(|definition| (definition, None, Bindings::default()));
        };
        let (resolved, bindings) = self.bind(template, arguments, &Bindings::default(), *span)?;
        let origin = self.semantic_origin(template)?;
        let key = canonical::family_key(
            &origin,
            &resolved,
            &bindings.parameter_contracts,
            &bindings.argument_identities,
        );
        self.active.push(key.clone());
        let result = self.definition(&template.definition, &bindings);
        self.active.pop();
        result.map(|definition| (definition, Some(key), bindings))
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
            refinements,
            ..
        } => {
            value_type.text == parameter
                || refinement::uses(refinements, parameter)
                || arguments.iter().any(|argument| match argument {
                    crate::NativeTypeArgumentSyntax::Type(value) => {
                        expression_uses(value, parameter)
                    }
                    crate::NativeTypeArgumentSyntax::Value(value) => integer_uses(value, parameter),
                })
        }
        TypeExpressionSyntax::Optional { value, .. }
        | TypeExpressionSyntax::DataReference { value, .. } => expression_uses(value, parameter),
        TypeExpressionSyntax::Collection {
            element, length, ..
        } => expression_uses(element, parameter) || integer_uses(length, parameter),
        TypeExpressionSyntax::Sequence {
            element,
            minimum_items,
            maximum_items,
            ..
        } => {
            expression_uses(element, parameter)
                || integer_uses(minimum_items, parameter)
                || integer_uses(maximum_items, parameter)
        }
    }
}

fn error(span: crate::Span, message: String) -> SyntaxCheckDiagnostic {
    SyntaxCheckDiagnostic {
        code: "CND-FRM-058",
        span,
        message,
    }
}

fn integer_uses(value: &crate::NativeIntegerExpressionSyntax, parameter: &str) -> bool {
    match value {
        crate::NativeIntegerExpressionSyntax::Literal { .. } => false,
        crate::NativeIntegerExpressionSyntax::Parameter(name) => name.text == parameter,
        crate::NativeIntegerExpressionSyntax::Group { value, .. } => integer_uses(value, parameter),
        crate::NativeIntegerExpressionSyntax::Binary { left, right, .. } => {
            integer_uses(left, parameter) || integer_uses(right, parameter)
        }
    }
}
