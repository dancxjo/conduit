//! Exact Value pairing contracts using the existing core typed-pair encoder.
//! Pairing composes proposals; it does not commit a recurrent model state.
use crate::fixed_numeric_catalog::fixed_numeric_type;
use alloc::{format, string::String, vec, vec::Vec};
use conduit_core::*;
use conduit_plot::{
    maximum_prepared_canonical_value_bytes, KindSignature, ProfileCatalog, StartupCatalog,
};

pub fn fixed_numeric_pair_contracts() -> Result<Vec<(String, StructuredInfoType, Kind)>, String> {
    let mut left = fixed_numeric_type("NumericF32Vector40")?;
    let mut result = Vec::new();
    for (identity, alias, right_width) in [
        ("numeric/pair40x164", "NumericPair40x164", 164),
        ("numeric/pair40-164x160", "NumericPair40_164x160", 160),
        (
            "numeric/pair40-164-160x128",
            "NumericPair40_164_160x128",
            128,
        ),
        (
            "numeric/pair40-164-160-128x128",
            "NumericPair40_164_160_128x128",
            128,
        ),
        (
            "numeric/pair40-164-160-128-128x256",
            "NumericPair40_164_160_128_128x256",
            256,
        ),
        (
            "numeric/pair40-164-160-128-128-256x1",
            "NumericPair40_164_160_128_128_256x1",
            1,
        ),
    ] {
        let right = fixed_numeric_type(&format!("NumericF32Vector{right_width}"))?;
        let lmax = maximum_prepared_canonical_value_bytes(&left).map_err(|e| format!("{e:?}"))?;
        let rmax = maximum_prepared_canonical_value_bytes(&right).map_err(|e| format!("{e:?}"))?;
        let encoder = PreparedTypedTuplePairEncoder::new(left.clone(), lmax, right.clone(), rmax)
            .map_err(|e| format!("{e:?}"))?;
        let paired = encoder.value_type().clone();
        let port = |name: &str, ty: &StructuredInfoType, direction| PortDescriptor {
            port_id: port_id(name),
            value_kind: ty.profile().unwrap().value_kind().clone(),
            direction,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        };
        let inputs = vec![
            port("left", &left, PortDirection::Input),
            port("right", &right, PortDirection::Input),
        ];
        let outputs = vec![port("result", &paired, PortDirection::Output)];
        let contracts = inputs
            .iter()
            .chain(&outputs)
            .zip([16_384; 3])
            .map(|(p, max)| FrontValueContract {
                location: if p.direction == PortDirection::Input {
                    FrontValueLocation::Input(p.port_id.clone())
                } else {
                    FrontValueLocation::Output(p.port_id.clone())
                },
                contract: CheckedValueContract::new(p.value_kind.clone(), max, vec![]).unwrap(),
            })
            .collect();
        let kind = Kind {
            kind_id: kind_id(identity),
            kind_contract_revision: KindIdentity::from(format!(
                "conduit.value/typed-pair@1/{identity}"
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
                max_queue_bytes: 16384,
            },
        };
        left = paired.clone();
        result.push((alias.into(), paired, kind));
    }
    Ok(result)
}
pub fn install_fixed_numeric_pair_catalogs(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    for (alias, ty, kind) in fixed_numeric_pair_contracts()? {
        startup.insert_structured_type(alias, ty)?;
        startup.insert(KindSignature {
            kind: kind.kind_id.as_str().into(),
            startup_parameters: vec![],
        })?;
        startup.insert_fore(
            kind.kind_id.as_str(),
            CheckedFront::new(vec![], kind.inputs.clone(), kind.outputs.clone(), None),
        )?;
        profile.insert_kind(kind).map_err(|e| format!("{e:?}"))?;
    }
    Ok(())
}
