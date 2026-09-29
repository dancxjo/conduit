use crate::prelude::*;
use crate::{NativeTypeValueContract, StartupCatalog, SyntaxCheckDiagnostic};
use conduit_core::CheckedValueContract;

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
        catalog
            .insert_native_type(declaration.alias.text.clone(), value_type, contracts)
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
    for contract in contracts {
        validate_at_path(&concrete, &contract.representation_path, &contract.contract).map_err(
            |error| {
                super::diagnostic(
                    span,
                    alloc::format!(
                        "native Type '{}' refinement refuses this value at '{}': {error:?}",
                        source_type,
                        contract.representation_path
                    ),
                )
            },
        )?;
    }
    Ok(())
}

fn validate_at_path(
    value: &conduit_core::StructuredInfoValue,
    path: &str,
    contract: &CheckedValueContract,
) -> Result<(), conduit_core::ValueConstraintRefusal> {
    if path.is_empty() {
        let conduit_core::StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
            return Err(conduit_core::ValueConstraintRefusal::WrongConstraintKind);
        };
        return contract.validate(bytes);
    }
    if let Some(rest) = path.strip_prefix("[]") {
        let conduit_core::StructuredInfoValueShape::Collection(values) = value.shape() else {
            return Err(conduit_core::ValueConstraintRefusal::WrongConstraintKind);
        };
        for item in values {
            validate_at_path(item, rest, contract)?;
        }
        return Ok(());
    }
    if let Some(rest) = path.strip_prefix("?some") {
        let conduit_core::StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
            return Err(conduit_core::ValueConstraintRefusal::WrongConstraintKind);
        };
        return if tag == "some" {
            validate_at_path(payload, rest, contract)
        } else {
            Ok(())
        };
    }
    if let Some(rest) = path.strip_prefix('.') {
        let (name, remaining) = split_path_component(rest);
        let conduit_core::StructuredInfoValueShape::Record(fields) = value.shape() else {
            return Err(conduit_core::ValueConstraintRefusal::WrongConstraintKind);
        };
        let field = fields
            .iter()
            .find(|field| field.name() == name)
            .ok_or(conduit_core::ValueConstraintRefusal::WrongConstraintKind)?;
        return validate_at_path(field.value(), remaining, contract);
    }
    if let Some(rest) = path.strip_prefix('|') {
        let (wanted, remaining) = split_path_component(rest);
        let conduit_core::StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
            return Err(conduit_core::ValueConstraintRefusal::WrongConstraintKind);
        };
        return if tag == wanted {
            validate_at_path(payload, remaining, contract)
        } else {
            Ok(())
        };
    }
    Err(conduit_core::ValueConstraintRefusal::WrongConstraintKind)
}

fn split_path_component(path: &str) -> (&str, &str) {
    let end = path
        .char_indices()
        .find_map(|(index, character)| {
            (index > 0 && matches!(character, '.' | '|' | '[' | '?')).then_some(index)
        })
        .unwrap_or(path.len());
    path.split_at(end)
}
