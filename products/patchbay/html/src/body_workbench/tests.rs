use super::*;
use conduit_body::{
    Body, BodyBiographyEvidence, BodyGraduationChoice, BodyGraduationEvidence, BodyMembership,
};
use conduit_core::SignId;

fn evidence(snapshot: &RendererSnapshot) -> Vec<u8> {
    let basis = &snapshot.presentation.basis;
    let born_body = Body::born(
        basis.source_document_id.clone().unwrap(),
        basis.checked_form_id.clone().unwrap(),
        1,
        SignId::from("patchbay/bornd"),
    )
    .unwrap();
    let membership = BodyMembership::new(born_body.body_id.clone()).unwrap();
    let mut evidence =
        BodyBiographyEvidence::born(born_body.clone(), membership, "Roseau".into()).unwrap();
    let body = born_body
        .admit_form(
            conduit_body::ResidentForm::new(
                conduit_core::SourceDocumentId::from("source/recorder"),
                conduit_core::CheckedFormId::from("checked/recorder"),
            ),
            SignId::from("patchbay/recorder-admitted"),
        )
        .unwrap();
    assert_eq!(basis.body_id.as_ref(), Some(&body.body_id));
    evidence
        .append_body_workload_events(
            body.clone(),
            &[(SignId::from("patchbay/recorder-admitted"), 2)],
        )
        .unwrap();
    let (body, wake) = body.wake(1, SignId::from("patchbay/woke")).unwrap();
    evidence.append_wake(body.clone(), wake, 3).unwrap();
    evidence
        .graduate(BodyGraduationEvidence {
            body_id: body.body_id,
            sequence: 4,
            sign_id: SignId::from("sign/roseau/graduated"),
            choice: BodyGraduationChoice::ExternalReader,
            patchbay_plan_id: None,
            patchbay_implementation_id: None,
        })
        .unwrap();
    serde_json::to_vec(&evidence).unwrap()
}

#[test]
fn attached_workbench_retains_exact_evidence_and_refuses_identity_drift() {
    let snapshot = crate::demonstration_snapshot().unwrap();
    let bytes = evidence(&snapshot);
    let attached = attach_body_workbench(
        snapshot.clone(),
        7,
        &bytes,
        BrowserBodyWorkbenchEntrance::ExternalReader,
    )
    .unwrap();
    let workbench = attached.body_workbench.as_ref().unwrap();
    assert_eq!(workbench.encoded_evidence, bytes);
    assert_eq!(workbench.current["friendly_name"], "Roseau");
    assert_eq!(workbench.current["workload_revision"], 1);
    assert_eq!(workbench.current["evidence_revision"], 7);
    assert_eq!(workbench.history["entries"].as_array().unwrap().len(), 4);
    let mut stale = workbench.clone();
    stale.body_id.push_str("-stale");
    assert!(validate_body_workbench(&stale, &snapshot.presentation).is_err());

    let entrance =
        body_workbench_snapshot(1, &bytes, BrowserBodyWorkbenchEntrance::ExternalReader).unwrap();
    let forms = entrance
        .presentation
        .subjects
        .iter()
        .filter(|subject| subject.role == PresentationRole::Form)
        .collect::<Vec<_>>();
    assert_eq!(forms.len(), 2);
    let initial_checked = snapshot
        .presentation
        .basis
        .checked_form_id
        .as_ref()
        .unwrap()
        .as_str();
    assert!(forms.iter().any(|subject| subject.label == initial_checked));
    assert!(forms
        .iter()
        .any(|subject| subject.label == "checked/recorder"));
    assert_eq!(
        entrance.presentation.basis.body_id,
        snapshot.presentation.basis.body_id
    );
    let navigation = entrance.navigation.unwrap();
    assert!(navigation
        .navigation
        .places
        .iter()
        .any(|place| place.place == conduit_presentation::PresentationPlace::Program));
    assert!(navigation
        .navigation
        .places
        .iter()
        .any(|place| place.place == conduit_presentation::PresentationPlace::Body));
}
