use crate::prelude::*;
use crate::{NativeTypeValueContract, StartupCatalog, SyntaxCheckDiagnostic};

pub(crate) fn install_import_aliases(
    document: &crate::SyntaxDocument,
    base: &StartupCatalog,
) -> Result<StartupCatalog, SyntaxCheckDiagnostic> {
    let mut catalog = base.clone();
    for declaration in &document.uses {
        let Some(value_type) = base.structured_type(&declaration.path).cloned() else {
            continue;
        };
        if base.get(&declaration.path).is_some() {
            return Err(super::diagnostic(
                declaration.path_span,
                alloc::format!(
                    "with path '{}' is ambiguous between a semantic Type and executable Kind",
                    declaration.path
                ),
            ));
        }
        if !document_mentions_type(document, &declaration.alias.text) {
            return Err(super::diagnostic(
                declaration.alias.span,
                alloc::format!("unused with Type alias '{}'", declaration.alias.text),
            ));
        }
        let contracts = base
            .structured_type_contracts(&declaration.path)
            .map_or_else(Vec::new, <[NativeTypeValueContract]>::to_vec);
        let invariants = base
            .structured_type_invariants(&declaration.path)
            .map_or_else(Vec::new, <[crate::PortableExpressionProgram]>::to_vec);
        catalog
            .insert_native_type(
                declaration.alias.text.clone(),
                value_type,
                contracts,
                invariants,
            )
            .map_err(|message| super::diagnostic(declaration.alias.span, message))?;
    }
    Ok(catalog)
}

fn document_mentions_type(document: &crate::SyntaxDocument, name: &str) -> bool {
    document.types.iter().any(|declaration| {
        let mut references = Vec::new();
        super::definition_references(&declaration.definition, &mut references);
        references.contains(&name)
    }) || document.forms.iter().any(|form| {
        form.front
            .startup_parameters
            .iter()
            .any(|parameter| parameter.value_type.text == name)
            || form
                .front
                .runtime_ports
                .iter()
                .any(|port| port.value_type.text == name)
            || form.back.iter().any(|statement| match statement {
                crate::BackStatement::NamedGear(gear) => gear
                    .retained
                    .as_deref()
                    .is_some_and(|retained| retained.value_type.text == name),
                _ => false,
            })
    })
}

pub(crate) fn validate_concrete_value(
    source_type: &str,
    value: &crate::CanonicalStructuredStartupValue,
    catalog: &StartupCatalog,
    span: crate::Span,
) -> Result<(), SyntaxCheckDiagnostic> {
    let Some(contracts) = catalog.structured_type_contracts(source_type) else {
        return Ok(());
    };
    let Some(concrete) = value.try_concrete() else {
        return Ok(());
    };
    crate::rust_binding::validate_native_contracts(&concrete, contracts)
        .and_then(|()| {
            crate::rust_binding::validate_native_invariants(
                &concrete,
                catalog
                    .structured_type_invariants(source_type)
                    .unwrap_or_default(),
            )
        })
        .map_err(|error| {
            super::diagnostic(
                span,
                alloc::format!(
                    "native Type '{source_type}' refinement refuses this value: {error:?}"
                ),
            )
        })
}
