//! #5355: source-derived graph semantics through nonvisual production readers.
//! The declared Text Host adapter is a fixture; this is not live Mask evidence.
use conduit_face::{plan_face_utterances, render_linear_face, FaceRole, FaceUtteranceProvenance};

#[test]
fn source_derived_patchbay_keeps_gear_port_cord_meaning_in_nonvisual_readers() {
    let face = crate::portable_demo::portable_demonstration_with_profile(
        crate::host_adapter::test_host_adapter(),
        crate::PatchbayHostProfile::Text,
    )
    .unwrap()
    .0;
    face.validate().unwrap();
    let specimen: conduit_face::Face = serde_json::from_str(include_str!(
        "../../../../../proof/conformance/face-waist/domain-derived/patchbay-face.json"
    ))
    .unwrap();
    assert_eq!(
        specimen, face,
        "shared Patchbay input differs from exact source projection"
    );
    assert!(face.basis.body_id.is_some());
    assert!(face.basis.checked_plot_id.is_some());
    assert!(face.basis.expanded_plot_id.is_some());
    assert!(face.basis.plan_id.is_some());
    let linear = render_linear_face(&face).unwrap();
    let aural = plan_face_utterances(&face).unwrap();
    assert_eq!(linear.face_id, face.identity);
    assert_eq!(linear.revision, face.revision);
    assert_eq!(aural.source_face_identity, face.identity.as_str());
    assert_eq!(aural.source_face_revision, face.revision);
    for role in [FaceRole::Gear, FaceRole::Port, FaceRole::Cord] {
        let subjects = face
            .subjects
            .iter()
            .filter(|subject| subject.role == role)
            .collect::<Vec<_>>();
        assert!(
            !subjects.is_empty(),
            "the expanded hello graph must supply {role:?}"
        );
        for subject in subjects {
            assert!(
                linear.lines.iter().any(|line| line.contains(&subject.name)),
                "linear reader lost {}",
                subject.identity
            );
            assert!(
                aural.clauses.iter().any(
                    |clause| matches!(&clause.provenance, FaceUtteranceProvenance::Subject(source)
                    if source.identity() == &subject.identity)
                        && clause.text.contains(&subject.name)
                ),
                "aural reader lost {}",
                subject.identity
            );
        }
    }
    for (index, _) in face.relationships.iter().enumerate() {
        assert!(
            aural.clauses.iter().any(
                |clause| matches!(&clause.provenance, FaceUtteranceProvenance::Relationship(source)
                if *source.index() == index as u32)
            ),
            "lost graph relationship {index}"
        );
    }
    for action in &face.actions {
        assert!(
            aural.clauses.iter().any(
                |clause| matches!(&clause.provenance, FaceUtteranceProvenance::Action(source)
                if source.identity() == &action.identity)
            ),
            "lost typed action {}",
            action.identity
        );
    }
}
