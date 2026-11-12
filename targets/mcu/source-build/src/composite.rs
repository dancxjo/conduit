//! Build-host specialization of existing bounded composite owners.
//! Explicit named Types and envelopes belong to fabrication, never runtime grants.
use conduit_core as c;
use conduit_plot as p;

pub struct ValueReference<'a> {
    pub type_name: &'a str,
    pub maximum_bytes: u32,
}
pub enum Specialization<'a> {
    SeededUntil(ValueReference<'a>),
    Merge(ValueReference<'a>),
    Zip {
        left: ValueReference<'a>,
        right: ValueReference<'a>,
        feedback: bool,
    },
}

#[derive(Clone)]
pub struct Value {
    pub contract: c::CheckedValueContract,
    pub schema: c::StructuredInfoType,
}
pub enum Role {
    SeededUntil(Value),
    Merge(Value),
    Zip {
        left: Value,
        right: Value,
        feedback: bool,
    },
}

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
        !matches!(
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
            Specialization::Merge(v) => {
                let v = value(v)?;
                conduit_semantic_catalog::install_flow_merge_finite_kind(
                    &v.contract,
                    &v.schema,
                    startup,
                    profiles,
                )?;
            }
            Specialization::Zip {
                left,
                right,
                feedback,
            } => {
                let (l, r) = (value(left)?, value(right)?);
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

pub(crate) fn selected(
    gear: &p::CheckedGear,
    types: Option<&p::CheckedSyntaxDocument>,
) -> Result<Option<(c::Kind, Role)>, String> {
    let name = gear.kind_id.as_str();
    if ![
        "state/seeded/flow/until",
        "flow/merge/finite",
        "flow/zip/finite",
        "flow/zip/feedback",
    ]
    .contains(&name)
    {
        return Ok(None);
    }
    let types = types.ok_or("composite software requires checked Source Type custody")?;
    let input = |name: &str| -> Result<Value, String> {
        let port = gear
            .inputs
            .iter()
            .find(|port| port.port_id.as_str() == name)
            .ok_or("composite input port")?;
        let contract = gear
            .semantic_contract
            .value_contracts()
            .iter()
            .find(|v| v.location == c::FrontValueLocation::Input(port.port_id.clone()))
            .ok_or("composite input envelope")?
            .contract
            .clone();
        if contract.maximum_bytes == 0 || contract.maximum_bytes > 4096 {
            return Err("MCU composite input envelope".into());
        }
        let schema = types
            .structured_type(&contract.value_kind)
            .cloned()
            .ok_or("composite schema custody")?;
        Ok(Value { contract, schema })
    };
    let (kind, role) = match name {
        "state/seeded/flow/until" => {
            let v = input("seed")?;
            conduit_composite::SeededStateBack::prepare_until(&v.contract, &v.schema)
                .map_err(str::to_owned)?;
            (
                conduit_semantic_catalog::seeded_state_until_semantic_contract(
                    &v.contract,
                    &v.schema,
                )
                .map_err(str::to_owned)?,
                Role::SeededUntil(v),
            )
        }
        "flow/merge/finite" => {
            let v = input("left")?;
            conduit_composite::FlowMergeFiniteBack::prepare(&v.schema, v.contract.maximum_bytes)
                .map_err(debug)?;
            (
                conduit_semantic_catalog::flow_merge_finite_semantic_contract(
                    &v.contract,
                    &v.schema,
                )
                .map_err(str::to_owned)?,
                Role::Merge(v),
            )
        }
        _ => {
            let (left, right) = (input("left")?, input("right")?);
            let feedback = name == "flow/zip/feedback";
            let build = if feedback {
                conduit_composite::FlowZipBack::prepare_typed_feedback
            } else {
                conduit_composite::FlowZipBack::prepare_typed_finite
            };
            build(
                &left.contract,
                left.schema.clone(),
                &right.contract,
                right.schema.clone(),
            )
            .map_err(debug)?;
            let semantic = if feedback {
                conduit_semantic_catalog::flow_zip_feedback_semantic_contract
            } else {
                conduit_semantic_catalog::flow_zip_finite_semantic_contract
            };
            (
                semantic(&left.contract, &left.schema, &right.contract, &right.schema)
                    .map_err(str::to_owned)?,
                Role::Zip {
                    left,
                    right,
                    feedback,
                },
            )
        }
    };
    Ok(Some((kind, role)))
}

fn debug(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}

pub(crate) fn emit_definition(role: &Role) -> Result<String, String> {
    let schema = |value: &Value| -> Result<String, String> {
        if !value.contract.constraints.is_empty() {
            return Err("composite constrained contract emission unsupported".into());
        }
        Ok(format!(
            "&{:?}",
            value.schema.canonical_bytes().map_err(debug)?
        ))
    };
    Ok(match role {
        Role::SeededUntil(v) => format!("D::SeededUntilStatic{{schema:{},maximum_bytes:{}}}", schema(v)?, v.contract.maximum_bytes),
        Role::Merge(v) => format!("D::MergeStatic{{schema:{},maximum_bytes:{}}}", schema(v)?, v.contract.maximum_bytes),
        Role::Zip { left, right, feedback } => format!("D::ZipStatic{{left_schema:{},left_maximum_bytes:{},right_schema:{},right_maximum_bytes:{},feedback:{feedback}}}", schema(left)?, left.contract.maximum_bytes, schema(right)?, right.contract.maximum_bytes),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    #[test]
    fn checked_composites_emit_exact_schema_images_and_distinct_zip_laws() {
        let source = include_bytes!("../tests/fixtures/composites.conduit");
        let frame = || ValueReference {
            type_name: "Frame",
            maximum_bytes: 1024,
        };
        let entries = [
            "state-client",
            "merge-client",
            "zip-client",
            "feedback-client",
        ]
        .map(String::from);
        let requests = [
            Specialization::SeededUntil(frame()),
            Specialization::Merge(frame()),
            Specialization::Zip {
                left: frame(),
                right: frame(),
                feedback: false,
            },
            Specialization::Zip {
                left: frame(),
                right: frame(),
                feedback: true,
            },
        ];
        assert!(crate::workset::compile_specialized(
            source,
            &format!("{:x}", Sha256::digest(source)),
            &entries,
            None,
            &[Specialization::SeededUntil(ValueReference {
                type_name: "Frame",
                maximum_bytes: 1
            })],
        )
        .err()
        .unwrap()
        .contains("admitted envelope"));
        let workset = crate::workset::compile_specialized(
            source,
            &format!("{:x}", Sha256::digest(source)),
            &entries,
            None,
            &requests,
        )
        .unwrap();
        for entry in &workset.entries {
            assert!(entry.rust.contains("Static{"));
            assert!(
                !entry
                    .rust
                    .contains("StructuredInfoType::from_canonical_bytes"),
                "rich schema decoding belongs inside measured runtime construction"
            );
            let material = crate::software::compile_with_types(
                &entry.plot.expanded,
                Some(&workset.checked_source),
                24 * 1024,
            )
            .unwrap();
            assert_eq!(material.nodes.len(), 1);
            assert!(material.nodes[0].offer_image.is_some());
        }
        assert!(workset.entries[2].rust.contains("feedback:false"));
        assert!(workset.entries[3].rust.contains("feedback:true"));
        assert!(crate::workset::compile(
            source,
            &format!("{:x}", Sha256::digest(source)),
            &entries,
            None
        )
        .is_err());
    }
}
