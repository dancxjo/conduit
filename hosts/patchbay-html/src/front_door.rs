//! Public HTML entrance backed by one canonical live local Body session.

use crate::RendererSnapshot;
use conduit_core::{BootId, HostId, SignId};
use patchbay_model::{
    LocalFrontDoor, RendererAdapterIdentity, RendererAdapterKind, RendererExecution,
    ZeroBodyFrontDoor,
};

pub fn front_door_snapshot() -> Result<RendererSnapshot, String> {
    let session = ZeroBodyFrontDoor::fresh()?;
    snapshot_for_zero_body_front_door(&session)
}

pub(crate) fn snapshot_for_zero_body_front_door(
    session: &ZeroBodyFrontDoor,
) -> Result<RendererSnapshot, String> {
    let projection = session.project()?;
    let navigation = projection.navigation;
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
        .attach_navigation(navigation)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}

pub(crate) fn snapshot_for_front_door(
    session: &LocalFrontDoor,
) -> Result<RendererSnapshot, String> {
    let projection = session.project()?;
    let navigation = projection.navigation;
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
    snapshot
        .attach_navigation(navigation)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_presentation::PresentationRole;

    #[test]
    fn public_front_door_truthfully_begins_without_a_body() {
        let snapshot = front_door_snapshot().unwrap();
        assert!(snapshot.parts.is_none());
        assert!(snapshot.presentation.basis.body_id.is_none());
        assert!(snapshot.entrance.body_id.is_none());
        assert_eq!(
            snapshot
                .presentation
                .subjects
                .iter()
                .find(|subject| {
                    Some(subject.identity.as_str()) == snapshot.entrance.selected_subject.as_deref()
                })
                .map(|subject| subject.role),
            Some(PresentationRole::Host)
        );
        assert!(snapshot.presentation.basis.plan_id.is_none());
        assert!(snapshot
            .presentation
            .subjects
            .iter()
            .any(|subject| subject.role == PresentationRole::Seed));
        assert!(!snapshot
            .presentation
            .subjects
            .iter()
            .any(|subject| subject.role == PresentationRole::Body));
        assert_eq!(snapshot.presentation.actions.len(), 2);
        assert_eq!(snapshot.presentation.disclosures.len(), 2);
        let seed = snapshot
            .presentation
            .subjects
            .iter()
            .find(|subject| subject.role == PresentationRole::Seed)
            .unwrap();
        let actions = snapshot
            .presentation
            .actions
            .iter()
            .filter(|action| action.target == seed.identity)
            .collect::<Vec<_>>();
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0].label, "Open");
        assert_eq!(actions[1].label, "Birth");
    }
}
