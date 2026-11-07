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
    for channel in conduit_environment_runtime::publication::PublicationChannel::ALL {
        conduit_environment_runtime::publication::PublicationContract::prepare(channel)
            .map_err(debug)?
            .install(startup, profiles)?;
    }
    let contract = conduit_tm1637_wire::WriteContract::prepare().map_err(debug)?;
    let kind = contract.kind();
    startup.insert(KindSignature {
        kind: kind.kind_id.as_str().into(),
        startup_parameters: vec![],
    })?;
    startup.insert_fore(kind.kind_id.as_str(), kind.checked_front())?;
    profiles.insert_kind(kind.clone()).map_err(debug)
}
fn debug(error: impl core::fmt::Debug) -> String {
    format!("device authoring declaration: {error:?}")
}
