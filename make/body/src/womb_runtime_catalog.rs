//! Exact checked Womb Fore. This catalogue grants no installed builder offer.
use crate::womb_runtime::*;

pub const WOMB_HOST_CALL: &str = "conduit.host/womb-prepare@1";

pub fn install_womb_catalogs(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    startup.insert(conduit_plot::KindSignature {
        kind: WOMB_KIND.into(),
        startup_parameters: vec![],
    })?;
    profile.insert_kind(womb_kind()).map_err(|e| e.to_string())
}

pub fn womb_kind() -> conduit_core::Kind {
    crate::preparation_catalog::digest_preparation_kind(
        WOMB_KIND,
        "conduit.womb/prepare@1",
        WOMB_REQUEST_KIND,
        WOMB_OUTCOME_KIND,
    )
}

pub fn womb_host_call_requirement() -> conduit_core::HostCallRequirement {
    conduit_core::HostCallRequirement {
        contract_id: conduit_core::HostCallContractId::from(WOMB_HOST_CALL),
        target_kind: Some(conduit_core::kind_id(WOMB_KIND)),
        maximum_in_flight: 1,
        maximum_input_bytes: WOMB_REQUEST_BYTES as u32,
        maximum_output_bytes: WOMB_OUTCOME_BYTES as u32,
    }
}

pub fn womb_kernel_call_binding(
    call: conduit_kernel::HostCallId,
) -> conduit_kernel::HostCallBinding {
    conduit_kernel::HostCallBinding {
        call,
        maximum_input_bytes: WOMB_REQUEST_BYTES as u32,
        maximum_output_bytes: WOMB_OUTCOME_BYTES as u32,
    }
}
