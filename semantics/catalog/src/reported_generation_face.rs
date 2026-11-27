//! Ordinary typed projection of reported finite quantity observations into Face.
use alloc::{format, string::String, vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, CheckedValueContract, FrontValueContract,
    FrontValueLocation, Kind, KindSemanticLaw, KindTerminalBehavior, PortDescriptor, PortDirection,
    PortTemporal,
};

pub const REPORTED_GENERATION_FACE_KIND: &str = "face/reported-generation-series";
pub const REPORTED_GENERATION_FACE_REVISION: &str = "face/reported-generation-series@1";

pub fn reported_generation_maximum_bytes() -> Result<u32, String> {
    conduit_dht11_observation::reported_generation::maximum_canonical_bytes().map_err(debug)
}

pub fn reported_generation_face_kind() -> Result<Kind, String> {
    let value_type = conduit_dht11_observation::reported_generation::value_type().map_err(debug)?;
    let profile = value_type.profile().map_err(debug)?;
    let maximum =
        conduit_dht11_observation::reported_generation::maximum_canonical_bytes().map_err(debug)?;
    let contract =
        CheckedValueContract::new(profile.value_kind().clone(), maximum, vec![]).map_err(debug)?;
    let input = port_id("observations");
    let configuration = crate::data_label_configuration();
    let kind = Kind {
        kind_id: kind_id(REPORTED_GENERATION_FACE_KIND),
        kind_contract_revision: REPORTED_GENERATION_FACE_REVISION.into(),
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
pub fn install_reported_generation_face_kind(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    let kind = reported_generation_face_kind()?;
    startup
        .ensure_structured_type(
            "ReportedRecordingGeneration",
            conduit_dht11_observation::reported_generation::value_type().map_err(debug)?,
        )
        .map_err(debug)?;
    startup.insert(conduit_plot::KindSignature {
        kind: REPORTED_GENERATION_FACE_KIND.into(),
        startup_parameters: crate::data_label_startup_signature(),
    })?;
    startup.insert_fore(REPORTED_GENERATION_FACE_KIND, kind.checked_front())?;
    profile.insert_kind(kind).map_err(debug)
}
/// Retained schema/decoder and allocating typed64 projection. The common route
/// separately charges inline inventory, input custody and two escaped slots.
pub fn reported_generation_face_workspace_bytes() -> Result<u32, String> {
    use conduit_dht11_observation::{
        reported_generation::PreparedReportedRecordingGenerationInput as Input, ObservationContract,
    };
    use core::mem::size_of;
    let contract = ObservationContract::prepare().map_err(debug)?;
    let input = Input::prepare().map_err(debug)?;
    let n = conduit_face::MAX_REPORTED_QUANTITY_OBSERVATIONS;
    let extra = 4 * (size_of::<conduit_face::FaceProperty>() + 128 + 32);
    // A concrete owned row holds optional two quantities, times, gap and a
    // static reason. 128 additional bytes conservatively cover row metadata
    // without introducing a dependency on the installed implementation.
    let rows = n.checked_mul(
        size_of::<Option<[conduit_core::Quantity; 2]>>()
            + 128
            + 2 * size_of::<conduit_face::ReportedQuantityObservation<'_>>(),
    );
    input
        .retained_backing_bytes()
        .and_then(|bytes| bytes.checked_add(contract.retained_backing_bytes()?))
        .and_then(|bytes| bytes.checked_add(Input::decoding_backing_bound(&contract)?))
        .and_then(|bytes| bytes.checked_add(rows?))
        .and_then(|bytes| {
            bytes.checked_add(conduit_face::reported_quantity_projection_workspace_bytes(
                n, 2, true,
            )?)
        })
        .and_then(|bytes| bytes.checked_add(3 * extra))
        .and_then(|bytes| u32::try_from(bytes).ok())
        .ok_or_else(|| "reported generation Face workspace overflow".into())
}
fn debug(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}
