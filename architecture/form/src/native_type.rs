use crate::prelude::*;
use crate::{
    CheckedNativeType, NativeTypeValueContract, StartupCatalog, SyntaxCheckDiagnostic,
    TypeDefinitionSyntax, TypeExpressionSyntax, TypeFieldSyntax, TypeSyntax,
};
use alloc::collections::BTreeSet;
use conduit_core::{
    data_reference_kind, kind_id, CheckedValueContract, KindId, StructuredFieldType,
    StructuredInfoType, StructuredVariantCase,
};
use sha2::{Digest, Sha256};

#[derive(Clone)]
struct CompiledRepresentation {
    value_type: StructuredInfoType,
    contracts: Vec<NativeTypeValueContract>,
}

pub(crate) fn check_native_types(
    declarations: &[TypeSyntax],
    base: &StartupCatalog,
) -> Result<(Vec<CheckedNativeType>, StartupCatalog), SyntaxCheckDiagnostic> {
    let mut catalog = base.clone();
    let mut by_name = alloc::collections::BTreeMap::new();
    for declaration in declarations {
        if by_name
            .insert(declaration.name.text.as_str(), declaration)
            .is_some()
        {
            return Err(diagnostic(
                declaration.name.span,
                alloc::format!(
                    "semantic Type '{}' is declared more than once",
                    declaration.name.text
                ),
            ));
        }
    }
    let mut active = Vec::new();
    let mut complete = BTreeSet::new();
    let mut compiled = alloc::collections::BTreeMap::new();
    for declaration in declarations {
        compile_named(
            declaration.name.text.as_str(),
            &by_name,
            &mut active,
            &mut complete,
            &mut compiled,
            &mut catalog,
        )?;
    }
    let checked = declarations
        .iter()
        .map(|declaration| {
            compiled
                .remove(declaration.name.text.as_str())
                .expect("every declaration compiled exactly once")
        })
        .collect();
    Ok((checked, catalog))
}

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
            return Err(diagnostic(
                declaration.path_span,
                alloc::format!(
                    "with path '{}' is ambiguous between a semantic Type and executable Kind",
                    declaration.path
                ),
            ));
        }
        if !document_mentions_type(document, &declaration.alias.text) {
            return Err(diagnostic(
                declaration.alias.span,
                alloc::format!("unused with Type alias '{}'", declaration.alias.text),
            ));
        }
        let contracts = base
            .structured_type_contracts(&declaration.path)
            .map_or_else(Vec::new, <[NativeTypeValueContract]>::to_vec);
        catalog
            .insert_native_type(declaration.alias.text.clone(), value_type, contracts)
            .map_err(|message| diagnostic(declaration.alias.span, message))?;
    }
    Ok(catalog)
}

fn document_mentions_type(document: &crate::SyntaxDocument, name: &str) -> bool {
    document.types.iter().any(|declaration| {
        let mut references = Vec::new();
        definition_references(&declaration.definition, &mut references);
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
                diagnostic(
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

fn compile_named<'a>(
    name: &'a str,
    declarations: &alloc::collections::BTreeMap<&'a str, &'a TypeSyntax>,
    active: &mut Vec<&'a str>,
    complete: &mut BTreeSet<&'a str>,
    checked: &mut alloc::collections::BTreeMap<String, CheckedNativeType>,
    catalog: &mut StartupCatalog,
) -> Result<(), SyntaxCheckDiagnostic> {
    if complete.contains(name) {
        return Ok(());
    }
    let declaration = declarations[name];
    if let Some(position) = active.iter().position(|candidate| *candidate == name) {
        let mut cycle = active[position..].to_vec();
        cycle.push(name);
        return Err(diagnostic(
            declaration.name.span,
            alloc::format!(
                "recursive semantic Type cycle is not finite: {}",
                cycle.join(" -> ")
            ),
        ));
    }
    active.push(name);
    let mut references = Vec::new();
    definition_references(&declaration.definition, &mut references);
    for reference in references {
        if declarations.contains_key(reference) {
            compile_named(reference, declarations, active, complete, checked, catalog)?;
        }
    }
    active.pop();
    let compiled = compile_definition(declaration, catalog)?;
    catalog
        .insert_native_type(
            declaration.name.text.clone(),
            compiled.value_type.clone(),
            compiled.value_contracts.clone(),
        )
        .map_err(|message| diagnostic(declaration.name.span, message))?;
    checked.insert(declaration.name.text.clone(), compiled);
    complete.insert(name);
    Ok(())
}

fn definition_references<'a>(definition: &'a TypeDefinitionSyntax, out: &mut Vec<&'a str>) {
    match definition {
        TypeDefinitionSyntax::Scalar(expression) => expression_references(expression, out),
        TypeDefinitionSyntax::Record(fields) => {
            for field in fields {
                expression_references(&field.value_type, out);
            }
        }
        TypeDefinitionSyntax::Variant(cases) => {
            for case in cases {
                for field in &case.fields {
                    expression_references(&field.value_type, out);
                }
            }
        }
    }
}

fn expression_references<'a>(expression: &'a TypeExpressionSyntax, out: &mut Vec<&'a str>) {
    match expression {
        TypeExpressionSyntax::Reference { value_type, .. } => out.push(&value_type.text),
        TypeExpressionSyntax::Optional { value, .. }
        | TypeExpressionSyntax::DataReference { value, .. } => expression_references(value, out),
        TypeExpressionSyntax::Sequence { element, .. } => expression_references(element, out),
    }
}

fn compile_definition(
    declaration: &TypeSyntax,
    catalog: &StartupCatalog,
) -> Result<CheckedNativeType, SyntaxCheckDiagnostic> {
    let compiled = match &declaration.definition {
        TypeDefinitionSyntax::Scalar(expression) => compile_expression(expression, catalog)?,
        TypeDefinitionSyntax::Record(fields) => {
            compile_record(&declaration.name.text, fields, catalog, declaration.span)?
        }
        TypeDefinitionSyntax::Variant(cases) => {
            let mut compiled_cases = Vec::with_capacity(cases.len());
            let mut contracts = Vec::new();
            for case in cases {
                let payload = if case.fields.is_empty() {
                    CompiledRepresentation {
                        value_type: StructuredInfoType::leaf(kind_id("value/unit"))
                            .map_err(|error| bounded(case.span, error))?,
                        contracts: Vec::new(),
                    }
                } else {
                    compile_record(
                        &alloc::format!("{}/{}", declaration.name.text, case.tag.text),
                        &case.fields,
                        catalog,
                        case.span,
                    )?
                };
                contracts.extend(prefix_contracts(
                    payload.contracts,
                    &alloc::format!("|{}", case.tag.text),
                ));
                compiled_cases.push(
                    StructuredVariantCase::new(case.tag.text.clone(), payload.value_type)
                        .map_err(|error| bounded(case.span, error))?,
                );
            }
            let identity = schema_identity_for_variant(&declaration.name.text, &compiled_cases);
            CompiledRepresentation {
                value_type: StructuredInfoType::variant(identity, compiled_cases)
                    .map_err(|error| bounded(declaration.span, error))?,
                contracts,
            }
        }
    };
    let value_type = match &declaration.definition {
        TypeDefinitionSyntax::Scalar(_) => {
            let identity = schema_identity(
                &declaration.name.text,
                &compiled.value_type,
                &compiled.contracts,
            );
            StructuredInfoType::nominal(identity, compiled.value_type)
                .map_err(|error| bounded(declaration.span, error))?
        }
        _ => compiled.value_type,
    };
    let identity = match value_type.shape() {
        conduit_core::StructuredInfoTypeShape::Nominal { schema, .. }
        | conduit_core::StructuredInfoTypeShape::Record { schema, .. }
        | conduit_core::StructuredInfoTypeShape::Variant { schema, .. } => schema.clone(),
        _ => unreachable!("checked native declarations are nominal"),
    };
    Ok(CheckedNativeType {
        name: declaration.name.text.clone(),
        identity,
        value_type,
        value_contracts: compiled.contracts,
    })
}

fn compile_record(
    semantic_name: &str,
    fields: &[TypeFieldSyntax],
    catalog: &StartupCatalog,
    span: crate::Span,
) -> Result<CompiledRepresentation, SyntaxCheckDiagnostic> {
    let mut compiled_fields = Vec::with_capacity(fields.len());
    let mut contracts = Vec::new();
    for field in fields {
        let compiled = compile_expression(&field.value_type, catalog)?;
        contracts.extend(prefix_contracts(
            compiled.contracts,
            &alloc::format!(".{}", field.name.text),
        ));
        compiled_fields.push(
            StructuredFieldType::new(field.name.text.clone(), compiled.value_type)
                .map_err(|error| bounded(field.span, error))?,
        );
    }
    let identity = schema_identity_for_record(semantic_name, &compiled_fields, &contracts);
    Ok(CompiledRepresentation {
        value_type: StructuredInfoType::record(identity, compiled_fields)
            .map_err(|error| bounded(span, error))?,
        contracts,
    })
}

fn compile_expression(
    expression: &TypeExpressionSyntax,
    catalog: &StartupCatalog,
) -> Result<CompiledRepresentation, SyntaxCheckDiagnostic> {
    match expression {
        TypeExpressionSyntax::Reference {
            value_type,
            maximum_bytes,
            refinements,
            span,
        } => {
            let representation = crate::value_type::checked_value_type(&value_type.text, catalog)
                .map_err(|_| {
                diagnostic(
                    value_type.span,
                    alloc::format!("semantic value Type '{}' is not in scope", value_type.text),
                )
            })?;
            let mut contracts = catalog
                .structured_type_contracts(&value_type.text)
                .map_or_else(Vec::new, <[NativeTypeValueContract]>::to_vec);
            if maximum_bytes.is_some() || !refinements.is_empty() {
                let primitive =
                    primitive_representation_kind(&representation).ok_or_else(|| {
                        diagnostic(
                            *span,
                            "refinements require a scalar primitive representation".into(),
                        )
                    })?;
                let (maximum_bytes, constraints) =
                    crate::value_type::refinement::checked_refinements(
                        refinements,
                        *maximum_bytes,
                        primitive,
                        *span,
                    )?;
                let mut contract =
                    CheckedValueContract::new(primitive.clone(), maximum_bytes, constraints)
                        .map_err(|error| {
                            diagnostic(
                                *span,
                                alloc::format!("invalid native Type refinement: {error:?}"),
                            )
                        })?;
                if let Some(inherited) = contracts
                    .iter_mut()
                    .find(|candidate| candidate.representation_path.is_empty())
                {
                    contract.maximum_bytes =
                        contract.maximum_bytes.min(inherited.contract.maximum_bytes);
                    contract
                        .constraints
                        .extend(inherited.contract.constraints.clone());
                    contract.constraints.sort();
                    contract.validate_definition().map_err(|error| {
                        diagnostic(
                            *span,
                            alloc::format!("native Type refinements conflict: {error:?}"),
                        )
                    })?;
                    inherited.contract = contract;
                } else {
                    contracts.push(NativeTypeValueContract {
                        representation_path: String::new(),
                        contract,
                    });
                }
            }
            Ok(CompiledRepresentation {
                value_type: representation,
                contracts,
            })
        }
        TypeExpressionSyntax::Optional { value, span } => {
            let compiled = compile_expression(value, catalog)?;
            Ok(CompiledRepresentation {
                value_type: conduit_core::optional_info_type(compiled.value_type)
                    .map_err(|error| bounded(*span, error))?,
                contracts: prefix_contracts(compiled.contracts, "?some"),
            })
        }
        TypeExpressionSyntax::DataReference { value, span } => {
            let compiled = compile_expression(value, catalog)?;
            let content_kind = compiled
                .value_type
                .profile()
                .map_err(|error| bounded(*span, error))?
                .value_kind()
                .clone();
            Ok(CompiledRepresentation {
                value_type: StructuredInfoType::leaf(data_reference_kind(&content_kind))
                    .map_err(|error| bounded(*span, error))?,
                contracts: Vec::new(),
            })
        }
        TypeExpressionSyntax::Sequence {
            element,
            minimum_items,
            maximum_items,
            span,
        } => {
            let compiled = compile_expression(element, catalog)?;
            Ok(CompiledRepresentation {
                value_type: StructuredInfoType::bounded_sequence(
                    compiled.value_type,
                    *minimum_items,
                    *maximum_items,
                )
                .map_err(|error| bounded(*span, error))?,
                contracts: prefix_contracts(compiled.contracts, "[]"),
            })
        }
    }
}

fn primitive_representation_kind(value_type: &StructuredInfoType) -> Option<&KindId> {
    match value_type.shape() {
        conduit_core::StructuredInfoTypeShape::Leaf(kind) => Some(kind),
        conduit_core::StructuredInfoTypeShape::Nominal { representation, .. } => {
            primitive_representation_kind(representation)
        }
        _ => None,
    }
}

fn prefix_contracts(
    contracts: Vec<NativeTypeValueContract>,
    prefix: &str,
) -> Vec<NativeTypeValueContract> {
    contracts
        .into_iter()
        .map(|contract| NativeTypeValueContract {
            representation_path: alloc::format!("{prefix}{}", contract.representation_path),
            contract: contract.contract,
        })
        .collect()
}

fn schema_identity_for_record(
    name: &str,
    fields: &[StructuredFieldType],
    contracts: &[NativeTypeValueContract],
) -> KindId {
    let mut canonical = b"conduit.native-type.record@1\0".to_vec();
    push(&mut canonical, name.as_bytes());
    for field in fields {
        push(&mut canonical, field.name().as_bytes());
        push(
            &mut canonical,
            &field
                .value_type()
                .canonical_bytes()
                .expect("checked field type"),
        );
    }
    push_contracts(&mut canonical, contracts);
    semantic_id(name, &canonical)
}

fn schema_identity_for_variant(name: &str, cases: &[StructuredVariantCase]) -> KindId {
    let mut canonical = b"conduit.native-type.variant@1\0".to_vec();
    push(&mut canonical, name.as_bytes());
    for case in cases {
        push(&mut canonical, case.tag().as_bytes());
        push(
            &mut canonical,
            &case
                .payload_type()
                .canonical_bytes()
                .expect("checked case type"),
        );
    }
    semantic_id(name, &canonical)
}

fn schema_identity(
    name: &str,
    representation: &StructuredInfoType,
    contracts: &[NativeTypeValueContract],
) -> KindId {
    let mut canonical = b"conduit.native-type.scalar@1\0".to_vec();
    push(&mut canonical, name.as_bytes());
    push(
        &mut canonical,
        &representation
            .canonical_bytes()
            .expect("checked representation"),
    );
    push_contracts(&mut canonical, contracts);
    semantic_id(name, &canonical)
}

fn push_contracts(canonical: &mut Vec<u8>, contracts: &[NativeTypeValueContract]) {
    for contract in contracts {
        push(canonical, contract.representation_path.as_bytes());
        push(canonical, &contract.contract.identity_bytes());
    }
}

fn push(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(bytes);
}

fn semantic_id(name: &str, canonical: &[u8]) -> KindId {
    let digest = Sha256::digest(canonical);
    let mut suffix = String::with_capacity(64);
    for byte in digest {
        use core::fmt::Write;
        write!(&mut suffix, "{byte:02x}").expect("writing to String cannot fail");
    }
    KindId::from(alloc::format!("type/{name}@{suffix}"))
}

fn bounded(span: crate::Span, error: conduit_core::StructuredInfoRefusal) -> SyntaxCheckDiagnostic {
    diagnostic(
        span,
        alloc::format!("native semantic Type exceeds finite checked bounds: {error:?}"),
    )
}

fn diagnostic(span: crate::Span, message: String) -> SyntaxCheckDiagnostic {
    SyntaxCheckDiagnostic {
        code: "CND-FRM-058",
        span,
        message,
    }
}
