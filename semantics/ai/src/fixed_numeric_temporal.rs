//! Explicit lockstep closing-Flow contracts for reusable stateless numerics.
use crate::fixed_numeric_catalog::fixed_numeric_contracts;
use alloc::{format, string::String, vec};
use conduit_core::*;
pub fn closing_numeric_contract(
    value_identity: &str,
    implementation: &str,
) -> Result<Kind, String> {
    let suffix = value_identity
        .strip_prefix("numeric/")
        .ok_or("numeric identity required")?;
    let mut kind = fixed_numeric_contracts()?
        .into_iter()
        .find(|kind| kind.kind_id.as_str() == value_identity)
        .ok_or("unsupported exact numeric shape")?;
    kind.kind_id = kind_id(&format!("numeric/flow-{suffix}"));
    kind.kind_contract_revision = KindIdentity::from(implementation);
    for port in kind.inputs.iter_mut().chain(&mut kind.outputs) {
        port.temporal = PortTemporal::Flow { closes: true };
    }
    Ok(kind)
}
pub fn closing_numeric_offer(
    value_identity: &str,
    implementation: &str,
) -> Result<CapabilityOffer, String> {
    let kind = closing_numeric_contract(value_identity, implementation)?;
    let identity = String::from(kind.kind_id.as_str());
    Ok(BackOfferBuilder::new(
        kind,
        Back {
            capability_id: CapabilityId::from(format!("{implementation}/{identity}")),
            execution_profile_id: ExecutionProfileId::from(implementation),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(implementation),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build())
}

pub fn install_closing_numeric_catalogs(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    install_closing_numeric_catalogs_mode(startup, profile, false)
}
fn install_closing_numeric_catalogs_mode(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
    capacity64: bool,
) -> Result<(), String> {
    for value in fixed_numeric_contracts()? {
        let identity = value.kind_id.as_str();
        let suffix = identity
            .strip_prefix("numeric/")
            .ok_or("numeric identity required")?;
        let implementation = if suffix == "history2x64" {
            crate::fixed_numeric_window_back::FLOW_WINDOW_IMPLEMENTATION
        } else if suffix == "one-pole40" {
            crate::fixed_numeric_scan_back::FLOW_SCAN_IMPLEMENTATION
        } else if suffix.starts_with("tanh") || suffix.starts_with("concatenate") {
            crate::fixed_numeric_operations_back::FLOW_OPERATION_IMPLEMENTATION
        } else if suffix.starts_with("gather") || suffix.starts_with("slice") {
            crate::fixed_numeric_index_back::FLOW_INDEX_IMPLEMENTATION
        } else if [
            "add",
            "multiply",
            "sigmoid",
            "complement",
            "exp",
            "scale",
            "clamp",
            "reciprocal-offset",
        ]
        .iter()
        .any(|name| {
            suffix
                .strip_prefix(name)
                .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
        }) {
            crate::fixed_numeric_signal_back::FLOW_ELEMENTWISE_IMPLEMENTATION
        } else {
            continue;
        };
        let kind = if capacity64 && capacity64_profile(implementation).is_ok() {
            closing_numeric_contract_capacity64(identity, implementation)?
        } else {
            closing_numeric_contract(identity, implementation)?
        };
        startup.insert(conduit_plot::KindSignature {
            kind: String::from(kind.kind_id.as_str()),
            startup_parameters: vec![],
        })?;
        startup.insert_fore(kind.kind_id.as_str(), kind.checked_front())?;
        profile
            .insert_kind(kind)
            .map_err(|error| format!("{error:?}"))?;
    }
    Ok(())
}

/// Distinct finite concurrency admission for stateless indexing/elementwise flows.
/// Default contracts remain at16 instances and all byte/queue bounds are unchanged.
pub fn capacity64_profile(implementation: &str) -> Result<String, String> {
    if ![
        crate::fixed_numeric_index_back::FLOW_INDEX_IMPLEMENTATION,
        crate::fixed_numeric_signal_back::FLOW_ELEMENTWISE_IMPLEMENTATION,
    ]
    .contains(&implementation)
    {
        return Err("capacity64 is supported only for stateless index/elementwise owners".into());
    }
    Ok(format!(
        "{}-capacity64@1",
        implementation
            .strip_suffix("@1")
            .ok_or("versioned numeric implementation required")?
    ))
}
pub fn closing_numeric_contract_capacity64(
    value_identity: &str,
    implementation: &str,
) -> Result<Kind, String> {
    let profile = capacity64_profile(implementation)?;
    let mut kind = closing_numeric_contract(value_identity, implementation)?;
    kind.kind_contract_revision = profile.into();
    kind.limits.max_active_instances = 64;
    Ok(kind)
}
pub fn closing_numeric_offer_capacity64(
    value_identity: &str,
    implementation: &str,
) -> Result<CapabilityOffer, String> {
    let profile = capacity64_profile(implementation)?;
    let kind = closing_numeric_contract_capacity64(value_identity, implementation)?;
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
/// Back preparation derives the expected offer from an explicit selected profile.
/// Full placement verification remains mandatory after this lookup.
pub fn closing_numeric_offer_for_placement(
    value_identity: &str,
    implementation: &str,
    placement: &PlannedGear,
) -> Result<CapabilityOffer, String> {
    if capacity64_profile(implementation)
        .is_ok_and(|p| p == placement.execution_profile_id.as_str())
    {
        closing_numeric_offer_capacity64(value_identity, implementation)
    } else {
        closing_numeric_offer(value_identity, implementation)
    }
}
pub fn install_closing_numeric_catalogs_capacity64(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    install_closing_numeric_catalogs_mode(startup, profile, true)
}
