//! Exact checked member Seed preparation Fores. Checking grants no builder offer.
use crate::womb_runtime::*;

pub const STEM_KIND: &str = "stem/prepare";
pub const STEM_REQUEST_KIND: &str = "conduit.stem/request@1";
pub const STEM_OUTCOME_KIND: &str = "conduit.stem/outcome@1";
pub const STEM_HOST_CALL: &str = "conduit.host/stem-prepare@1";

pub const WOMB_HOST_CALL: &str = "conduit.host/womb-prepare@1";

pub fn install_womb_catalogs(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    install_preparation_kind(startup, profile, womb_kind())
}

pub fn install_stem_catalogs(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    install_preparation_kind(startup, profile, stem_kind())
}

fn install_preparation_kind(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
    kind: conduit_core::Kind,
) -> Result<(), String> {
    startup.insert(conduit_plot::KindSignature {
        kind: kind.kind_id.as_str().into(),
        startup_parameters: vec![],
    })?;
    profile.insert_kind(kind).map_err(|error| error.to_string())
}

pub fn stem_kind() -> conduit_core::Kind {
    crate::preparation_catalog::digest_preparation_kind(
        STEM_KIND,
        "conduit.stem/prepare@1",
        STEM_REQUEST_KIND,
        STEM_OUTCOME_KIND,
    )
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
    preparation_host_call(WOMB_KIND, WOMB_HOST_CALL)
}

pub fn stem_host_call_requirement() -> conduit_core::HostCallRequirement {
    preparation_host_call(STEM_KIND, STEM_HOST_CALL)
}

fn preparation_host_call(kind: &str, contract: &str) -> conduit_core::HostCallRequirement {
    conduit_core::HostCallRequirement {
        contract_id: conduit_core::HostCallContractId::from(contract),
        target_kind: Some(conduit_core::kind_id(kind)),
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
