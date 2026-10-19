//! Owned generic fixed numeric port contracts. No network ordering or speech policy.
use alloc::{collections::BTreeMap, format, string::String, vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, CheckedValueContract, FrontValueContract,
    FrontValueLocation, Kind, KindIdentity, KindSemanticLaw, PortDescriptor, PortDirection,
    PortTemporal, StructuredInfoType,
};
use conduit_plot::{
    check_syntax_document, parse_syntax_document, CheckedNativeType, KindSignature, ProfileCatalog,
    StartupCatalog,
};

mod pointwise;
pub(crate) use pointwise::is_pure_pointwise;

pub const FIXED_NUMERIC_SOURCE: &str = include_str!("../fixed_numeric.conduit");
pub const FIXED_NUMERIC_SIGNAL_SOURCE: &str = include_str!("../fixed_numeric_signal.conduit");
pub const FIXED_NUMERIC_REVISION: &str = "conduit.numeric/fixed-f32-libm@3";

#[cfg(feature = "hosted-catalog-cache")]
fn cached_fixed_numeric_types() -> &'static Result<Vec<CheckedNativeType>, String> {
    static CATALOG: std::sync::OnceLock<Result<Vec<CheckedNativeType>, String>> =
        std::sync::OnceLock::new();
    CATALOG.get_or_init(fixed_numeric_types_uncached)
}
pub fn fixed_numeric_types() -> Result<Vec<CheckedNativeType>, String> {
    #[cfg(feature = "hosted-catalog-cache")]
    {
        cached_fixed_numeric_types().clone()
    }
    #[cfg(not(feature = "hosted-catalog-cache"))]
    fixed_numeric_types_uncached()
}
fn fixed_numeric_types_uncached() -> Result<Vec<CheckedNativeType>, String> {
    let mut catalog = StartupCatalog::new();
    catalog.insert_value_kind_alias(
        "ResourceRef",
        kind_id(conduit_core::RESOURCE_REFERENCE_INFO_ID),
    )?;
    check_syntax_document(
        &parse_syntax_document(&format!(
            "{FIXED_NUMERIC_SOURCE}\n{FIXED_NUMERIC_SIGNAL_SOURCE}"
        )),
        &catalog,
    )
    .map(|document| document.native_types)
    .map_err(|error| format!("{}: {}", error.code, error.message))
}

pub fn fixed_numeric_type(name: &str) -> Result<StructuredInfoType, String> {
    #[cfg(feature = "hosted-catalog-cache")]
    {
        cached_fixed_numeric_types()
            .as_ref()
            .map_err(Clone::clone)?
            .iter()
            .find(|ty| ty.name == name)
            .map(|ty| ty.value_type.clone())
            .ok_or_else(|| format!("unknown fixed numeric Type {name}"))
    }
    #[cfg(not(feature = "hosted-catalog-cache"))]
    fixed_numeric_types()?
        .into_iter()
        .find(|ty| ty.name == name)
        .map(|ty| ty.value_type)
        .ok_or_else(|| format!("unknown fixed numeric Type {name}"))
}

#[cfg(feature = "hosted-catalog-cache")]
fn cached_fixed_numeric_contracts() -> &'static Result<Vec<Kind>, String> {
    static CATALOG: std::sync::OnceLock<Result<Vec<Kind>, String>> = std::sync::OnceLock::new();
    CATALOG.get_or_init(fixed_numeric_contracts_uncached)
}
pub fn fixed_numeric_contracts() -> Result<Vec<Kind>, String> {
    #[cfg(feature = "hosted-catalog-cache")]
    {
        cached_fixed_numeric_contracts().clone()
    }
    #[cfg(not(feature = "hosted-catalog-cache"))]
    fixed_numeric_contracts_uncached()
}
/// Select one owned contract without cloning the rest of the immutable catalogue.
/// Both lookup paths retain the same checked compiled Source and error result.
pub(crate) fn fixed_numeric_contract(identity: &str) -> Result<Option<Kind>, String> {
    #[cfg(feature = "hosted-catalog-cache")]
    {
        Ok(cached_fixed_numeric_contracts()
            .as_ref()
            .map_err(Clone::clone)?
            .iter()
            .find(|kind| kind.kind_id.as_str() == identity)
            .cloned())
    }
    #[cfg(not(feature = "hosted-catalog-cache"))]
    Ok(fixed_numeric_contracts_uncached()?
        .into_iter()
        .find(|kind| kind.kind_id.as_str() == identity))
}
fn fixed_numeric_contracts_uncached() -> Result<Vec<Kind>, String> {
    let types = fixed_numeric_types()?;
    let mut bounds: BTreeMap<_, u32> = types
        .iter()
        .map(|ty| {
            Ok((
                ty.value_type
                    .profile()
                    .map_err(|e| format!("{e:?}"))?
                    .value_kind()
                    .clone(),
                conduit_plot::maximum_prepared_canonical_value_bytes(&ty.value_type)
                    .map_err(|e| format!("{e:?}"))?,
            ))
        })
        .collect::<Result<_, String>>()?;
    bounds.insert(kind_id("value/u16"), 2);
    bounds.insert(kind_id("value/u64"), 8);
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
            vec![("result", "NumericWindowResult2x64")],
        ),
    ];
    let mut specs: Vec<crate::fixed_numeric_signal_catalog::Spec> = specs
        .into_iter()
        .map(|(n, inputs, outputs)| {
            (
                n.into(),
                inputs
                    .into_iter()
                    .map(|(n, t)| (n.into(), t.into()))
                    .collect(),
                outputs
                    .into_iter()
                    .map(|(n, t)| (n.into(), t.into()))
                    .collect(),
            )
        })
        .collect();
    for spec in crate::fixed_numeric_signal_catalog::fixed_signal_specs() {
        if !specs.iter().any(|old| old.0 == spec.0) {
            specs.push(spec);
        }
    }
    specs
        .into_iter()
        .map(|(name, inputs, outputs)| {
            let inputs: Vec<_> = inputs
                .into_iter()
                .map(|(n, t)| port(&n, &t, PortDirection::Input))
                .collect::<Result<_, _>>()?;
            let outputs: Vec<_> = outputs
                .into_iter()
                .map(|(n, t)| port(&n, &t, PortDirection::Output))
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
                    contract: CheckedValueContract::new(
                        p.value_kind.clone(),
                        bounds[&p.value_kind],
                        vec![],
                    )
                    .expect("finite numeric port envelope"),
                })
                .collect();
            Ok(Kind {
                kind_id: kind_id(&name),
                kind_contract_revision: KindIdentity::from(pointwise::revision(&name)),
                startup_parameters: vec![],
                shorthand: None,
                configuration: vec![],
                inputs,
                outputs,
                semantic_laws: {
                    let mut laws = vec![KindSemanticLaw::ValueContracts(contracts)];
                    laws.extend(pointwise::laws(&name));
                    laws
                },
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
    install_catalogs_with(startup, profile, Ok)
}
/// Explicit larger instance profile for stateless Value operators.
#[cfg(feature = "kernel-step")]
pub fn install_fixed_numeric_catalogs_capacity64(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    install_catalogs_with(startup, profile, |kind| {
        if crate::fixed_numeric_value_capacity::value_capacity64_implementation(
            kind.kind_id.as_str(),
        )
        .is_some()
        {
            crate::fixed_numeric_value_capacity::value_capacity64_contract(kind)
        } else {
            Ok(kind)
        }
    })
}
fn install_catalogs_with(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
    transform: impl Fn(Kind) -> Result<Kind, String>,
) -> Result<(), String> {
    startup.insert_value_kind_alias(
        "ResourceRef",
        kind_id(conduit_core::RESOURCE_REFERENCE_INFO_ID),
    )?;
    for ty in fixed_numeric_types()? {
        startup.insert_checked_native_type(ty.name.clone(), &ty)?;
    }
    for kind in fixed_numeric_contracts()? {
        let kind = transform(kind)?;
        startup.insert(KindSignature {
            kind: kind.kind_id.as_str().into(),
            startup_parameters: vec![],
        })?;
        startup.insert_fore(kind.kind_id.as_str(), kind.checked_front())?;
        profile
            .insert_kind(kind)
            .map_err(|error| format!("{error:?}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod lookup_tests {
    use super::*;
    #[test]
    fn selected_contracts_and_types_preserve_the_checked_catalogue() {
        for kind in fixed_numeric_contracts().unwrap() {
            assert_eq!(
                fixed_numeric_contract(kind.kind_id.as_str()).unwrap(),
                Some(kind)
            );
        }
        for ty in fixed_numeric_types().unwrap() {
            assert_eq!(fixed_numeric_type(&ty.name).unwrap(), ty.value_type);
        }
        assert!(fixed_numeric_contract("numeric/not-owned")
            .unwrap()
            .is_none());
        assert_eq!(
            fixed_numeric_type("NotOwned"),
            Err("unknown fixed numeric Type NotOwned".into())
        );
    }
    #[test]
    fn narrowing_a_returned_contract_cannot_mutate_the_cached_base() {
        let original = fixed_numeric_contracts().unwrap().remove(0);
        let mut narrowed = fixed_numeric_contract(original.kind_id.as_str())
            .unwrap()
            .unwrap();
        narrowed.limits.max_active_instances = 0;
        assert_ne!(narrowed, original);
        assert_eq!(
            fixed_numeric_contract(original.kind_id.as_str()).unwrap(),
            Some(original)
        );
    }
}
