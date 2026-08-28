extern crate alloc;

use alloc::{string::String, vec, vec::Vec};
use conduit_presentation::{
    render_linear_presentation, Presentation, PresentationAction, PresentationActionAvailability,
    PresentationActionRefusal, PresentationBasis, PresentationDisclosure,
    PresentationDisclosureLevel, PresentationError, PresentationRole, PresentationSubject,
    MAX_PRESENTATION_REASON_BYTES,
};

fn subject(identity: &str) -> PresentationSubject {
    PresentationSubject {
        identity: identity.into(),
        role: PresentationRole::Seed,
        label: "Example Seed".into(),
        accessibility_name: "Example checked Seed".into(),
    }
}

fn action(
    identity: &str,
    target: &str,
    availability: PresentationActionAvailability,
) -> PresentationAction {
    PresentationAction {
        identity: identity.into(),
        intent: "conduit.intent/open@1".into(),
        target: target.into(),
        label: "Open".into(),
        disclosure: PresentationDisclosureLevel::CurrentAction,
        availability,
    }
}

fn presentation(
    actions: Vec<PresentationAction>,
    disclosures: Vec<PresentationDisclosure>,
) -> Result<Presentation, PresentationError> {
    Presentation::new_with_semantics(
        7,
        PresentationBasis {
            seed_id: None,
            body_id: None,
            wake_id: None,
            source_document_id: None,
            checked_form_id: None,
            expanded_form_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![],
        },
        vec![subject("seed/example")],
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
        "seed/example",
        PresentationActionAvailability::Available,
    );
    let primary = PresentationDisclosure {
        subject: "seed/example".into(),
        level: PresentationDisclosureLevel::Primary,
    };
    let first = presentation(vec![available.clone()], vec![primary.clone()]).unwrap();
    let repeat = presentation(vec![available], vec![primary]).unwrap();
    assert_eq!(first.identity, repeat.identity);

    let unavailable = presentation(
        vec![action(
            "action/open",
            "seed/example",
            PresentationActionAvailability::Unavailable {
                reason_code: "authority/not-admitted".into(),
                explanation: "No admitted authority can open this Seed.".into(),
            },
        )],
        vec![PresentationDisclosure {
            subject: "seed/example".into(),
            level: PresentationDisclosureLevel::ExactProvenance,
        }],
    )
    .unwrap();
    assert_ne!(first.identity, unavailable.identity);

    let output = render_linear_presentation(&unavailable)
        .unwrap()
        .lines
        .join("\n");
    assert!(output.contains("ACTION id=\"action/open\""));
    assert!(output.contains("unavailable code=\"authority/not-admitted\""));
    assert!(output.contains("DISCLOSURE subject=\"seed/example\" level=ExactProvenance"));
}

#[test]
fn action_resolution_is_read_only_and_fails_closed() {
    let value = presentation(
        vec![
            action(
                "action/open",
                "seed/example",
                PresentationActionAvailability::Unavailable {
                    reason_code: "authority/not-admitted".into(),
                    explanation: "No admitted authority can open this Seed.".into(),
                },
            ),
            action(
                "action/born",
                "seed/example",
                PresentationActionAvailability::Refused {
                    reason_code: "body/already-exists".into(),
                    explanation: "A Body already exists for this Seed.".into(),
                },
            ),
        ],
        vec![],
    )
    .unwrap();
    let identity = value.identity.clone();
    assert_eq!(
        value.resolve_action(6, "action/open"),
        Err(PresentationActionRefusal::StaleRevision)
    );
    assert_eq!(
        value.resolve_action(7, "action/missing"),
        Err(PresentationActionRefusal::UnknownAction)
    );
    assert_eq!(
        value.resolve_action(7, "action/open"),
        Err(PresentationActionRefusal::Unavailable {
            reason_code: "authority/not-admitted".into()
        })
    );
    assert_eq!(
        value.resolve_action(7, "action/born"),
        Err(PresentationActionRefusal::Refused {
            reason_code: "body/already-exists".into()
        })
    );
    assert_eq!(value.identity, identity);
}

#[test]
fn malformed_actions_and_disclosures_fail_closed() {
    let available = action(
        "action/open",
        "seed/example",
        PresentationActionAvailability::Available,
    );
    assert_eq!(
        presentation(vec![available.clone(), available], vec![]),
        Err(PresentationError::DuplicateAction)
    );
    assert_eq!(
        presentation(
            vec![action(
                "action/open",
                "seed/unknown",
                PresentationActionAvailability::Available
            )],
            vec![]
        ),
        Err(PresentationError::UnknownActionTarget)
    );
    assert_eq!(
        presentation(
            vec![action(
                "action/open",
                "seed/example",
                PresentationActionAvailability::Unavailable {
                    reason_code: "reason/large".into(),
                    explanation: String::from_utf8(vec![b'x'; MAX_PRESENTATION_REASON_BYTES + 1])
                        .unwrap(),
                }
            )],
            vec![]
        ),
        Err(PresentationError::ReasonTooLong)
    );
    assert_eq!(
        presentation(
            vec![],
            vec![PresentationDisclosure {
                subject: "seed/unknown".into(),
                level: PresentationDisclosureLevel::Context,
            }]
        ),
        Err(PresentationError::UnknownDisclosureSubject)
    );
}
