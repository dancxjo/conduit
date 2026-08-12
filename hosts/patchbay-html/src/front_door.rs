//! Public HTML entrance backed by one canonical live local Body session.

use crate::RendererSnapshot;
use conduit_core::{BootId, HostId, SignId};
use patchbay_model::{
    LocalFrontDoor, RendererAdapterIdentity, RendererAdapterKind, RendererExecution,
};

pub fn front_door_snapshot() -> Result<RendererSnapshot, String> {
    let session = LocalFrontDoor::fresh()?;
    snapshot_for_front_door(&session)
}

pub(crate) fn snapshot_for_front_door(
    session: &LocalFrontDoor,
) -> Result<RendererSnapshot, String> {
    let projection = session.project()?;
    let execution = RendererExecution::prepare(
        projection.presentation,
        RendererAdapterKind::HtmlDomSvg,
        RendererAdapterIdentity {
            host_id: HostId::from("patchbay-html/front-door"),
            boot_id: BootId::from("patchbay-html/front-door/boot-1"),
            target_subject: "patchbay-html/front-door/document".into(),
        },
        SignId::from("patchbay-html/front-door/prepared"),
    )
    .map_err(|error| error.to_string())?;
    let mut snapshot =
        RendererSnapshot::from_execution(execution).map_err(|error| error.to_string())?;
    snapshot
        .attach_parts(projection.parts)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_presentation::PresentationRole;

    #[test]
    fn public_front_door_contains_only_live_canonical_topology() {
        let snapshot = front_door_snapshot().unwrap();
        let parts = snapshot.parts.as_ref().unwrap();
        assert_eq!(parts.parts.len(), 1);
        assert!(parts.wants_to_join.is_empty());
        assert_eq!(
            snapshot
                .presentation
                .subjects
                .iter()
                .find(|subject| subject.identity == snapshot.entrance.selected_subject)
                .map(|subject| subject.role),
            Some(PresentationRole::Part)
        );
        assert!(snapshot.presentation.basis.plan_id.is_none());
        assert!(snapshot.presentation.subjects.iter().all(|subject| {
            !subject.label.contains("Pico") && !subject.label.contains("tab 3")
        }));
    }
}
