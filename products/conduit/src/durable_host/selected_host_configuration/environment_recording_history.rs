//! Explicit original recording JSON selection; all read custody stays in Root.
use super::snapshot_read::ReadBinding;
use conduit_core::AuthorityGrant;
use conduit_host_make::{CheckedHostConfiguration, HostImage};
use conduit_linux_snapshot_store::SnapshotDirectory;
use conduit_std_host::{
    environment_recording_history::CheckedEnvironmentRecordingHistory, StdHost, StdHostConfig,
};

pub(super) fn selected(checked: &CheckedHostConfiguration) -> bool {
    checked.configuration().bases.iter().any(|base| {
        base.kind == conduit_std_offers::ENVIRONMENT_RECORDING_HISTORY_BASE
            && base.initialization.is_some()
    })
}
pub(super) fn validate(checked: &CheckedHostConfiguration) -> Result<(), String> {
    let parameters = CheckedEnvironmentRecordingHistory::from_configuration(checked)?;
    if !checked.configuration().bases.iter().any(|base| {
        base.kind == conduit_std_offers::REPORTED_GENERATION_FACE_BASE
            && base.initialization.is_some()
    }) {
        return Err(
            "installed environment recording requires its selected quantity Face Base".into(),
        );
    }
    if parameters.directory.is_none() || parameters.reference.is_none() {
        return Err("installed environment recording requires an explicit directory and original JSON snapshot reference".into());
    }
    Ok(())
}
pub(super) fn initialize(
    config: StdHostConfig,
    image: &HostImage,
    checked: &CheckedHostConfiguration,
) -> Result<StdHost, String> {
    validate(checked)?;
    let parameters = CheckedEnvironmentRecordingHistory::from_configuration(checked)?;
    // Configuration expresses local operator intent. Opening this exact existing
    // directory acquires descriptor custody; producer IDs do not authorize IO.
    let directory =
        SnapshotDirectory::open_selected(std::path::Path::new(parameters.directory.unwrap()))
            .map_err(|why| format!("open selected environment-recording directory: {why}"))?;
    let read = ReadBinding::authorize(&config, &directory, parameters.reference.unwrap())?;
    let offer = conduit_std_offers::environment_recording_history_offer()?;
    let [requirement] = offer.authority_requirements.as_slice() else {
        return Err("environment recording requires one compiled Source-use grant".into());
    };
    let source_use = AuthorityGrant {
        grant_id: format!(
            "{}/environment-recording-history-use",
            config.boot_id.as_str()
        )
        .into(),
        contract_id: requirement.contract_id.clone(),
        host_call_contract_id: requirement.host_call_contract_id.clone(),
        subject_kind: offer.kind_id,
        host_id: config.host_id.clone(),
        boot_id: config.boot_id.clone(),
        capability_id: offer.capability_id,
    };
    // SAFETY: the selected local operator configuration authorizes this readonly
    // capture; Root retains actual directory custody/current independent read
    // grants and the separate Source-use grant. Constructor funds all capture
    // work before IO and retains only frozen bytes. No producer trust, durable
    // residence, publication ACK or restart continuity is inferred.
    unsafe {
        StdHost::from_selected_environment_recording_history_image(
            config,
            image,
            &conduit_workspace_make::catalog(),
            checked,
            source_use,
            read.capture(),
        )
    }
}
