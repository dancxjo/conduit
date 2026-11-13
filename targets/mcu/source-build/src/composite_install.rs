//! Inert typed family catalogue preparation on the fabrication Host.
use super::*;

pub(crate) fn install(
    syntax: &p::SyntaxDocument,
    requests: &[Specialization<'_>],
    startup: &mut p::StartupCatalog,
    profiles: &mut p::ProfileCatalog,
) -> Result<(), String> {
    if requests.len() > 16 {
        return Err("composite specialization count".into());
    }
    if requests.is_empty() {
        return Ok(());
    }
    // Paired output aliases are produced by the specialization below. They
    // cannot be required to exist while preparing its input declarations.
    // The original, unmodified document (including every import) is fully
    // checked after installation; this temporary view confers no Plot authority.
    let mut declarations = syntax.clone();
    declarations.uses.retain(|import| {
        typed_family(&import.path).is_none()
            && !matches!(
                import.path.as_str(),
                "flow/zip/finite/paired" | "flow/zip/feedback/paired"
            )
    });
    let types = p::prepare_source_types(&declarations, startup)
        .map_err(|e| format!("composite Types: {e:?}"))?;
    let value = |reference: &ValueReference<'_>| -> Result<Value, String> {
        if reference.maximum_bytes == 0 || reference.maximum_bytes > 4096 {
            return Err(
                "MCU composite envelope exceeds selected 4096-byte construction profile".into(),
            );
        }
        let schema = types
            .named_type(reference.type_name)
            .ok_or("unknown composite Source Type")?
            .clone();
        let contract = c::CheckedValueContract::new(
            schema.profile().map_err(debug)?.value_kind().clone(),
            reference.maximum_bytes,
            vec![],
        )
        .map_err(debug)?;
        Ok(Value { contract, schema })
    };
    for request in requests {
        match request {
            Specialization::SeededUntil(v) => {
                let v = value(v)?;
                conduit_semantic_catalog::install_seeded_state_until_kind(
                    &v.contract,
                    &v.schema,
                    startup,
                    profiles,
                )?;
            }
            Specialization::SeededUntilInstance(v) => {
                let v = value(v)?;
                let mut kind = conduit_semantic_catalog::seeded_state_until_semantic_contract(
                    &v.contract,
                    &v.schema,
                )
                .map_err(str::to_owned)?;
                kind.kind_id = c::typed_kind_instance_id(
                    kind.kind_id.as_str(),
                    core::iter::once((&v.schema, &v.contract)),
                )
                .map_err(debug)?;
                crate::workset::install(kind, startup, profiles)?;
            }
            Specialization::Merge(v) => {
                let v = value(v)?;
                conduit_semantic_catalog::install_flow_merge_finite_kind(
                    &v.contract,
                    &v.schema,
                    startup,
                    profiles,
                )?;
            }
            Specialization::MergeInstance(v) => {
                let v = value(v)?;
                let mut kind = conduit_semantic_catalog::flow_merge_finite_semantic_contract(
                    &v.contract,
                    &v.schema,
                )
                .map_err(str::to_owned)?;
                kind.kind_id = c::typed_kind_instance_id(
                    kind.kind_id.as_str(),
                    core::iter::once((&v.schema, &v.contract)),
                )
                .map_err(debug)?;
                crate::workset::install(kind, startup, profiles)?;
            }
            Specialization::Zip {
                left,
                right,
                feedback,
            }
            | Specialization::ZipInstance {
                left,
                right,
                feedback,
            } => {
                let (l, r) = (value(left)?, value(right)?);
                if matches!(request, Specialization::ZipInstance { .. }) {
                    let semantic = if *feedback {
                        conduit_semantic_catalog::flow_zip_feedback_semantic_contract
                    } else {
                        conduit_semantic_catalog::flow_zip_finite_semantic_contract
                    };
                    let mut kind = semantic(&l.contract, &l.schema, &r.contract, &r.schema)
                        .map_err(str::to_owned)?;
                    kind.kind_id = c::typed_kind_instance_id(
                        kind.kind_id.as_str(),
                        [(&l.schema, &l.contract), (&r.schema, &r.contract)].into_iter(),
                    )
                    .map_err(debug)?;
                    let paired = c::PreparedTypedTuplePairEncoder::new(
                        l.schema.clone(),
                        l.contract.maximum_bytes,
                        r.schema.clone(),
                        r.contract.maximum_bytes,
                    )
                    .map_err(debug)?;
                    startup.insert_structured_type(
                        format!("{}/paired", kind.kind_id.as_str()),
                        paired.value_type().clone(),
                    )?;
                    crate::workset::install(kind, startup, profiles)?;
                    continue;
                }
                let install = if *feedback {
                    conduit_semantic_catalog::install_flow_zip_feedback_kind
                } else {
                    conduit_semantic_catalog::install_flow_zip_finite_kind
                };
                install(
                    &l.contract,
                    &l.schema,
                    &r.contract,
                    &r.schema,
                    startup,
                    profiles,
                )?;
            }
        }
    }
    Ok(())
}
