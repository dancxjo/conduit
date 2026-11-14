use conduit_body::Body;
use conduit_face::{
    render_linear_face, BodyFace, FaceContext, FaceDisclosureLevel, FaceFocus, FaceNames,
    FaceResidentPlotName, FaceRole,
};

#[test]
fn library_exposes_named_resident_plots_without_changing_body_or_action_truth() {
    let body = Body::born(
        "source/clock".into(),
        "checked/clock".into(),
        1,
        "sign/born".into(),
    )
    .unwrap();
    let before = body.clone();
    let resident = &body.workset.plots()[0];
    let names = [FaceResidentPlotName {
        source_document_id: &resident.source_document_id,
        checked_plot_id: &resident.checked_plot_id,
        name: "House clock",
    }];
    let project = |context| {
        BodyFace::project_with_names(
            &body,
            None,
            7,
            context,
            FaceFocus::Body,
            vec![],
            FaceNames {
                body_name: Some("House"),
                resident_plots: &names,
            },
        )
        .unwrap()
        .face
    };
    let overview = project(FaceContext::Overview);
    let library = project(FaceContext::Library);
    overview.validate().unwrap();
    library.validate().unwrap();
    let plot = library
        .subjects
        .iter()
        .find(|s| s.role == FaceRole::Plot)
        .unwrap();
    assert_eq!(plot.name, "House clock");
    assert!(library
        .disclosures
        .iter()
        .any(|d| d.subject == plot.identity && d.level == FaceDisclosureLevel::Primary));
    assert!(overview
        .disclosures
        .iter()
        .any(|d| d.subject == plot.identity && d.level == FaceDisclosureLevel::Context));
    assert!(library
        .text
        .iter()
        .any(|t| t.text == "House · Plots library: one resident plot."));
    assert!(overview
        .text
        .iter()
        .any(|t| t.text == "House is lulled with one resident plot."));
    assert_eq!(overview.basis, library.basis);
    assert_eq!(overview.actions, library.actions);
    assert_eq!(body, before);
    let linear = render_linear_face(&library).unwrap();
    assert!(linear
        .lines
        .iter()
        .any(|line| line.contains("Plots library")));
    assert!(linear.lines.iter().any(|line| line.contains("House clock")));
}
