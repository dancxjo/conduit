//! Inline variant records retain checked owners without creating Source aliases.
use super::*;
pub(super) fn compile(
    declaration: &TypeSyntax,
    semantic_name: &str,
    catalog: &mut StartupCatalog,
) -> Result<CompiledRepresentation, SyntaxCheckDiagnostic> {
    let TypeDefinitionSyntax::Variant(cases) = &declaration.definition else {
        unreachable!("variant dispatch")
    };
    let mut compiled_cases = Vec::with_capacity(cases.len());
    let mut contracts = Vec::new();
    for case in cases {
        let payload = match &case.payload {
            TypeVariantPayloadSyntax::Empty => CompiledRepresentation {
                value_type: StructuredInfoType::leaf(kind_id("value/empty"))
                    .map_err(|error| bounded(case.span, error))?,
                contracts: Vec::new(),
            },
            TypeVariantPayloadSyntax::Type(expression) => compile_expression(expression, catalog)?,
            TypeVariantPayloadSyntax::Record(fields) => {
                let name = alloc::format!("{}/{}", semantic_name, case.tag.text);
                let record = compile_record(
                    &name,
                    declaration.generic_context.as_deref(),
                    fields,
                    &[],
                    catalog,
                    case.span,
                )?;
                let conduit_core::StructuredInfoTypeShape::Record { schema, .. } =
                    record.value_type.shape()
                else {
                    unreachable!("record compiler")
                };
                catalog
                    .retain_native_dependency(CheckedNativeType {
                        name,
                        identity: schema.clone(),
                        value_type: record.value_type.clone(),
                        value_contracts: record.contracts.clone(),
                        invariants: Vec::new(),
                    })
                    .map_err(|message| diagnostic(case.span, message))?;
                record
            }
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
        semantic_name,
        declaration.generic_context.as_deref(),
        &compiled_cases,
    );
    Ok(CompiledRepresentation {
        value_type: StructuredInfoType::variant(identity, compiled_cases)
            .map_err(|error| bounded(declaration.span, error))?,
        contracts,
    })
}
