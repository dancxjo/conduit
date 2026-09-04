use conduit_body::{
    Body, BodyBiographyEvidence, BodyGraduationChoice, BodyGraduationEvidence, BodyMembership,
    BodyWorkset, ResidentForm,
};
use conduit_core::{CheckedFormId, SignId, SourceDocumentId};
use conduit_presentation::PresentationRole;
use patchbay_html::{body_workbench_snapshot, BrowserBodyWorkbenchEntrance};

fn resident(name: &str) -> ResidentForm {
    ResidentForm::new(
        SourceDocumentId::from("source/reviewed-inventory"),
        CheckedFormId::from(format!("checked/{name}")),
    )
}

#[test]
fn patchbay_handoff_projects_every_initial_form_as_ordinary_active_work() {
    let initial = [
        resident("clock"),
        resident("lantern"),
        resident("telegraph"),
    ];
    let body = Body::born_with_forms(
        BodyWorkset::from_forms(initial.clone()).unwrap(),
        1,
        SignId::from("sign/born"),
    )
    .unwrap();
    let membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let mut biography =
        BodyBiographyEvidence::born(body.clone(), membership, "Talvi Applebough".into()).unwrap();
    biography
        .graduate(BodyGraduationEvidence {
            body_id: body.body_id.clone(),
            sequence: 2,
            sign_id: SignId::from("sign/graduated"),
            choice: BodyGraduationChoice::ExternalReader,
            patchbay_plan_id: None,
            patchbay_implementation_id: None,
        })
        .unwrap();

    let encoded = serde_json::to_vec(&biography).unwrap();
    let snapshot =
        body_workbench_snapshot(1, &encoded, BrowserBodyWorkbenchEntrance::ExternalReader).unwrap();
    let workbench = snapshot.body_workbench.unwrap();
    assert_eq!(
        workbench.current["active_forms"].as_array().unwrap().len(),
        3
    );
    let visible_forms = snapshot
        .presentation
        .subjects
        .iter()
        .filter(|subject| subject.role == PresentationRole::Form)
        .map(|subject| subject.label.as_str())
        .collect::<Vec<_>>();
    for form in initial {
        assert!(visible_forms.contains(&form.checked_form_id.as_str()));
    }
    assert_eq!(snapshot.presentation.basis.body_id, Some(body.body_id));
}
