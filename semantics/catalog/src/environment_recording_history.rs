//! Reported quantity channels from one original environment recording generation.
//! Canonical recording JSON is domain input, never generic Batch decoding.
//! Host selection supplies producer context and independently authorized capture;
//! the semantic Source names no storage, transport, implementation or authority.
use alloc::{format, string::String, vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, CheckedValueContract, FrontValueContract,
    FrontValueLocation, Kind, KindSemanticLaw, KindTerminalBehavior, PortDescriptor, PortDirection,
    PortTemporal,
};
pub const ENVIRONMENT_RECORDING_HISTORY_KIND: &str = "data/environment-recording-history";
pub const ENVIRONMENT_RECORDING_HISTORY_REVISION: &str = "data/environment-recording-history@1";

pub fn environment_recording_history_kind() -> Result<Kind, String> {
    let checked = conduit_data::checked_quantity_observation_snapshot_type().map_err(debug)?;
    let profile = checked.value_type.profile().map_err(debug)?;
    let maximum =
        conduit_data::resolved_quantity_observation_snapshot_byte_bound().map_err(debug)?;
    let contract =
        CheckedValueContract::new(profile.value_kind().clone(), maximum, vec![]).map_err(debug)?;
    let mut tick = conduit_time::time_every_outputs().remove(0);
    tick.direction = PortDirection::Input;
    let observation = port_id("observations");
    let kind = Kind {
        kind_id: kind_id(ENVIRONMENT_RECORDING_HISTORY_KIND),
        kind_contract_revision: ENVIRONMENT_RECORDING_HISTORY_REVISION.into(),
        startup_parameters: vec![],
        shorthand: None,
        inputs: vec![tick],
        outputs: vec![PortDescriptor {
            port_id: observation.clone(),
            value_kind: profile.value_kind().clone(),
            direction: PortDirection::Output,
            temporal: PortTemporal::Flow { closes: false },
            abnormal_kind: None,
        }],
        configuration: vec![],
        semantic_laws: vec![
            KindSemanticLaw::ValueContracts(vec![FrontValueContract {
                location: FrontValueLocation::Output(observation),
                contract,
            }]),
            KindSemanticLaw::Terminal(KindTerminalBehavior::CompletesWhenInputsClose),
        ],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: maximum,
        },
    };
    kind.validate().map_err(debug)?;
    Ok(kind)
}
pub fn install_environment_recording_history_kind(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    let kind = environment_recording_history_kind()?;
    startup
        .ensure_structured_type(
            "ResolvedQuantityObservationSnapshot",
            conduit_data::resolved_quantity_observation_snapshot_type().map_err(debug)?,
        )
        .map_err(debug)?;
    startup.insert(conduit_plot::KindSignature {
        kind: ENVIRONMENT_RECORDING_HISTORY_KIND.into(),
        startup_parameters: vec![],
    })?;
    startup.insert_fore(ENVIRONMENT_RECORDING_HISTORY_KIND, kind.checked_front())?;
    profile.insert_kind(kind).map_err(debug)
}
fn debug(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}
