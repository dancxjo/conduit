//! Domain-derived lifecycle specimens for #5355. These checks establish linear
//! and deterministic aural projection, not native/browser Shows or audible speech.

use conduit_body::Body;
use conduit_core::{CheckedPlotId, SignId, SourceDocumentId};
use conduit_presentation::{
    plan_face_utterances, render_linear_presentation, Face, FaceContext, FaceFocus,
    FaceUtteranceProvenance,
};

#[cfg(feature = "plot-catalog")]
mod common;

#[test]
fn one_lulled_body_keeps_context_specific_faces_and_nonvisual_meaning() {
    let body = Body::born(
        SourceDocumentId::from("source/cross-mask-lifecycle"),
        CheckedPlotId::from("checked/cross-mask-lifecycle"),
        1,
        SignId::from("sign/cross-mask-born"),
    )
    .unwrap();
    let before = format!("{body:?}");
    let overview = Face::project(
        &body,
        None,
        7,
        FaceContext::Overview,
        FaceFocus::Body,
        vec![],
    )
    .unwrap();
    let library = Face::project(
        &body,
        None,
        7,
        FaceContext::Library,
        FaceFocus::Body,
        vec![],
    )
    .unwrap();

    assert_ne!(
        overview.presentation.identity,
        library.presentation.identity
    );
    assert_eq!(
        overview.presentation.revision,
        library.presentation.revision
    );
    for face in [&overview, &library] {
        let presentation = &face.presentation;
        assert_eq!(presentation.basis.body_id.as_ref(), Some(&body.body_id));
        assert!(presentation.basis.wake_id.is_none());
        assert!(presentation.basis.active_play_id.is_none());
        let linear = render_linear_presentation(presentation).unwrap();
        let aural = plan_face_utterances(presentation).unwrap();
        assert_eq!(linear.presentation_id, presentation.identity);
        assert_eq!(linear.revision, presentation.revision);
        assert_eq!(aural.source_face_identity, presentation.identity.as_str());
        assert_eq!(aural.source_face_revision, presentation.revision);
        let lines = linear.lines.join("\n");
        let spoken = aural
            .clauses
            .iter()
            .map(|clause| clause.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        for subject in &presentation.subjects {
            assert!(
                aural.clauses.iter().any(|clause| {
                    matches!(&clause.provenance, FaceUtteranceProvenance::Subject(source)
                    if source.identity() == &subject.identity)
                }),
                "aural lost exact subject {}",
                subject.identity
            );
            assert!(
                lines.contains(&subject.name),
                "linear lost {}",
                subject.identity
            );
            assert!(
                spoken.contains(&subject.name),
                "aural lost {}",
                subject.identity
            );
        }
        for action in &presentation.actions {
            assert!(
                aural.clauses.iter().any(|clause| {
                    matches!(&clause.provenance, FaceUtteranceProvenance::Action(source)
                    if source.identity() == &action.identity)
                }),
                "aural lost exact action {}",
                action.identity
            );
            assert!(
                lines.contains(&action.name),
                "linear lost {}",
                action.identity
            );
            assert!(
                spoken.contains(&action.name),
                "aural lost {}",
                action.identity
            );
        }
    }
    assert_eq!(
        format!("{body:?}"),
        before,
        "projection changed owner truth"
    );
}

#[cfg(feature = "plot-catalog")]
#[test]
fn context_switch_stale_show_and_invented_agency_refuse_without_changing_body() {
    use conduit_presentation::{FaceInteraction, FaceInteractionRefusal};

    let body = Body::born(
        SourceDocumentId::from("source/cross-mask-agency"),
        CheckedPlotId::from("checked/cross-mask-agency"),
        1,
        SignId::from("sign/cross-mask-agency-born"),
    )
    .unwrap();
    let before = serde_json::to_value(&body).unwrap();
    let overview = Face::project(
        &body,
        None,
        7,
        FaceContext::Overview,
        FaceFocus::Body,
        vec![],
    )
    .unwrap();
    let library = Face::project(
        &body,
        None,
        7,
        FaceContext::Library,
        FaceFocus::Body,
        vec![],
    )
    .unwrap();
    let current = &overview.presentation;
    // This helper plans a fixture Mask and acknowledges a fixture Show. It
    // proves correlation admission, not execution of a graphical renderer.
    let show = common::available_mask_show(current);
    let library_show = common::available_mask_show(&library.presentation);
    let action = current
        .actions
        .iter()
        .find(|action| action.name == "Wake")
        .unwrap();
    let interaction =
        FaceInteraction::new(current, &show, &action.identity, &action.target, vec![], 1).unwrap();
    assert_eq!(interaction.validate_against(current, &show), Ok(()));
    assert_eq!(
        interaction.validate_against(&library.presentation, &library_show),
        Err(FaceInteractionRefusal::StaleFace)
    );
    assert_eq!(
        interaction.validate_against(current, &library_show),
        Err(FaceInteractionRefusal::StaleShow)
    );
    assert_eq!(
        FaceInteraction::new(current, &show, "invented/start", &action.target, vec![], 2),
        Err(FaceInteractionRefusal::UnknownAction)
    );
    assert_eq!(
        FaceInteraction::new(
            current,
            &show,
            &action.identity,
            "invented/target",
            vec![],
            2
        ),
        Err(FaceInteractionRefusal::WrongTarget)
    );
    assert_eq!(serde_json::to_value(&body).unwrap(), before);
}
