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
        let kind = closing_numeric_contract(identity, implementation)?;
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
