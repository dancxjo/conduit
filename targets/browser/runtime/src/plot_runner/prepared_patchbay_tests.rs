//! Current Body's prepared adapter instance; direct CPU calls, not Host effects.
use super::super::super::*;
use conduit_face::{ApplicationEvent, ApplicationEventKind, ApplicationView};

#[test]
fn resident_adapter_retains_edit_request_in_current_observation_and_blocks_overwrite() {
    let source = include_str!("../../../../../plots/patchbay/main.conduit");
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
        let overwrite = ApplicationEvent {
            revision: edited.revision,
            action: "patchbay.plot.0".into(),
            kind: ApplicationEventKind::Activate,
            value: vec![],
        }
        .encode(&edited)
        .unwrap();
        assert!(app
            .execute(&overwrite)
            .unwrap_err()
            .contains("awaits disposition"));
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
    let pending = &first["pending_owner_request"];
    assert_eq!(pending["application_revision"], revision);
    assert_eq!(pending["request"]["kind"], "EditCurrent");
    assert_eq!(pending["request"]["subject_identity"], expected_subject);
    assert_eq!(
        pending["request"]["expanded_plot_id"],
        expected_expanded.as_str()
    );
    assert_eq!(session.patchbay_observation().unwrap(), first);
    session.cancel().unwrap();
}
