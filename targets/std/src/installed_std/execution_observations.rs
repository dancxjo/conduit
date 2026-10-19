//! Projects retained installed execution outcomes into shared Core observations.
use conduit_core::{bind_sign, ActivePlayIdentity, FaceId, HostAdvertisement,
    Observation, ObservationKind, PlanFragment, TerminalDisposition};
use conduit_plan_lowering::lowering::{KernelExecutionIdentityMap, KernelIdentityMap};
use super::{math_host::MathHost, structured_face_host::StructuredFaceHost};

pub(super) struct ObservationContext<'a> {
    pub advertisement: &'a HostAdvertisement,
    pub fragment: &'a PlanFragment,
    pub active_play: &'a ActivePlayIdentity,
    pub lowered: &'a KernelIdentityMap,
    pub execution_identity: &'a mut KernelExecutionIdentityMap,
    pub next_sign_sequence: &'a mut u64,
}

pub(super) fn project(
    context: ObservationContext<'_>,
    structured_face_host: StructuredFaceHost,
    math_host: &MathHost,
    terminal_disposition: TerminalDisposition,
) -> Result<(Vec<Observation>, Vec<FaceId>), String> {
    let ObservationContext { advertisement, fragment, active_play, lowered,
        execution_identity, next_sign_sequence } = context;
    let (mut observations, face_ids) = structured_face_host.project(
        advertisement,
        fragment,
        active_play,
        lowered,
        execution_identity,
        next_sign_sequence,
    )?;
    if let Some(failure) = math_host.failure_observation(
        advertisement,
        fragment,
        active_play,
        execution_identity,
        next_sign_sequence,
    )? {
        observations.push(failure);
    }
    let terminal_sign = bind_sign(
        &advertisement.host_id,
        &advertisement.boot_id,
        Some(&active_play.active_play_id),
        *next_sign_sequence,
    );
    *next_sign_sequence = next_sign_sequence
        .checked_add(1)
        .ok_or_else(|| "std sign sequence exhausted".to_string())?;
    execution_identity
        .bind_sign(&terminal_sign, None, None, None)
        .map_err(|error| format!("bind std terminal sign: {error:?}"))?;
    observations.push(Observation {
        sign_id: terminal_sign.sign_id,
        active_play_id: Some(active_play.active_play_id.clone()),
        face_id: None,
        host_id: advertisement.host_id.clone(),
        boot_id: advertisement.boot_id.clone(),
        plan_id: Some(fragment.plan_id.clone()),
        placement_id: None,
        connection_id: None,
        kind: ObservationKind::PlanTerminal {
            disposition: terminal_disposition,
        },
    });
    Ok((observations, face_ids))
}
