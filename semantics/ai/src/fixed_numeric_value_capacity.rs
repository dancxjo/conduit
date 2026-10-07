//! Explicit larger instance budget for stateless Value numerical owners.
//! Mathematical contracts and per-instance storage remain exact and unchanged.
use crate::fixed_numeric_catalog::fixed_numeric_contracts;
pub const INDEX_IMPLEMENTATION: &str = "conduit.numeric/scalar-explicit-index@1";
pub const ELEMENTWISE_IMPLEMENTATION: &str = "conduit.numeric/scalar-finite-elementwise@1";
use alloc::{format, string::String, vec};
use conduit_core::{Back, BackOfferBuilder, CapabilityOffer, Kind, PlannedGear};

pub fn value_capacity64_profile(implementation: &str) -> Result<String, String> {
    if ![
        INDEX_IMPLEMENTATION,
        ELEMENTWISE_IMPLEMENTATION,
        "conduit.numeric/scalar-tanh@1",
    ]
    .contains(&implementation)
    {
        return Err(
            "Value64 supports stateless index/elementwise/tanh implementations only".into(),
        );
    }
    Ok(format!(
        "{}-capacity64@1",
        implementation
            .strip_suffix("@1")
            .ok_or("versioned implementation required")?
    ))
}
pub fn value_capacity64_implementation(kind: &str) -> Option<&'static str> {
    if kind.starts_with("numeric/tanh") {
        return Some("conduit.numeric/scalar-tanh@1");
    }
    if kind.starts_with("numeric/gather") || kind.starts_with("numeric/slice") {
        return Some(INDEX_IMPLEMENTATION);
    }
    for operation in [
        "add",
        "multiply",
        "complement",
        "sigmoid",
        "exp",
        "reciprocal-offset",
        "scale",
        "clamp",
    ] {
        if kind.starts_with(&format!("numeric/{operation}")) {
            return Some(ELEMENTWISE_IMPLEMENTATION);
        }
    }
    None
}
pub fn value_capacity64_contract(mut kind: Kind) -> Result<Kind, String> {
    let implementation =
        value_capacity64_implementation(kind.kind_id.as_str()).ok_or("unsupported Value64 Kind")?;
    // This input is the exact owned contract, not an arbitrary named Kind.
    let original = fixed_numeric_contracts()?
        .into_iter()
        .find(|k| k.kind_id == kind.kind_id)
        .ok_or("unknown owned Value Kind")?;
    if kind != original {
        return Err("Value64 requires the exact owned base contract".into());
    }
    kind.kind_contract_revision = value_capacity64_profile(implementation)?.into();
    kind.limits.max_active_instances = 64;
    Ok(kind)
}
pub fn value_offer_capacity64(base: CapabilityOffer) -> Result<CapabilityOffer, String> {
    let implementation = base.implementation.implementation_id.as_str();
    let profile = value_capacity64_profile(implementation)?;
    if value_capacity64_implementation(base.kind_id.as_str()) != Some(implementation) {
        return Err("wrong Value64 implementation".into());
    }
    let kind = fixed_numeric_contracts()?
        .into_iter()
        .find(|k| k.kind_id == base.kind_id)
        .ok_or("unknown owned Value Kind")?;
    let kind = value_capacity64_contract(kind)?;
    let identity = String::from(kind.kind_id.as_str());
    Ok(BackOfferBuilder::new(
        kind,
        Back {
            capability_id: format!("{profile}/{identity}").into(),
            execution_profile_id: profile.into(),
            implementation_id: implementation.into(),
            artifact_id: implementation.into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build())
}
pub fn value_offer_for_placement(
    base: CapabilityOffer,
    placement: &PlannedGear,
) -> Result<CapabilityOffer, String> {
    if value_capacity64_profile(base.implementation.implementation_id.as_str())
        .is_ok_and(|p| p == placement.execution_profile_id.as_str())
    {
        value_offer_capacity64(base)
    } else {
        Ok(base)
    }
}
