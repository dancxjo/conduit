//! Current Body's prepared adapter instance; direct CPU calls, not Host effects.
use super::super::super::*;
use conduit_face::{ApplicationEvent, ApplicationEventKind, ApplicationView};

#[test]
fn resident_adapter_retains_edit_request_in_current_observation_and_blocks_overwrite() {
    let source = include_str!("../../../../../plots/patchbay/native-retained.conduit");
    let mut request = body_start::tests::request_from_sources(&[source]);
    request.source = source.into();
    let (mut session, _) = body_start::prepare(request).unwrap();
    assert!(session.patchbay_observation().unwrap()["pending_owner_request"].is_null());
    let (revision, expected_subject, expected_expanded) = {
        let app = session
            .scheduler
            .applications
            .iter_mut()
            .flatten()
            .find(|app| {
                matches!(
                    app.as_ref(),
                    application_state::PreparedApplication::Patchbay(_)
                )
            })
            .unwrap();
        let initial = ApplicationView::decode(&app.execute(&[]).unwrap()).unwrap();
        let selected = ApplicationEvent {
            revision: initial.revision,
            action: "patchbay.plot.0".into(),
            kind: ApplicationEventKind::Activate,
            value: vec![],
        }
        .encode(&initial)
        .unwrap();
        let selected = ApplicationView::decode(&app.execute(&selected).unwrap()).unwrap();
        let edit = ApplicationEvent {
            revision: selected.revision,
            action: patchbay_application::EDIT_CURRENT_ACTION_ID.into(),
            kind: ApplicationEventKind::Activate,
            value: vec![],
        }
        .encode(&selected)
        .unwrap();
        let edited = ApplicationView::decode(&app.execute(&edit).unwrap()).unwrap();
        assert!(edited.actions.is_empty());
        assert!(edited
            .nodes
            .iter()
            .any(|node| node.state == conduit_face::ApplicationNodeState::Busy));
        let domain_view = match app.as_mut() {
            application_state::PreparedApplication::Patchbay(app) => {
                ApplicationView::decode(&app.port.apply(&[]).unwrap().view).unwrap()
            }
            _ => unreachable!(),
        };
        let overwrite = ApplicationEvent {
            revision: domain_view.revision,
            action: "patchbay.plot.0".into(),
            kind: ApplicationEventKind::Activate,
            value: vec![],
        }
        .encode(&domain_view)
        .unwrap();
        let busy = ApplicationView::decode(&app.execute(&overwrite).unwrap()).unwrap();
        assert_eq!(busy, edited);
        assert_eq!(
            ApplicationView::decode(&app.execute(&[0]).unwrap()).unwrap(),
            edited
        );
        let application_state::PreparedApplication::Patchbay(app) = app.as_ref() else {
            unreachable!()
        };
        (
            app.port.revision(),
            app.port.inspection().unwrap().subject_identity,
            app.port.graph().expanded_plot_id.clone(),
        )
    };
    let first = session.patchbay_observation().unwrap();
    assert_eq!(
        first["last_input_refusal"]["reason_code"],
        "patchbay-owner-request-pending"
    );
    let pending = &first["pending_owner_request"];
    assert_eq!(pending["application_revision"], revision);
    assert_eq!(pending["request"]["kind"], "EditCurrent");
    assert_eq!(pending["request"]["subject_identity"], expected_subject);
    assert_eq!(
        pending["request"]["expanded_plot_id"],
        expected_expanded.as_str()
    );
    assert_eq!(session.patchbay_observation().unwrap(), first);
    let context = session.patchbay_face.as_ref().unwrap();
    let disposition = patchbay_disposition::RequestDisposition {
        body_id: context.plan.body_id.clone(),
        body_plan_id: context.plan.plan_id.clone(),
        active_play_id: session.active_play_id.clone(),
        owner_checked_plot_id: context.resident.checked_plot_id.clone(),
        application_revision: revision,
        outcome: patchbay_disposition::RequestOutcome::Uncertain,
        evidence_id: "host/request-uncertain".into(),
    };
    let mut foreign = disposition.clone();
    foreign.active_play_id = "play/foreign".into();
    assert!(session.patchbay_request_disposition(foreign).is_err());
    let mut stale = disposition.clone();
    stale.application_revision -= 1;
    assert!(session.patchbay_request_disposition(stale).is_err());
    let mut oversized = disposition.clone();
    oversized.evidence_id = "x".repeat(257);
    assert!(session.patchbay_request_disposition(oversized).is_err());
    assert_eq!(session.patchbay_observation().unwrap(), first);
    let uncertain = session
        .patchbay_request_disposition(disposition.clone())
        .unwrap();
    assert_eq!(
        uncertain["pending_owner_request"],
        first["pending_owner_request"]
    );
    assert_eq!(uncertain["last_owner_disposition"]["outcome"], "Uncertain");
    let mut cancelled = disposition;
    cancelled.outcome = patchbay_disposition::RequestOutcome::Cancelled;
    cancelled.evidence_id = "host/request-cancelled".into();
    let resolved = session
        .patchbay_request_disposition(cancelled.clone())
        .unwrap();
    assert!(resolved["pending_owner_request"].is_null());
    assert_eq!(
        resolved["last_owner_disposition"]["request"],
        first["pending_owner_request"]["request"]
    );
    assert_eq!(resolved["last_owner_disposition"]["outcome"], "Cancelled");
    assert!(session.patchbay_request_disposition(cancelled).is_err());
    assert_eq!(session.patchbay_observation().unwrap(), resolved);
    let app = session
        .scheduler
        .applications
        .iter_mut()
        .flatten()
        .find(|app| {
            matches!(
                app.as_ref(),
                application_state::PreparedApplication::Patchbay(_)
            )
        })
        .unwrap();
    let current = ApplicationView::decode(&app.execute(&[]).unwrap()).unwrap();
    assert_eq!(current.revision, revision);
    assert!(!current.actions.is_empty());
    let next = ApplicationEvent {
        revision,
        action: "patchbay.plot.0".into(),
        kind: ApplicationEventKind::Activate,
        value: vec![],
    }
    .encode(&current)
    .unwrap();
    let next = ApplicationView::decode(&app.execute(&next).unwrap()).unwrap();
    assert_eq!(next.revision, revision + 1);
    session.cancel().unwrap();
}
