//! Resolve parameter annotations through the ordinary checked native catalogue.
use super::{binding::type_span, error};
use crate::prelude::*;
use crate::{StartupCatalog, SyntaxCheckDiagnostic, TypeExpressionSyntax as Type, TypeSyntax};
use alloc::collections::{BTreeMap, BTreeSet};

pub(super) fn prepare(
    declarations: &[TypeSyntax],
    base: &StartupCatalog,
) -> Result<StartupCatalog, SyntaxCheckDiagnostic> {
    let by_name = declarations
        .iter()
        .map(|value| (value.name.text.as_str(), value))
        .collect::<BTreeMap<_, _>>();
    let mut dependencies = Dependencies {
        declarations: &by_name,
        active: BTreeSet::new(),
        complete: BTreeSet::new(),
    };
    for declaration in declarations {
        for parameter in &declaration.parameters {
            if let Some(annotation) = &parameter.value_type {
                let Type::Reference {
                    value_type,
                    arguments,
                    ..
                } = annotation.as_ref()
                else {
                    return Err(error(
                        type_span(annotation),
                        "native Info parameters require a checked named integral Type".into(),
                    ));
                };
                if !arguments.is_empty() {
                    return Err(error(
                        type_span(annotation),
                        "name the closed integral Type before using it as a parameter annotation"
                            .into(),
                    ));
                }
                dependencies.visit(&value_type.text)?;
            }
        }
    }
    let catalog = if dependencies.complete.is_empty() {
        base.clone()
    } else {
        let subset = declarations
            .iter()
            .filter(|value| dependencies.complete.contains(value.name.text.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        super::super::check_native_types(&subset, base)?.1
    };
    for declaration in declarations {
        for parameter in &declaration.parameters {
            if let Some(annotation) = &parameter.value_type {
                checked(annotation, &catalog)?;
            }
        }
    }
    Ok(catalog)
}

pub(super) fn checked(
    annotation: &Type,
    catalog: &StartupCatalog,
) -> Result<super::super::CompiledRepresentation, SyntaxCheckDiagnostic> {
    let compiled = super::super::compile_expression(annotation, catalog)?;
    if super::super::primitive_representation_kind(&compiled.value_type).map(|kind| kind.as_str())
        != Some("value/u16")
    {
        return Err(error(
            type_span(annotation),
            "the first native Info parameter profile requires U16 or a checked U16 refinement"
                .into(),
        ));
    }
    Ok(compiled)
}

struct Dependencies<'a, 'b> {
    declarations: &'b BTreeMap<&'a str, &'a TypeSyntax>,
    active: BTreeSet<&'a str>,
    complete: BTreeSet<&'a str>,
}
impl<'a> Dependencies<'a, '_> {
    fn visit(&mut self, name: &str) -> Result<(), SyntaxCheckDiagnostic> {
        let Some(declaration) = self.declarations.get(name).copied() else {
            return Ok(());
        };
        let name = declaration.name.text.as_str();
        if self.complete.contains(name) {
            return Ok(());
        }
        if self.active.len() >= 32 || !self.active.insert(name) {
            return Err(error(declaration.name.span, "native parameter Type dependencies are recursive or exceed their finite depth budget".into()));
        }
        let mut references = Vec::new();
        super::super::definition_references(&declaration.definition, &mut references);
        for parameter in &declaration.parameters {
            if let Some(annotation) = &parameter.value_type {
                super::super::expression_references(annotation, &mut references);
            }
        }
        for reference in references {
            if !declaration
                .parameters
                .iter()
                .any(|parameter| parameter.name.text == reference)
            {
                self.visit(reference)?;
            }
        }
        self.active.remove(name);
        self.complete.insert(name);
        Ok(())
    }
}
pub(super) fn validate_laws(
    value_type: &conduit_core::StructuredInfoType,
    scalar: u16,
    catalog: &StartupCatalog,
    span: crate::Span,
) -> Result<(), SyntaxCheckDiagnostic> {
    fn construct(
        value_type: &conduit_core::StructuredInfoType,
        scalar: u16,
        catalog: &StartupCatalog,
        span: crate::Span,
    ) -> Result<conduit_core::StructuredInfoValue, SyntaxCheckDiagnostic> {
        use conduit_core::{StructuredInfoTypeShape as Shape, StructuredInfoValue as Value};
        let value = match value_type.shape() {
            Shape::Leaf(_) => Value::leaf(value_type.clone(), scalar.to_le_bytes().to_vec()),
            Shape::Nominal { representation, .. } => {
                let representation = construct(representation, scalar, catalog, span)?;
                Value::nominal(value_type.clone(), representation)
            }
            _ => {
                return Err(error(
                    span,
                    "native Info parameter requires an integral scalar representation".into(),
                ))
            }
        }
        .map_err(|refusal| {
            error(
                span,
                alloc::format!("invalid native Info argument: {refusal:?}"),
            )
        })?;
        let profile = value_type.profile().map_err(|refusal| {
            error(
                span,
                alloc::format!("invalid native parameter Type: {refusal:?}"),
            )
        })?;
        if let Some(name) = catalog.structured_type_name(profile.value_kind()) {
            if let Some(laws) = catalog.structured_type_invariants(name) {
                crate::rust_binding::validate_native_invariants(&value, laws).map_err(
                    |refusal| {
                        error(
                            span,
                            alloc::format!(
                                "native Info argument violates its declared Type law: {refusal:?}"
                            ),
                        )
                    },
                )?;
            }
        }
        Ok(value)
    }
    construct(value_type, scalar, catalog, span).map(|_| ())
}

pub(super) fn prepare_expression(
    expression: &Type,
    declarations: &[TypeSyntax],
    base: &StartupCatalog,
) -> Result<StartupCatalog, SyntaxCheckDiagnostic> {
    let mut references = Vec::new();
    super::super::expression_references(expression, &mut references);
    prepare_references(&references, declarations, base)
}

pub(super) fn prepare_references(
    references: &[&str],
    declarations: &[TypeSyntax],
    base: &StartupCatalog,
) -> Result<StartupCatalog, SyntaxCheckDiagnostic> {
    let by_name = declarations
        .iter()
        .filter(|value| base.structured_type(&value.name.text).is_none())
        .map(|value| (value.name.text.as_str(), value))
        .collect::<BTreeMap<_, _>>();
    let mut dependencies = Dependencies {
        declarations: &by_name,
        active: BTreeSet::new(),
        complete: BTreeSet::new(),
    };
    for reference in references {
        dependencies.visit(reference)?;
    }
    if dependencies.complete.is_empty() {
        return Ok(base.clone());
    }
    let subset = declarations
        .iter()
        .filter(|value| dependencies.complete.contains(value.name.text.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    super::super::check_native_types(&subset, base).map(|(_, catalog)| catalog)
}
