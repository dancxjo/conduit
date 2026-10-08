#![cfg(feature = "plot-catalog")]

mod common;

use common::{
    available_mask_show, checked_renderer_plot, host, plan_for, presentation, WAYLAND_RESOURCE,
};
use conduit_core::{
    encode_count, CheckedValueContract, IntervalEndpoint, Quantity, QuantityUnit, ValueConstraint,
    COUNT_ENCODED_LEN, COUNT_INFO_ID, DISTANCE_INFO_ID, QUANTITY_ENCODED_LEN,
};
use conduit_plot::TextPatternExpression;
use conduit_presentation::{
    FaceActionArgument, FaceInteraction, FaceInteractionArgument, FaceInteractionDisposition,
    FaceInteractionFailure, FaceInteractionLedger, FaceInteractionRefusal, MaskShow, Presentation,
    PresentationAction, PresentationActionAvailability, PresentationContextBasis,
    PresentationDisclosureLevel, PresentationInteractionContext, PresentationRelationshipKind,
    UTF8_TEXT_VALUE_KIND,
};

fn available_interaction_basis() -> (Presentation, MaskShow) {
    let plot = checked_renderer_plot();
    let plan = plan_for(
        &plot,
        host(
            "linux-host",
            "linux-boot",
            "renderer-wayland",
            "presentation/renderer-wayland@1",
            "patchbay-native/wayland@1",
            "presentation/base/wayland-surface@1",
            WAYLAND_RESOURCE,
        ),
    );
    let base = presentation(&plot, &plan);
    let presentation = Presentation::new_with_semantics(
        base.revision,
        base.basis,
        base.subjects,
        base.relationships,
        base.properties,
        base.text,
        vec![PresentationAction {
            identity: "message/send".into(),
            intent: "message/send".into(),
            target: "patchbay/plot".into(),
            name: "Send".into(),
            arguments: vec![FaceActionArgument::text(
                "message/input".into(),
                "Message".into(),
                1,
                8,
            )
            .unwrap()],
            disclosure: PresentationDisclosureLevel::CurrentAction,
            availability: PresentationActionAvailability::Available,
        }],
        base.disclosures,
    )
    .unwrap();
    let show = available_mask_show(&presentation);
    (presentation, show)
}

fn argument(name: &str, value: &[u8]) -> FaceInteractionArgument {
    FaceInteractionArgument {
        name: name.into(),
        value_kind: UTF8_TEXT_VALUE_KIND.into(),
        value: value.to_vec(),
    }
}

fn bounded_lowercase_pattern(maximum: u16) -> conduit_core::CheckedTextPattern {
    TextPatternExpression::Repeat {
        expression: Box::new(TextPatternExpression::ScalarRange {
            first: 'a' as u32,
            last: 'z' as u32,
        }),
        minimum: 1,
        maximum,
    }
    .compile(u32::from(maximum))
    .expect("reviewed bounded lowercase pattern compiles during checking")
}

fn patterned_argument(maximum: u16) -> FaceActionArgument {
    FaceActionArgument {
        name: "message/input".into(),
        value_name: "Lowercase message".into(),
        contract: CheckedValueContract::new(
            UTF8_TEXT_VALUE_KIND.into(),
            u32::from(maximum),
            vec![
                ValueConstraint::ByteLength {
                    minimum: 1,
                    maximum: u32::from(maximum),
                },
                ValueConstraint::TextPattern {
                    pattern: bounded_lowercase_pattern(maximum),
                    anchored_start: true,
                    anchored_end: true,
                    negated: false,
                },
            ],
        )
        .expect("reviewed Face pattern contract is canonical"),
    }
}

fn ranged_and_member_arguments() -> Vec<FaceActionArgument> {
    vec![
        FaceActionArgument {
            name: "sample/count".into(),
            value_name: "Sample count from two through four".into(),
            contract: CheckedValueContract::new(
                COUNT_INFO_ID.into(),
                COUNT_ENCODED_LEN as u32,
                vec![ValueConstraint::UnsignedRange {
                    minimum: Some(2),
                    maximum: Some(4),
                    minimum_endpoint: IntervalEndpoint::Inclusive,
                    maximum_endpoint: IntervalEndpoint::Inclusive,
                }],
            )
            .expect("reviewed Face count range is canonical"),
        },
        FaceActionArgument {
            name: "sample/distance".into(),
            value_name: "Distance from one through two meters".into(),
            contract: CheckedValueContract::new(
                DISTANCE_INFO_ID.into(),
                QUANTITY_ENCODED_LEN as u32,
                vec![ValueConstraint::QuantityRange {
                    minimum: Some(Quantity::new(1, QuantityUnit::Meter)),
                    maximum: Some(Quantity::new(2, QuantityUnit::Meter)),
                    minimum_endpoint: IntervalEndpoint::Inclusive,
                    maximum_endpoint: IntervalEndpoint::Inclusive,
                }],
            )
            .expect("reviewed Face distance range is canonical"),
        },
        FaceActionArgument {
            name: "sample/mode".into(),
            value_name: "Sampling mode".into(),
            contract: CheckedValueContract::new(
                UTF8_TEXT_VALUE_KIND.into(),
                8,
                vec![ValueConstraint::CanonicalMembership {
                    members: vec![b"careful".to_vec(), b"quick".to_vec()],
                    negated: false,
                }],
            )
            .expect("reviewed Face finite membership is canonical"),
        },
    ]
}

fn rebuild_with_interactions(
    face: &Presentation,
    actions: Vec<PresentationAction>,
) -> Presentation {
    Presentation::new_with_semantics(
        face.revision,
        face.basis.clone(),
        face.subjects.clone(),
        face.relationships.clone(),
        face.properties.clone(),
        face.text.clone(),
        actions,
        face.disclosures.clone(),
    )
    .unwrap()
}

#[test]
fn zero_argument_action_needs_no_counterfeit_empty_input() {
    let (base, _) = available_interaction_basis();
    let mut actions = base.actions.clone();
    actions.push(PresentationAction {
        identity: "message/refresh".into(),
        intent: "message/refresh".into(),
        target: "patchbay/plot".into(),
        name: "Refresh".into(),
        arguments: vec![],
        disclosure: PresentationDisclosureLevel::CurrentAction,
        availability: PresentationActionAvailability::Available,
    });
    let face = rebuild_with_interactions(&base, actions);
    let show = available_mask_show(&face);

    FaceInteraction::new(&face, &show, "message/refresh", "patchbay/plot", vec![], 1).unwrap();
    assert_eq!(
        FaceInteraction::new(
            &face,
            &show,
            "message/refresh",
            "patchbay/plot",
            vec![argument("counterfeit/empty", b"")],
            2,
        ),
        Err(FaceInteractionRefusal::UnknownArgument)
    );
}

#[test]
fn multiple_named_arguments_are_admitted_as_one_complete_atomic_interaction() {
    let (base, _) = available_interaction_basis();
    let mut actions = base.actions.clone();
    actions[0]
        .arguments
        .push(FaceActionArgument::text("message/subject".into(), "Subject".into(), 1, 16).unwrap());
    let face = rebuild_with_interactions(&base, actions);
    let show = available_mask_show(&face);
    let complete = vec![
        argument("message/input", b"hello"),
        argument("message/subject", b"greeting"),
    ];

    let interaction = FaceInteraction::new(
        &face,
        &show,
        "message/send",
        "patchbay/plot",
        complete.clone(),
        1,
    )
    .unwrap();
    assert_eq!(interaction.arguments, complete);
    assert_eq!(
        FaceInteraction::new(
            &face,
            &show,
            "message/send",
            "patchbay/plot",
            vec![argument("message/input", b"hello")],
            2,
        ),
        Err(FaceInteractionRefusal::MissingArgument)
    );
    assert_eq!(
        FaceInteraction::new(
            &face,
            &show,
            "message/send",
            "patchbay/plot",
            vec![
                argument("message/input", b"hello"),
                argument("message/input", b"again"),
            ],
            3,
        ),
        Err(FaceInteractionRefusal::DuplicateArgument)
    );
}

#[test]
fn cancellation_and_renderer_failure_are_terminal_evidence_not_success() {
    let (presentation, show) = available_interaction_basis();
    for failure in [
        FaceInteractionFailure::Cancelled,
        FaceInteractionFailure::AdapterUnavailable,
        FaceInteractionFailure::DeliveryFailed,
    ] {
        let interaction = FaceInteraction::new(
            &presentation,
            &show,
            "message/send",
            "patchbay/plot",
            vec![argument("message/input", b"ok")],
            failure as u64,
        )
        .unwrap();
        let mut ledger = FaceInteractionLedger::new(1, 1).unwrap();
        ledger.admit(interaction).unwrap();
        let evidence = ledger
            .finish_front(FaceInteractionDisposition::Failed(failure))
            .unwrap();
        assert_eq!(
            evidence.disposition,
            FaceInteractionDisposition::Failed(failure)
        );
    }
}

#[test]
fn exact_available_interaction_round_trips_and_evidence_omits_plaintext() {
    let (presentation, show) = available_interaction_basis();
    let interaction = FaceInteraction::new(
        &presentation,
        &show,
        "message/send",
        "patchbay/plot",
        vec![argument("message/input", b"hello")],
        7,
    )
    .unwrap();
    let decoded = FaceInteraction::decode(&interaction.encode()).unwrap();
    decoded.validate_against(&presentation, &show).unwrap();
    let mut stale_interaction = decoded.clone();
    stale_interaction.show_id = "show/stale".into();
    assert_eq!(
        stale_interaction.validate_against(&presentation, &show),
        Err(FaceInteractionRefusal::StaleShow)
    );
    let mut ledger = FaceInteractionLedger::new(1, 1).unwrap();
    ledger.admit(decoded).unwrap();
    let evidence = ledger
        .finish_front(FaceInteractionDisposition::Accepted {
            operation_request_id: "request/7".into(),
        })
        .unwrap();
    assert_eq!(evidence.arguments[0].value_bytes, 5);
    assert!(!format!("{evidence:?}").contains("hello"));
}

#[test]
fn finite_evidence_ack_retains_all_sixty_four_receipts_and_duplicate_guard() {
    let (face, show) = available_interaction_basis();
    let make = |sequence| {
        FaceInteraction::new(
            &face,
            &show,
            "message/send",
            "patchbay/plot",
            vec![argument("message/input", b"ok")],
            sequence,
        )
        .unwrap()
    };
    let mut ledger = FaceInteractionLedger::new(1, 1).unwrap();
    let mut retained = Vec::new();
    for sequence in 0..64 {
        let interaction = make(sequence);
        ledger.admit(interaction.clone()).unwrap();
        let evidence = ledger
            .finish_front(FaceInteractionDisposition::Accepted {
                operation_request_id: format!("request/{sequence}"),
            })
            .unwrap()
            .clone();
        assert_eq!(ledger.evidence(), core::slice::from_ref(&evidence));
        assert_eq!(
            ledger.acknowledge_persisted_evidence_prefix(&[make(sequence + 1).identity]),
            Err(conduit_presentation::FaceEvidenceAckRefusal::MismatchedPrefix)
        );
        assert_eq!(ledger.evidence(), core::slice::from_ref(&evidence));
        retained.push(evidence.clone());
        ledger
            .acknowledge_persisted_evidence_prefix(&[evidence.interaction_id])
            .unwrap();
        assert!(ledger.evidence().is_empty());
        assert_eq!(
            ledger.admit(interaction),
            Err(FaceInteractionRefusal::DuplicateDelivery)
        );
    }
    assert_eq!(retained.len(), 64);
    assert_eq!(
        ledger.admit(make(64)),
        Err(FaceInteractionRefusal::EvidenceExhausted)
    );
}

#[test]
fn invalid_disposition_keeps_queued_interaction_for_retry() {
    let (face, show) = available_interaction_basis();
    let interaction = FaceInteraction::new(
        &face,
        &show,
        "message/send",
        "patchbay/plot",
        vec![argument("message/input", b"ok")],
        12,
    )
    .unwrap();
    let mut ledger = FaceInteractionLedger::new(1, 1).unwrap();
    ledger.admit(interaction.clone()).unwrap();
    assert_eq!(
        ledger.finish_front(FaceInteractionDisposition::Accepted {
            operation_request_id: "".into(),
        }),
        Err(FaceInteractionRefusal::MalformedEncoding)
    );
    assert_eq!(ledger.queued_len(), 1);
    assert!(ledger.evidence().is_empty());
    ledger
        .finish_front(FaceInteractionDisposition::Accepted {
            operation_request_id: "request/12".into(),
        })
        .unwrap();
}

#[test]
fn changing_only_interaction_context_stales_action_and_input_correlation() {
    let (presentation, show) = available_interaction_basis();
    let interaction = FaceInteraction::new(
        &presentation,
        &show,
        "message/send",
        "patchbay/plot",
        vec![argument("message/input", b"hello")],
        8,
    )
    .unwrap();
    let other_context = presentation
        .clone()
        .with_interaction_context(PresentationInteractionContext {
            identity: "presentation/context/other-participant".into(),
            basis: vec![PresentationContextBasis {
                source: "patchbay/plot".into(),
                relationship: PresentationRelationshipKind::Contains,
                target: "patchbay/renderer".into(),
            }],
        })
        .unwrap();

    assert_ne!(presentation.identity, other_context.identity);
    assert_eq!(
        interaction.validate_against(&other_context, &show),
        Err(FaceInteractionRefusal::StaleFace)
    );
}

#[test]
fn stale_wrong_empty_oversize_malformed_duplicate_and_pressure_refuse_distinctly() {
    let (presentation, show) = available_interaction_basis();
    let make = |value: &[u8], sequence| {
        FaceInteraction::new(
            &presentation,
            &show,
            "message/send",
            "patchbay/plot",
            vec![argument("message/input", value)],
            sequence,
        )
    };
    assert_eq!(
        make(b"", 0),
        Err(FaceInteractionRefusal::ViolatedConstraint)
    );
    assert_eq!(
        make(b"123456789", 0),
        Err(FaceInteractionRefusal::OversizeValue)
    );
    assert_eq!(
        make(&[0xff], 0),
        Err(FaceInteractionRefusal::MalformedEncoding)
    );
    assert_eq!(
        FaceInteraction::new(
            &presentation,
            &show,
            "message/send",
            "patchbay/plot",
            vec![argument("missing", b"ok")],
            0
        ),
        Err(FaceInteractionRefusal::MissingArgument)
    );
    let accepted = make(b"ok", 1).unwrap();
    let mut ledger = FaceInteractionLedger::new(1, 2).unwrap();
    ledger.admit(accepted.clone()).unwrap();
    assert_eq!(
        ledger.admit(accepted),
        Err(FaceInteractionRefusal::DuplicateDelivery)
    );
    assert_eq!(
        ledger.admit(make(b"next", 2).unwrap()),
        Err(FaceInteractionRefusal::QueuePressure)
    );
    let mut stale = show.clone();
    stale.presentation_revision += 1;
    assert_eq!(
        FaceInteraction::new(
            &presentation,
            &stale,
            "message/send",
            "patchbay/plot",
            vec![argument("message/input", b"ok")],
            3
        ),
        Err(FaceInteractionRefusal::StaleShow)
    );
}

#[test]
fn checked_text_pattern_is_face_truth_across_admission_identity_and_linear_inspection() {
    let (base, _) = available_interaction_basis();
    let mut actions = base.actions.clone();
    actions[0].arguments = vec![patterned_argument(8)];
    let face = rebuild_with_interactions(&base, actions);
    let show = available_mask_show(&face);

    FaceInteraction::new(
        &face,
        &show,
        "message/send",
        "patchbay/plot",
        vec![argument("message/input", b"conduit")],
        1,
    )
    .expect("an exact full pattern match is admitted");
    assert_eq!(
        FaceInteraction::new(
            &face,
            &show,
            "message/send",
            "patchbay/plot",
            vec![argument("message/input", b"Conduit")],
            2,
        ),
        Err(FaceInteractionRefusal::ViolatedConstraint)
    );
    assert_eq!(
        FaceInteraction::new(
            &face,
            &show,
            "message/send",
            "patchbay/plot",
            vec![argument("message/input", b"toolonggg")],
            3,
        ),
        Err(FaceInteractionRefusal::OversizeValue)
    );
    assert_eq!(
        FaceInteraction::new(
            &face,
            &show,
            "message/send",
            "patchbay/plot",
            vec![argument("message/input", &[0xff])],
            4,
        ),
        Err(FaceInteractionRefusal::MalformedEncoding)
    );

    let mut stale_show = show.clone();
    stale_show.presentation_revision += 1;
    assert_eq!(
        FaceInteraction::new(
            &face,
            &stale_show,
            "message/send",
            "patchbay/plot",
            vec![argument("message/input", b"Conduit")],
            5,
        ),
        Err(FaceInteractionRefusal::StaleShow),
        "Show correlation is checked before value admission"
    );

    let mut other_actions = face.actions.clone();
    other_actions[0].arguments = vec![patterned_argument(7)];
    let other_face = rebuild_with_interactions(&face, other_actions);
    assert_ne!(face.identity, other_face.identity);

    let linear = conduit_presentation::render_linear_presentation(&face).unwrap();
    let action = linear
        .lines
        .iter()
        .find(|line| line.starts_with("ACTION "))
        .expect("deterministic-linear Mask exposes the action contract");
    assert!(action.contains("TextPattern"));
    assert!(action.contains("maximum_input_characters: 8"));
}

#[test]
fn ranges_and_finite_membership_are_face_truth_across_admission_and_linear_inspection() {
    let (base, _) = available_interaction_basis();
    let mut actions = base.actions.clone();
    actions[0].arguments = ranged_and_member_arguments();
    let face = rebuild_with_interactions(&base, actions);
    let show = available_mask_show(&face);
    let interaction = |count: u64, distance: Quantity, mode: &[u8], sequence| {
        FaceInteraction::new(
            &face,
            &show,
            "message/send",
            "patchbay/plot",
            vec![
                FaceInteractionArgument {
                    name: "sample/count".into(),
                    value_kind: COUNT_INFO_ID.into(),
                    value: encode_count(count).to_vec(),
                },
                FaceInteractionArgument {
                    name: "sample/distance".into(),
                    value_kind: DISTANCE_INFO_ID.into(),
                    value: distance.encode().to_vec(),
                },
                argument("sample/mode", mode),
            ],
            sequence,
        )
    };

    interaction(
        3,
        Quantity::new(150, QuantityUnit::Centimeter),
        b"careful",
        1,
    )
    .expect("all canonical values satisfy the Face contracts");
    assert_eq!(
        interaction(
            5,
            Quantity::new(150, QuantityUnit::Centimeter),
            b"careful",
            2,
        ),
        Err(FaceInteractionRefusal::ViolatedConstraint)
    );
    assert_eq!(
        interaction(3, Quantity::new(3, QuantityUnit::Meter), b"careful", 3,),
        Err(FaceInteractionRefusal::ViolatedConstraint)
    );
    assert_eq!(
        interaction(
            3,
            Quantity::new(150, QuantityUnit::Centimeter),
            b"reckless",
            4,
        ),
        Err(FaceInteractionRefusal::ViolatedConstraint)
    );

    let linear = conduit_presentation::render_linear_presentation(&face).unwrap();
    let action = linear
        .lines
        .iter()
        .find(|line| line.starts_with("ACTION "))
        .expect("deterministic-linear Mask exposes the exact action contracts");
    for constraint in ["UnsignedRange", "QuantityRange", "CanonicalMembership"] {
        assert!(
            action.contains(constraint),
            "missing {constraint}: {action}"
        );
    }
}
