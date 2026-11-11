//! Finite ingress into an explicitly coalesced Current.
use alloc::vec;
use conduit_core::*;

pub const FLOW_COALESCE_FINITE_KIND: &str = "flow/coalesce-latest-finite";
pub const FLOW_COALESCE_FINITE_REVISION: &str = "conduit.flow/coalesce-latest-finite@1";

/// Closure drains the last replacement and completes the producer. It does not
/// manufacture a replacement or a terminal payload for the Current consumer.
pub fn flow_coalesce_finite_contract(value: &CheckedValueContract) -> Kind {
    let mut kind: Kind =
        super::flow_coalesce_latest_contract(&value.value_kind, value.maximum_bytes).into();
    kind.kind_id = kind_id(FLOW_COALESCE_FINITE_KIND);
    kind.kind_contract_revision = FLOW_COALESCE_FINITE_REVISION.into();
    kind.inputs[0].temporal = PortTemporal::Flow { closes: true };
    kind.semantic_laws = vec![KindSemanticLaw::ValueContracts(vec![
        FrontValueContract {
            location: FrontValueLocation::Input(port_id("in")),
            contract: value.clone(),
        },
        FrontValueContract {
            location: FrontValueLocation::Output(port_id("out")),
            contract: value.clone(),
        },
    ])];
    kind
}

#[cfg(feature = "plot-catalog")]
pub fn install_flow_coalesce_finite_kind(
    value: &CheckedValueContract,
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    let kind = flow_coalesce_finite_contract(value);
    startup.insert(conduit_plot::KindSignature {
        kind: FLOW_COALESCE_FINITE_KIND.into(),
        startup_parameters: vec![],
    })?;
    startup.insert_fore(FLOW_COALESCE_FINITE_KIND, kind.checked_front())?;
    profile
        .insert_kind(kind)
        .map_err(|error| alloc::format!("{error:?}"))
}
