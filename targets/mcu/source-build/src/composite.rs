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
    SeededUntilInstance(ValueReference<'a>),
    Merge(ValueReference<'a>),
    /// Explicit canonical typed-family import; no hidden Source rewrite.
    MergeInstance(ValueReference<'a>),
    ZipInstance {
        left: ValueReference<'a>,
        right: ValueReference<'a>,
        feedback: bool,
    },
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

#[path = "composite_install.rs"]
mod construction;
pub(crate) use construction::install;

fn typed_family(name: &str) -> Option<&'static str> {
    [
        "state/seeded/flow/until",
        "flow/merge/finite",
        "flow/zip/finite",
        "flow/zip/feedback",
    ]
    .into_iter()
    .find(|family| {
        name.strip_prefix(family)
            .is_some_and(|tail| tail.starts_with("/typed-"))
    })
}

pub(crate) fn selected(
    gear: &p::CheckedGear,
    types: Option<&p::CheckedSyntaxDocument>,
) -> Result<Option<(c::Kind, Role)>, String> {
    let selected_name = gear.kind_id.as_str();
    let typed = typed_family(selected_name);
    let name = typed.unwrap_or(selected_name);
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
    let (mut kind, role) = match name {
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
    if let Some(family) = typed {
        kind.kind_id = match &role {
            Role::SeededUntil(value) | Role::Merge(value) => c::typed_kind_instance_id(
                family,
                core::iter::once((&value.schema, &value.contract)),
            ),
            Role::Zip { left, right, .. } => c::typed_kind_instance_id(
                family,
                [
                    (&left.schema, &left.contract),
                    (&right.schema, &right.contract),
                ]
                .into_iter(),
            ),
        }
        .map_err(debug)?;
        if kind.kind_id.as_str() != selected_name {
            return Err("typed composite identity differs from checked schemas/contracts".into());
        }
    }
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
    fn authored_bme_protocol_compiles_through_shared_mcu_source() {
        let units = [
            include_str!("../../../../plots/device-protocols/bme280-lifecycle.conduit"),
            // This entry needs the shared join, not the separate externally
            // driven finite-feedback demonstration in the same source file.
            include_str!("../../../../plots/device-protocols/bme280-feedback.conduit")
                .split("\nplot bme280-feedback")
                .next()
                .unwrap(),
            include_str!("../../../../plots/device-protocols/bme280-clock-events.conduit"),
            include_str!("../../../../plots/device-protocols/main.conduit"),
            include_str!("../../../../plots/device-protocols/bme280-capture-observation.conduit"),
            include_str!("../../../../plots/device-protocols/bme280-protocol-frame.conduit"),
            include_str!("../../../../plots/device-protocols/bme280-autonomous.conduit"),
        ];
        let imports = units
            .iter()
            .flat_map(|s| s.lines())
            .filter(|s| s.starts_with("with "))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join("\n");
        let bodies = units
            .iter()
            .flat_map(|s| s.lines())
            .filter(|s| !s.starts_with("with "))
            .collect::<Vec<_>>()
            .join("\n");
        let source = format!("{imports}\n{bodies}");
        let reference = |type_name, maximum_bytes| ValueReference {
            type_name,
            maximum_bytes,
        };
        let requests = [
            Specialization::SeededUntil(reference("BmeProtocolFrame", 4096)),
            Specialization::Zip {
                left: reference("BmeProtocolState", 4096),
                right: reference("BmeProtocolEvent", 4096),
                feedback: true,
            },
            Specialization::Merge(reference("BmeClockContext", 4096)),
            Specialization::Zip {
                left: reference("BmeClockContext", 4096),
                right: reference("ClockResult", 512),
                feedback: false,
            },
        ];
        let workset = crate::workset::compile_specialized(
            source.as_bytes(),
            &format!("{:x}", Sha256::digest(source.as_bytes())),
            &["bme280-autonomous".into()],
            None,
            &requests,
        )
        .unwrap();
        assert_eq!(workset.entries.len(), 1);
        assert!(workset.entries[0].nodes > 40);
    }

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

#[cfg(test)]
#[path = "correlated_i2c_tests.rs"]
mod correlated_i2c_tests;

#[cfg(test)]
#[path = "composite_instance_tests.rs"]
mod instance_tests;

#[cfg(test)]
#[path = "composite_zip_instance_tests.rs"]
mod zip_instance_tests;
