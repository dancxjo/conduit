//! Capture only the checked dependency closure of one shipped family.
use crate::prelude::*;
use crate::{CheckedNativeType, StartupCatalog, SyntaxCheckDiagnostic, TypeSyntax};
use alloc::collections::{BTreeMap, BTreeSet};

pub(super) fn select(
    root: &TypeSyntax,
    declarations: &[TypeSyntax],
    owner: &StartupCatalog,
) -> Result<(Vec<TypeSyntax>, StartupCatalog), SyntaxCheckDiagnostic> {
    super::super::generic::budget::validate_iter(declarations.iter())?;
    super::budget::validate(owner, None, root.name.span)?;
    let declarations = declarations
        .iter()
        .map(|value| (value.name.text.as_str(), value))
        .collect::<BTreeMap<_, _>>();
    let mut pending = alloc::vec![root.name.text.clone()];
    let mut visited = BTreeSet::new();
    let mut families = BTreeSet::new();
    while let Some(name) = pending.pop() {
        if !visited.insert(name.clone()) {
            continue;
        }
        if let Some(declaration) = declarations.get(name.as_str()) {
            pending.extend(references(declaration));
        } else if owner.native_families.contains_key(&name) {
            families.insert(name);
        }
    }
    let selected = declarations
        .values()
        .filter(|value| visited.contains(&value.name.text))
        .map(|value| (*value).clone())
        .collect();
    let mut owner = owner.clone();
    owner
        .native_families
        .retain(|name, _| families.contains(name));
    Ok((selected, owner))
}

pub(super) fn dependencies(
    templates: &[TypeSyntax],
    declarations: &[TypeSyntax],
    catalog: &StartupCatalog,
) -> Result<Vec<CheckedNativeType>, SyntaxCheckDiagnostic> {
    let template_names = templates
        .iter()
        .map(|value| value.name.text.as_str())
        .collect::<BTreeSet<_>>();
    let by_name = declarations
        .iter()
        .map(|value| (value.name.text.as_str(), value))
        .collect::<BTreeMap<_, _>>();
    let mut pending = templates.iter().flat_map(references).collect::<Vec<_>>();
    let mut visited = BTreeSet::new();
    let mut dependencies = Vec::new();
    let mut remaining = 16_000usize;
    while let Some(name) = pending.pop() {
        if !visited.insert(name.clone()) || template_names.contains(name.as_str()) {
            continue;
        }
        if let Some(declaration) = by_name.get(name.as_str()) {
            pending.extend(references(declaration));
        }
        if catalog.structured_type(&name).is_none() && catalog.value_kind_alias(&name).is_none() {
            continue;
        }
        let span = templates[0].name.span;
        if dependencies.len() >= 4096 {
            return Err(super::super::diagnostic(
                span,
                "native family capture exceeds its dependency-count profile".into(),
            ));
        }
        let value_type =
            crate::value_type::checked_value_type(&name, catalog).map_err(|refusal| {
                super::super::diagnostic(span, alloc::format!("invalid captured Type: {refusal:?}"))
            })?;
        let identity = value_type
            .profile()
            .map_err(|refusal| {
                super::super::diagnostic(span, alloc::format!("invalid captured Type: {refusal:?}"))
            })?
            .value_kind()
            .clone();
        nested_names(&value_type, catalog, &mut pending, &mut remaining, span)?;
        dependencies.push(CheckedNativeType {
            name: name.clone(),
            identity,
            value_type,
            value_contracts: catalog
                .structured_type_contracts(&name)
                .unwrap_or_default()
                .to_vec(),
            invariants: catalog
                .structured_type_invariants(&name)
                .unwrap_or_default()
                .to_vec(),
        });
    }
    Ok(dependencies)
}

fn references(declaration: &TypeSyntax) -> Vec<String> {
    let mut references = Vec::new();
    super::super::definition_references(&declaration.definition, &mut references);
    for parameter in &declaration.parameters {
        if let Some(annotation) = &parameter.value_type {
            super::super::expression_references(annotation, &mut references);
        }
    }
    references
        .into_iter()
        .filter(|name| {
            !declaration
                .parameters
                .iter()
                .any(|parameter| parameter.name.text == *name)
        })
        .map(str::to_string)
        .collect()
}

fn nested_names(
    value: &conduit_core::StructuredInfoType,
    catalog: &StartupCatalog,
    names: &mut Vec<String>,
    remaining: &mut usize,
    span: crate::Span,
) -> Result<(), SyntaxCheckDiagnostic> {
    if *remaining == 0 {
        return Err(super::super::diagnostic(
            span,
            "native family capture exceeds its finite structure-work profile".into(),
        ));
    }
    *remaining -= 1;
    use conduit_core::StructuredInfoTypeShape as Shape;
    match value.shape() {
        Shape::Leaf(_) => {}
        Shape::Nominal {
            schema,
            representation,
        } => {
            if let Some(name) = catalog.structured_type_name(schema) {
                names.push(name.into());
            }
            nested_names(representation, catalog, names, remaining, span)?;
        }
        Shape::Collection { element, .. } | Shape::Sequence { element, .. } => {
            nested_names(element, catalog, names, remaining, span)?;
        }
        Shape::Record { schema, fields } => {
            if let Some(name) = catalog.structured_type_name(schema) {
                names.push(name.into());
            }
            for field in fields {
                nested_names(field.value_type(), catalog, names, remaining, span)?;
            }
        }
        Shape::Variant { schema, cases } => {
            if let Some(name) = catalog.structured_type_name(schema) {
                names.push(name.into());
            }
            for case in cases {
                nested_names(case.payload_type(), catalog, names, remaining, span)?;
            }
        }
    }
    Ok(())
}
