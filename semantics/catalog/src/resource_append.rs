//! Generic finite append equipment; no observation, catalog or delivery semantics.
#[cfg(feature = "plot-catalog")]
use alloc::vec::Vec;
use conduit_core::*;
pub use conduit_resource_snapshot::append_contract::{
    APPEND_KIND as RESOURCE_APPEND_KIND, APPEND_REVISION as RESOURCE_APPEND_REVISION,
};
/// Opaque existing Bytes, bounded to 200, and a canonical declared typed result.
/// A recorded output denotes verified media commitment only; capacity and
/// uncertainty remain ordinary values. Input closure drains
/// completed indices then closes; cancellation cannot retract committed media.
pub fn resource_append_semantic_contract() -> Kind {
    conduit_resource_snapshot::append_contract::definition()
}

#[cfg(feature = "plot-catalog")]
pub fn install_resource_append_catalog(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    let kind = resource_append_semantic_contract();
    let result = conduit_resource_snapshot::append_result::result_type()
        .map_err(|e| alloc::format!("{e:?}"))?;
    startup.insert_structured_type("resource/try-append-record/result", result.clone())?;
    startup.insert_structured_type("RecordAppendResult", result)?;
    startup.insert(conduit_plot::KindSignature {
        kind: RESOURCE_APPEND_KIND.into(),
        startup_parameters: Vec::new(),
    })?;
    startup.insert_fore(RESOURCE_APPEND_KIND, kind.checked_front())?;
    profile
        .insert_kind(kind)
        .map_err(|e| alloc::format!("{e:?}"))
}

/// Install the ordinary append contract together with the independently typed
/// read operation and their single-residence service. This is Source vocabulary,
/// not an initialized provider, resource grant or permission to read media.
#[cfg(feature = "plot-catalog")]
pub fn install_resource_record_service_catalog(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    install_resource_append_catalog(startup, profile)?;
    let result = conduit_resource_snapshot::read_result::result_type()
        .map_err(|e| alloc::format!("{e:?}"))?;
    startup.insert_structured_type("resource/read-record/result", result.clone())?;
    startup.insert_structured_type("RecordReadResult", result)?;
    for kind in [
        conduit_resource_snapshot::read_contract::definition(),
        conduit_resource_snapshot::service_contract::definition(),
    ] {
        startup.insert(conduit_plot::KindSignature {
            kind: kind.kind_id.as_str().into(),
            startup_parameters: Vec::new(),
        })?;
        startup.insert_fore(kind.kind_id.as_str(), kind.checked_front())?;
        profile
            .insert_kind(kind)
            .map_err(|e| alloc::format!("{e:?}"))?;
    }
    Ok(())
}
