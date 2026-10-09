use crate::prelude::*;
use crate::{
    CheckedNativeType, NativeTypeValueContract, StartupCatalog, SyntaxCheckDiagnostic,
    TypeDefinitionSyntax, TypeExpressionSyntax, TypeFieldSyntax, TypeSyntax,
    TypeVariantPayloadSyntax,
};
use alloc::collections::BTreeSet;
use conduit_core::{
    data_reference_kind, kind_id, CheckedValueContract, KindId, StructuredFieldType,
    StructuredInfoType, StructuredVariantCase,
};
mod generic;
mod identity;
mod invariant;
mod use_contract;
use identity::{schema_identity, schema_identity_for_record, schema_identity_for_variant};
pub(crate) use use_contract::{install_import_aliases, validate_concrete_value};

#[derive(Clone)]
struct CompiledRepresentation {
    value_type: StructuredInfoType,
    contracts: Vec<NativeTypeValueContract>,
}

pub(crate) fn check_native_types(
    declarations: &[TypeSyntax],
    base: &StartupCatalog,
) -> Result<(Vec<CheckedNativeType>, StartupCatalog), SyntaxCheckDiagnostic> {
    let (declarations, public_names) = generic::instantiate(declarations)?;
    let mut catalog = base.clone();
    let mut by_name = alloc::collections::BTreeMap::new();
    for declaration in &declarations {
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
    for declaration in &declarations {
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
        .filter(|declaration| public_names.contains(&declaration.name.text))
        .map(|declaration| {
            compiled
                .remove(declaration.name.text.as_str())
                .expect("every declaration compiled exactly once")
        })
        .collect();
    Ok((checked, catalog))
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
            compiled.invariants.clone(),
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
                match &case.payload {
                    TypeVariantPayloadSyntax::Unit => {}
                    TypeVariantPayloadSyntax::Type(expression) => {
                        expression_references(expression, out);
                    }
                    TypeVariantPayloadSyntax::Record(fields) => {
                        for field in fields {
                            expression_references(&field.value_type, out);
                        }
                    }
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
        TypeExpressionSyntax::Collection { element, .. } => expression_references(element, out),
    }
}

fn compile_definition(
    declaration: &TypeSyntax,
    catalog: &StartupCatalog,
) -> Result<CheckedNativeType, SyntaxCheckDiagnostic> {
    let compiled = match &declaration.definition {
        TypeDefinitionSyntax::Scalar(expression) => compile_expression(expression, catalog)?,
        TypeDefinitionSyntax::Record(fields) => compile_record(
            &declaration.name.text,
            declaration.generic_context.as_deref(),
            fields,
            &declaration.invariants,
            catalog,
            declaration.span,
        )?,
        TypeDefinitionSyntax::Variant(cases) => {
            let mut compiled_cases = Vec::with_capacity(cases.len());
            let mut contracts = Vec::new();
            for case in cases {
                let payload = match &case.payload {
                    TypeVariantPayloadSyntax::Unit => CompiledRepresentation {
                        value_type: StructuredInfoType::leaf(kind_id("value/unit"))
                            .map_err(|error| bounded(case.span, error))?,
                        contracts: Vec::new(),
                    },
                    TypeVariantPayloadSyntax::Type(expression) => {
                        compile_expression(expression, catalog)?
                    }
                    TypeVariantPayloadSyntax::Record(fields) => compile_record(
                        &alloc::format!("{}/{}", declaration.name.text, case.tag.text),
                        declaration.generic_context.as_deref(),
                        fields,
                        &[],
                        catalog,
                        case.span,
                    )?,
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
            let identity = schema_identity_for_variant(
                &declaration.name.text,
                declaration.generic_context.as_deref(),
                &compiled_cases,
            );
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
                declaration.generic_context.as_deref(),
                &compiled.value_type,
                &compiled.contracts,
                &declaration.invariants,
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
    let invariants = invariant::compile(declaration, &value_type, catalog)?;
    Ok(CheckedNativeType {
        name: declaration.name.text.clone(),
        identity,
        value_type,
        value_contracts: compiled.contracts,
        invariants,
    })
}

fn compile_record(
    semantic_name: &str,
    generic_context: Option<&str>,
    fields: &[TypeFieldSyntax],
    invariants: &[crate::Expression],
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
    let identity = schema_identity_for_record(
        semantic_name,
        generic_context,
        &compiled_fields,
        &contracts,
        invariants,
    );
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
            arguments,
            maximum_bytes,
            refinements,
            span,
        } => {
            debug_assert!(
                arguments.is_empty(),
                "generic applications are resolved before checking"
            );
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
                    minimum_items.literal_value().ok_or_else(|| {
                        diagnostic(
                            minimum_items.span(),
                            "native sequence minimum is not closed".into(),
                        )
                    })?,
                    maximum_items.literal_value().ok_or_else(|| {
                        diagnostic(
                            maximum_items.span(),
                            "native sequence maximum is not closed".into(),
                        )
                    })?,
                )
                .map_err(|error| bounded(*span, error))?,
                contracts: prefix_contracts(compiled.contracts, "[]"),
            })
        }
        TypeExpressionSyntax::Collection {
            element,
            length,
            span,
        } => {
            let compiled = compile_expression(element, catalog)?;
            Ok(CompiledRepresentation {
                value_type: StructuredInfoType::collection(
                    compiled.value_type,
                    Some(length.literal_value().ok_or_else(|| {
                        diagnostic(
                            length.span(),
                            "native collection extent is not closed".into(),
                        )
                    })?),
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
