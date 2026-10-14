//! Domain-derived lifecycle specimens for #5355. These checks establish linear
//! and deterministic aural projection, not native/browser Shows or audible speech.

use conduit_body::Body;
use conduit_core::{CheckedPlotId, SignId, SourceDocumentId};
use conduit_face::{
    plan_face_utterances, render_linear_face, BodyFace, FaceContext, FaceFocus,
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
    let overview = BodyFace::project(
        &body,
        None,
        7,
        FaceContext::Overview,
        FaceFocus::Body,
        vec![],
    )
    .unwrap();
    let library = BodyFace::project(
        &body,
        None,
        7,
        FaceContext::Library,
        FaceFocus::Body,
        vec![],
    )
    .unwrap();

    assert_ne!(overview.face.identity, library.face.identity);
    assert_eq!(overview.face.revision, library.face.revision);
    for face in [&overview, &library] {
        let face = &face.face;
        assert_eq!(face.basis.body_id.as_ref(), Some(&body.body_id));
        assert!(face.basis.wake_id.is_none());
        assert!(face.basis.active_play_id.is_none());
        let linear = render_linear_face(face).unwrap();
        let aural = plan_face_utterances(face).unwrap();
        assert_eq!(linear.face_id, face.identity);
        assert_eq!(linear.revision, face.revision);
        assert_eq!(aural.source_face_identity, face.identity.as_str());
        assert_eq!(aural.source_face_revision, face.revision);
        let lines = linear.lines.join("\n");
        let spoken = aural
            .clauses
            .iter()
            .map(|clause| clause.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        for subject in &face.subjects {
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
        for action in &face.actions {
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
    use conduit_face::{
        FaceInteraction, FaceInteractionRefusal, ManifestationFailure, ManifestationLifecycle,
    };

    let body = Body::born(
        SourceDocumentId::from("source/cross-mask-agency"),
        CheckedPlotId::from("checked/cross-mask-agency"),
        1,
        SignId::from("sign/cross-mask-agency-born"),
    )
    .unwrap();
    let before = serde_json::to_value(&body).unwrap();
    let overview = BodyFace::project(
        &body,
        None,
        7,
        FaceContext::Overview,
        FaceFocus::Body,
        vec![],
    )
    .unwrap();
    let library = BodyFace::project(
        &body,
        None,
        7,
        FaceContext::Library,
        FaceFocus::Body,
        vec![],
    )
    .unwrap();
    let current = &overview.face;
    // This helper plans a fixture Mask and acknowledges a fixture Show. It
    // proves correlation admission, not execution of a graphical renderer.
    let show = common::available_mask_show(current);
    let library_show = common::available_mask_show(&library.face);
    let action = current
        .actions
        .iter()
        .find(|action| action.name == "Wake")
        .unwrap();
    let interaction =
        FaceInteraction::new(current, &show, &action.identity, &action.target, vec![], 1).unwrap();
    assert_eq!(interaction.validate_against(current, &show), Ok(()));
    assert_eq!(
        interaction.validate_against(&library.face, &library_show),
        Err(FaceInteractionRefusal::StaleFace)
    );
    assert_eq!(
        interaction.validate_against(current, &library_show),
        Err(FaceInteractionRefusal::StaleShow)
    );
    let refreshed = BodyFace::project(
        &body,
        None,
        8,
        FaceContext::Overview,
        FaceFocus::Body,
        vec![],
    )
    .unwrap();
    let refreshed_show = common::available_mask_show(&refreshed.face);
    assert_eq!(
        interaction.validate_against(&refreshed.face, &refreshed_show),
        Err(FaceInteractionRefusal::StaleFace)
    );
    let closed = show
        .transition(
            ManifestationLifecycle::Closed,
            SignId::from("sign/show-closed"),
        )
        .unwrap();
    assert_eq!(
        interaction.validate_against(current, &closed),
        Err(FaceInteractionRefusal::StaleShow)
    );
    let failed = show
        .fail(
            ManifestationFailure::AdapterUnavailable,
            SignId::from("sign/show-lost"),
        )
        .unwrap();
    assert_eq!(
        interaction.validate_against(current, &failed),
        Err(FaceInteractionRefusal::FailedShow)
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

#[test]
fn one_body_wake_projects_current_state_without_inventing_plan_or_play() {
    let body = Body::born(
        SourceDocumentId::from("source/cross-mask-wake"),
        CheckedPlotId::from("checked/cross-mask-wake"),
        1,
        SignId::from("sign/cross-mask-wake-born"),
    )
    .unwrap();
    let lulled = BodyFace::project(
        &body,
        None,
        11,
        FaceContext::Overview,
        FaceFocus::Body,
        vec![],
    )
    .unwrap()
    .face;
    let (awake_body, wake) = body
        .wake(2, SignId::from("sign/cross-mask-wake-awake"))
        .unwrap();
    let truth = serde_json::to_value(&awake_body).unwrap();
    let awake = BodyFace::project(
        &awake_body,
        Some(&wake),
        12,
        FaceContext::Overview,
        FaceFocus::Body,
        vec![],
    )
    .unwrap()
    .face;
    let recorded_lulled: conduit_face::Face = serde_json::from_str(include_str!(
        "../../../proof/conformance/face-waist/domain-derived/lulled-body-face.json"
    ))
    .unwrap();
    let recorded_awake: conduit_face::Face = serde_json::from_str(include_str!(
        "../../../proof/conformance/face-waist/domain-derived/awake-body-face.json"
    ))
    .unwrap();
    assert_eq!(recorded_lulled, lulled);
    assert_eq!(recorded_awake, awake);
    assert_eq!(lulled.basis.body_id, awake.basis.body_id);
    assert_ne!(lulled.identity, awake.identity);
    assert!(lulled.basis.wake_id.is_none());
    assert_eq!(awake.basis.wake_id.as_ref(), Some(&wake.wake_id));
    for face in [&lulled, &awake] {
        assert!(face.basis.plan_id.is_none());
        assert!(face.basis.active_play_id.is_none());
        let linear = render_linear_face(face).unwrap();
        let aural = plan_face_utterances(face).unwrap();
        assert_eq!(linear.face_id, face.identity);
        assert_eq!(linear.revision, face.revision);
        assert_eq!(aural.source_face_identity, face.identity.as_str());
        assert_eq!(aural.source_face_revision, face.revision);
        for action in &face.actions {
            assert!(aural.clauses.iter().any(
                |clause| matches!(&clause.provenance, FaceUtteranceProvenance::Action(source)
                    if source.identity() == &action.identity)
            ));
            assert!(linear
                .lines
                .iter()
                .any(|line| line.contains(&action.identity)));
        }
    }
    assert_eq!(
        serde_json::to_value(&awake_body).unwrap(),
        truth,
        "renderer projection changed current Body truth"
    );
}
