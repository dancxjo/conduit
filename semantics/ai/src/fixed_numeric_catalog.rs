//! Owned generic fixed numeric port contracts. No network ordering or speech policy.
use alloc::{format, string::String, vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, CheckedFront, CheckedValueContract, FrontValueContract,
    FrontValueLocation, Kind, KindIdentity, KindSemanticLaw, PortDescriptor, PortDirection,
    PortTemporal, StructuredInfoType,
};
use conduit_plot::{
    check_syntax_document, parse_syntax_document, CheckedNativeType, KindSignature, ProfileCatalog,
    StartupCatalog,
};

pub const FIXED_NUMERIC_SOURCE: &str = include_str!("../fixed_numeric.conduit");
pub const FIXED_NUMERIC_REVISION: &str = "conduit.numeric/fixed-f32-libm@2";

pub fn fixed_numeric_types() -> Result<Vec<CheckedNativeType>, String> {
    let mut catalog = StartupCatalog::new();
    catalog.insert_value_kind_alias(
        "ResourceRef",
        kind_id(conduit_core::RESOURCE_REFERENCE_INFO_ID),
    )?;
    check_syntax_document(&parse_syntax_document(FIXED_NUMERIC_SOURCE), &catalog)
        .map(|document| document.native_types)
        .map_err(|error| format!("{}: {}", error.code, error.message))
}

pub fn fixed_numeric_type(name: &str) -> Result<StructuredInfoType, String> {
    fixed_numeric_types()?
        .into_iter()
        .find(|ty| ty.name == name)
        .map(|ty| ty.value_type)
        .ok_or_else(|| format!("unknown fixed numeric Type {name}"))
}

pub fn fixed_numeric_contracts() -> Result<Vec<Kind>, String> {
    let types = fixed_numeric_types()?;
    let port = |name: &str, ty: &str, direction| -> Result<PortDescriptor, String> {
        let value_kind = if ty == "U16" {
            kind_id("value/u16")
        } else {
            types
                .iter()
                .find(|item| item.name == ty)
                .ok_or_else(|| format!("absent Type {ty}"))?
                .value_type
                .profile()
                .map_err(|e| format!("{e:?}"))?
                .value_kind()
                .clone()
        };
        Ok(PortDescriptor {
            port_id: port_id(name),
            value_kind,
            direction,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        })
    };
    let specs = [
        (
            "numeric/embedding224x12",
            vec![("index", "U16"), ("weights", "NumericEmbedding224x12")],
            vec![("result", "NumericF32Vector12")],
        ),
        (
            "numeric/concatenate20x12",
            vec![
                ("left", "NumericF32Vector20"),
                ("right", "NumericF32Vector12"),
            ],
            vec![("result", "NumericF32Vector32")],
        ),
        (
            "numeric/dense3x2",
            vec![
                ("value", "NumericF32Vector3"),
                ("weights", "NumericF32MatrixRef3x2"),
                ("bias", "NumericF32BiasRef2"),
            ],
            vec![("result", "NumericF32Vector2")],
        ),
        (
            "numeric/dense32x64",
            vec![
                ("value", "NumericF32Vector32"),
                ("weights", "NumericF32MatrixRef32x64"),
                ("bias", "NumericF32BiasRef64"),
            ],
            vec![("result", "NumericF32Vector64")],
        ),
        (
            "numeric/dense192x128",
            vec![
                ("value", "NumericF32Vector192"),
                ("weights", "NumericF32MatrixRef192x128"),
                ("bias", "NumericF32BiasRef128"),
            ],
            vec![("result", "NumericF32Vector128")],
        ),
        (
            "numeric/dense128x320",
            vec![
                ("value", "NumericF32Vector128"),
                ("weights", "NumericF32MatrixRef128x320"),
                ("bias", "NumericF32BiasRef320"),
            ],
            vec![("result", "NumericF32Vector320")],
        ),
        (
            "numeric/tanh64",
            vec![("value", "NumericF32Vector64")],
            vec![("result", "NumericF32Vector64")],
        ),
        (
            "numeric/tanh128",
            vec![("value", "NumericF32Vector128")],
            vec![("result", "NumericF32Vector128")],
        ),
        (
            "numeric/tanh320",
            vec![("value", "NumericF32Vector320")],
            vec![("result", "NumericF32Vector320")],
        ),
        (
            "numeric/history2x64",
            vec![
                ("value", "NumericF32Vector64"),
                ("history", "NumericHistory2x64"),
            ],
            vec![("result", "NumericWindow2x64")],
        ),
    ];
    specs
        .into_iter()
        .map(|(name, inputs, outputs)| {
            let inputs: Vec<_> = inputs
                .into_iter()
                .map(|(n, t)| port(n, t, PortDirection::Input))
                .collect::<Result<_, _>>()?;
            let outputs: Vec<_> = outputs
                .into_iter()
                .map(|(n, t)| port(n, t, PortDirection::Output))
                .collect::<Result<_, _>>()?;
            // Each queue and value has a finite envelope. Resource bytes remain
            // separately admitted and borrowed; they are not copied into queues.
            let contracts = inputs
                .iter()
                .chain(&outputs)
                .map(|p| FrontValueContract {
                    location: match p.direction {
                        PortDirection::Input => FrontValueLocation::Input(p.port_id.clone()),
                        PortDirection::Output => FrontValueLocation::Output(p.port_id.clone()),
                    },
                    contract: CheckedValueContract::new(p.value_kind.clone(), 16_384, vec![])
                        .expect("finite numeric port envelope"),
                })
                .collect();
            Ok(Kind {
                kind_id: kind_id(name),
                kind_contract_revision: KindIdentity::from(format!(
                    "{FIXED_NUMERIC_REVISION}/{name}"
                )),
                startup_parameters: vec![],
                shorthand: None,
                configuration: vec![],
                inputs,
                outputs,
                semantic_laws: vec![KindSemanticLaw::ValueContracts(contracts)],
                limits: CapabilityLimits {
                    max_active_instances: 16,
                    max_queue_items: 1,
                    max_queue_bytes: 16_384,
                },
            })
        })
        .collect()
}

pub fn install_fixed_numeric_catalogs(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    startup.insert_value_kind_alias(
        "ResourceRef",
        kind_id(conduit_core::RESOURCE_REFERENCE_INFO_ID),
    )?;
    for ty in fixed_numeric_types()? {
        startup.insert_structured_type(ty.name, ty.value_type)?;
    }
    for kind in fixed_numeric_contracts()? {
        startup.insert(KindSignature {
            kind: kind.kind_id.as_str().into(),
            startup_parameters: vec![],
        })?;
        startup.insert_fore(
            kind.kind_id.as_str(),
            CheckedFront::new(vec![], kind.inputs.clone(), kind.outputs.clone(), None),
        )?;
        profile
            .insert_kind(kind)
            .map_err(|error| format!("{error:?}"))?;
    }
    Ok(())
}
