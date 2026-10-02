use conduit_body::{
    Body, BodyBiographyEvidence, BodyGraduationChoice, BodyGraduationEvidence, BodyMembership,
    BodyWorkset, ResidentPlot,
};
use conduit_browser_patchbay_workbench::{body_workbench_snapshot, BrowserBodyWorkbenchEntrance};
use conduit_core::{CheckedPlotId, SignId, SourceDocumentId};
use conduit_presentation::PresentationRole;

fn resident(name: &str) -> ResidentPlot {
    ResidentPlot::new(
        SourceDocumentId::from("source/reviewed-inventory"),
        CheckedPlotId::from(format!("checked/{name}")),
    )
}

#[test]
fn patchbay_handoff_projects_every_initial_plot_as_ordinary_active_work() {
    let initial = [
        resident("clock"),
        resident("lantern"),
        resident("telegraph"),
    ];
    let body = Body::born_with_plots(
        BodyWorkset::from_plots(initial.clone()).unwrap(),
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
            reader_plan_id: None,
            reader_implementation_id: None,
        })
        .unwrap();

    let encoded = serde_json::to_vec(&biography).unwrap();
    let snapshot =
        body_workbench_snapshot(1, &encoded, BrowserBodyWorkbenchEntrance::ExternalReader).unwrap();
    let workbench = snapshot.body_workbench.unwrap();
    assert_eq!(
        workbench.current["active_plots"].as_array().unwrap().len(),
        3
    );
    assert_eq!(workbench.current["workload_revision"], 0);
    assert!(snapshot.presentation.properties.iter().any(|property| {
        property.subject == format!("body/{}", body.body_id.as_str())
            && property.name == "workload-revision"
            && property.value == conduit_presentation::PresentationPropertyValue::Count(0)
    }));
    let visible_plots = snapshot
        .presentation
        .subjects
        .iter()
        .filter(|subject| subject.role == PresentationRole::Plot)
        .map(|subject| subject.name.as_str())
        .collect::<Vec<_>>();
    for plot in initial {
        assert!(visible_plots.contains(&plot.checked_plot_id.as_str()));
        assert!(snapshot
            .presentation
            .relationships
            .iter()
            .any(|relationship| {
                relationship.source == format!("body/{}", body.body_id.as_str())
                    && relationship.target == format!("plot/{}", plot.checked_plot_id.as_str())
                    && relationship.kind
                        == conduit_presentation::PresentationRelationshipKind::Contains
            }));
    }
    assert!(!snapshot
        .presentation
        .relationships
        .iter()
        .any(|relationship| {
            relationship.source == format!("body/{}", body.body_id.as_str())
                && relationship.kind == conduit_presentation::PresentationRelationshipKind::Realizes
                && relationship.target.starts_with("plot/")
        }));
    assert_eq!(snapshot.presentation.basis.body_id, Some(body.body_id));
}
