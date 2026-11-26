//! Ordinary typed projection of reported finite quantity observations into Face.
use alloc::{format, string::String, vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, CheckedValueContract, FrontValueContract,
    FrontValueLocation, Kind, KindSemanticLaw, KindTerminalBehavior, PortDescriptor, PortDirection,
    PortTemporal,
};

pub const QUANTITY_OBSERVATION_FACE_KIND: &str = "face/quantity-observation-series";
pub const QUANTITY_OBSERVATION_FACE_REVISION: &str = "face/quantity-observation-series@1";

pub fn quantity_observation_face_kind() -> Result<Kind, String> {
    let checked = conduit_data::checked_quantity_observation_snapshot_type().map_err(debug)?;
    let profile = checked.value_type.profile().map_err(debug)?;
    let maximum =
        conduit_data::resolved_quantity_observation_snapshot_byte_bound().map_err(debug)?;
    let contract =
        CheckedValueContract::new(profile.value_kind().clone(), maximum, vec![]).map_err(debug)?;
    let input = port_id("observations");
    let configuration = crate::data_label_configuration();
    let kind = Kind {
        kind_id: kind_id(QUANTITY_OBSERVATION_FACE_KIND),
        kind_contract_revision: QUANTITY_OBSERVATION_FACE_REVISION.into(),
        startup_parameters: crate::startup_front(&configuration),
        shorthand: None,
        inputs: vec![PortDescriptor {
            port_id: input.clone(),
            value_kind: profile.value_kind().clone(),
            direction: PortDirection::Input,
            temporal: PortTemporal::Flow { closes: false },
            abnormal_kind: None,
        }],
        outputs: vec![],
        configuration,
        semantic_laws: vec![
            KindSemanticLaw::ValueContracts(vec![FrontValueContract {
                location: FrontValueLocation::Input(input),
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
pub fn install_quantity_observation_face_kind(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    let kind = quantity_observation_face_kind()?;
    startup
        .ensure_structured_type(
            "ResolvedQuantityObservationSnapshot",
            conduit_data::resolved_quantity_observation_snapshot_type().map_err(debug)?,
        )
        .map_err(debug)?;
    startup.insert(conduit_plot::KindSignature {
        kind: QUANTITY_OBSERVATION_FACE_KIND.into(),
        startup_parameters: crate::data_label_startup_signature(),
    })?;
    startup.insert_fore(QUANTITY_OBSERVATION_FACE_KIND, kind.checked_front())?;
    profile.insert_kind(kind).map_err(debug)
}
pub fn quantity_observation_face_workspace_bytes() -> Result<u32, String> {
    let prepared =
        conduit_face::PreparedQuantityObservationSnapshotFace::prepare().map_err(debug)?;
    prepared
        .retained_backing_bytes()
        .and_then(|n| n.checked_add(prepared.projection_workspace_bytes()?))
        .and_then(|n| u32::try_from(n).ok())
        .ok_or_else(|| "quantity observation Face workspace overflow".into())
}
fn debug(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}
