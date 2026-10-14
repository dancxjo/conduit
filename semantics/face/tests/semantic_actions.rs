extern crate alloc;

use alloc::{string::String, vec, vec::Vec};
use conduit_face::{
    render_linear_face, Face, FaceAction, FaceActionAvailability, FaceActionRefusal, FaceBasis,
    FaceDisclosure, FaceDisclosureLevel, FaceError, FaceRole, FaceSubject, MAX_FACE_REASON_BYTES,
};

fn subject(identity: &str) -> FaceSubject {
    FaceSubject {
        identity: identity.into(),
        role: FaceRole::Plot,
        name: "Example checked plot".into(),
    }
}

fn action(identity: &str, target: &str, availability: FaceActionAvailability) -> FaceAction {
    FaceAction {
        identity: identity.into(),
        intent: "conduit.intent/open@1".into(),
        target: target.into(),
        name: "Open".into(),
        arguments: vec![],
        disclosure: FaceDisclosureLevel::CurrentAction,
        availability,
    }
}

fn face(actions: Vec<FaceAction>, disclosures: Vec<FaceDisclosure>) -> Result<Face, FaceError> {
    Face::new_with_semantics(
        7,
        FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: None,
            checked_plot_id: None,
            expanded_plot_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![],
        },
        vec![subject("plot/example")],
        vec![],
        vec![],
        vec![],
        actions,
        disclosures,
    )
}

#[test]
fn semantic_changes_are_identity_bearing_and_linear_projection_preserves_them() {
    let available = action(
        "action/open",
        "plot/example",
        FaceActionAvailability::Available,
    );
    let primary = FaceDisclosure {
        subject: "plot/example".into(),
        level: FaceDisclosureLevel::Primary,
    };
    let first = face(vec![available.clone()], vec![primary.clone()]).unwrap();
    let repeat = face(vec![available], vec![primary]).unwrap();
    assert_eq!(first.identity, repeat.identity);

    let unavailable = face(
        vec![action(
            "action/open",
            "plot/example",
            FaceActionAvailability::Unavailable {
                reason_code: "authority/not-admitted".into(),
                explanation: "No admitted authority can open this plot.".into(),
            },
        )],
        vec![FaceDisclosure {
            subject: "plot/example".into(),
            level: FaceDisclosureLevel::ExactProvenance,
        }],
    )
    .unwrap();
    assert_ne!(first.identity, unavailable.identity);

    let output = render_linear_face(&unavailable).unwrap().lines.join("\n");
    assert!(output.contains("ACTION id=\"action/open\""));
    assert!(output.contains("unavailable code=\"authority/not-admitted\""));
    assert!(output.contains("DISCLOSURE subject=\"plot/example\" level=ExactProvenance"));
}

#[test]
fn action_resolution_is_read_only_and_fails_closed() {
    let value = face(
        vec![
            action(
                "action/open",
                "plot/example",
                FaceActionAvailability::Unavailable {
                    reason_code: "authority/not-admitted".into(),
                    explanation: "No admitted authority can open this plot.".into(),
                },
            ),
            action(
                "action/born",
                "plot/example",
                FaceActionAvailability::Refused {
                    reason_code: "body/already-exists".into(),
                    explanation: "A body already exists for this plot.".into(),
                },
            ),
        ],
        vec![],
    )
    .unwrap();
    let identity = value.identity.clone();
    assert_eq!(
        value.resolve_action(6, "action/open"),
        Err(FaceActionRefusal::StaleRevision)
    );
    assert_eq!(
        value.resolve_action(7, "action/missing"),
        Err(FaceActionRefusal::UnknownAction)
    );
    assert_eq!(
        value.resolve_action(7, "action/open"),
        Err(FaceActionRefusal::Unavailable {
            reason_code: "authority/not-admitted".into()
        })
    );
    assert_eq!(
        value.resolve_action(7, "action/born"),
        Err(FaceActionRefusal::Refused {
            reason_code: "body/already-exists".into()
        })
    );
    assert_eq!(value.identity, identity);
}

#[test]
fn malformed_actions_and_disclosures_fail_closed() {
    let available = action(
        "action/open",
        "plot/example",
        FaceActionAvailability::Available,
    );
    assert_eq!(
        face(vec![available.clone(), available], vec![]),
        Err(FaceError::DuplicateAction)
    );
    assert_eq!(
        face(
            vec![action(
                "action/open",
                "plot/unknown",
                FaceActionAvailability::Available
            )],
            vec![]
        ),
        Err(FaceError::UnknownActionTarget)
    );
    assert_eq!(
        face(
            vec![action(
                "action/open",
                "plot/example",
                FaceActionAvailability::Unavailable {
                    reason_code: "reason/large".into(),
                    explanation: String::from_utf8(vec![b'x'; MAX_FACE_REASON_BYTES + 1]).unwrap(),
                }
            )],
            vec![]
        ),
        Err(FaceError::ReasonTooLong)
    );
    assert_eq!(
        face(
            vec![],
            vec![FaceDisclosure {
                subject: "plot/unknown".into(),
                level: FaceDisclosureLevel::Context,
            }]
        ),
        Err(FaceError::UnknownDisclosureSubject)
    );
}
