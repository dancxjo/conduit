//! Explicit current control reading through the actual selected peer equipment.
use super::{wire::OwnerRoute, PresentationHost};
use conduit_face::{Face, FaceRole};
use conduit_std_host::{direct_spoken_template_mask::DirectSpokenTemplateLook, RunControl};

#[derive(serde::Serialize)]
pub(super) struct SpokenReadingEvidence {
    execution: conduit_std_host::spoken_template_execution::SpokenTemplateExecutionSummary,
    route: conduit_face::RemoteOwnerMaskRouteSeal,
    face_bytes: usize,
    route_binding_bytes: usize,
    route_seal_bytes: usize,
    control_document_bytes: usize,
}

pub(super) fn read(
    peer: &mut PresentationHost,
    owner: &mut OwnerRoute,
    face: &Face,
) -> Result<SpokenReadingEvidence, String> {
    let host = peer.runtime.presentation_host_mut()?;
    let prepared =
        host.prepare_direct_spoken_template_mask(face, DirectSpokenTemplateLook::Control)?;
    let snapshot = owner.select_spoken(&prepared.remote_selection()?)?;
    if snapshot.face != *face || snapshot.interactions_admitted {
        return Err("spoken selection differs from the current read-only Face".into());
    }
    let prepared = prepared.with_remote_route(host, face, &snapshot.route)?;
    let front = face
        .subjects
        .iter()
        .find(|subject| subject.role == FaceRole::Body)
        .ok_or("spoken control Face has no Body subject")?
        .identity
        .clone();
    let route_id = snapshot.route.route_plan_id.as_str();
    let (fragment, preparation, reading) = prepared.into_preparation(
        host,
        face,
        front,
        "artifact/presentation/control".into(),
        format!("sign/{route_id}/prepared").into(),
        format!("sign/{route_id}/available").into(),
    )?;
    let control = RunControl::default();
    let mut session = host.execute_prepared_spoken_template_opening(
        face,
        fragment,
        preparation,
        reading,
        &control,
    )?;
    let show = session.show().clone();
    owner.acknowledge(&show)?;
    while !session.complete() {
        session.read_next(host, face, &show, &control)?;
    }
    Ok(SpokenReadingEvidence {
        execution: session.finish()?,
        face_bytes: serde_json::to_vec(face)
            .map_err(|error| error.to_string())?
            .len(),
        route_binding_bytes: snapshot
            .route
            .encoded_basis_bytes()
            .map_err(|error| format!("spoken route encoding:{error:?}"))?,
        route_seal_bytes: serde_json::to_vec(&snapshot.route)
            .map_err(|error| error.to_string())?
            .len(),
        control_document_bytes: snapshot.document_bytes,
        route: snapshot.route,
    })
}
