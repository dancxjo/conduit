//! Protocol meaning only. No pin, initialized provider or execution grant.
use conduit_plot::{KindSignature, ProfileCatalog, StartupCatalog};
pub(super) fn install(
    startup: &mut StartupCatalog,
    profiles: &mut ProfileCatalog,
) -> Result<(), String> {
    conduit_pulse_capture::PulseReadContract::prepare()
        .map_err(debug)?
        .install(startup, profiles)?;
    conduit_environment_runtime::observation::ObservationContract::prepare()
        .map_err(debug)?
        .install(startup, profiles)?;
    conduit_environment_runtime::recording::LogContract::prepare(
        &conduit_environment_runtime::observation::ObservationContract::prepare().map_err(debug)?,
    )
    .map_err(debug)?
    .install(startup, profiles)?;
    conduit_semantic_catalog::install_resource_append_catalog(startup, profiles)?;
    for channel in conduit_environment_runtime::publication::PublicationChannel::ALL {
        conduit_environment_runtime::publication::PublicationContract::prepare(channel)
            .map_err(debug)?
            .install(startup, profiles)?;
    }
    install_kind(
        conduit_time::tick_trigger_semantic_contract(),
        startup,
        profiles,
    )?;
    let absence =
        conduit_core::StructuredInfoType::leaf(conduit_core::kind_id(conduit_core::EMPTY_INFO_ID))
            .and_then(|ty| ty.profile())
            .map_err(debug)?;
    startup.insert_value_kind_alias("Tm1637Absence", absence.value_kind().clone())?;
    let contract = conduit_tm1637_wire::WriteContract::prepare().map_err(debug)?;
    install_kind(contract.kind().clone(), startup, profiles)
}
fn install_kind(
    kind: conduit_core::Kind,
    startup: &mut StartupCatalog,
    profiles: &mut ProfileCatalog,
) -> Result<(), String> {
    startup.insert(KindSignature {
        kind: kind.kind_id.as_str().into(),
        startup_parameters: vec![],
    })?;
    startup.insert_fore(kind.kind_id.as_str(), kind.checked_front())?;
    profiles.insert_kind(kind).map_err(debug)
}
fn debug(error: impl core::fmt::Debug) -> String {
    format!("device authoring declaration: {error:?}")
}
