#![cfg(feature = "form-catalog")]

mod common;

use common::{checked_renderer_form, host, plan_for, presentation, WAYLAND_RESOURCE};
use conduit_core::{bind_active_play, SignId};
use conduit_presentation::{
    Manifestation, ManifestationLifecycle, Presentation, PresentationAction,
    PresentationActionAvailability, PresentationDisclosureLevel, PresentationInput,
    PresentationInteraction, PresentationInteractionDisposition, PresentationInteractionFailure,
    PresentationInteractionLedger, PresentationInteractionRefusal, UTF8_TEXT_VALUE_KIND,
};

fn available_interaction_basis() -> (Presentation, Manifestation) {
    let form = checked_renderer_form();
    let plan = plan_for(
        &form,
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
    let base = presentation(&form, &plan);
    let presentation = Presentation::new_with_interactions(
        base.revision,
        base.basis,
        base.subjects,
        base.relationships,
        base.properties,
        base.text,
        vec![PresentationAction {
            identity: "message/send".into(),
            intent: "message/send".into(),
            target: "patchbay/form".into(),
            label: "Send".into(),
            disclosure: PresentationDisclosureLevel::CurrentAction,
            availability: PresentationActionAvailability::Available,
        }],
        vec![PresentationInput {
            identity: "message/input".into(),
            target: "patchbay/form".into(),
            value_kind: UTF8_TEXT_VALUE_KIND.into(),
            maximum_bytes: 8,
            allow_empty: false,
            label: "Message".into(),
            accessibility_name: "Message".into(),
            submit_action: "message/send".into(),
        }],
        base.disclosures,
    )
    .unwrap();
    let placement = plan.fragments[0].placements[0].placement_id.clone();
    let active = bind_active_play(
        &plan.plan_id,
        &plan.fragments[0].host_id,
        &plan.fragments[0].boot_id,
        1,
    );
    let manifestation = Manifestation::prepared(
        &presentation,
        &plan,
        active,
        placement,
        "patchbay/form".into(),
        "display/0".into(),
        SignId::from("interaction/prepared"),
    )
    .unwrap()
    .transition(
        ManifestationLifecycle::Available,
        SignId::from("interaction/available"),
    )
    .unwrap();
    (presentation, manifestation)
}

#[test]
fn cancellation_and_renderer_failure_are_terminal_evidence_not_success() {
    let (presentation, manifestation) = available_interaction_basis();
    for failure in [
        PresentationInteractionFailure::Cancelled,
        PresentationInteractionFailure::AdapterUnavailable,
        PresentationInteractionFailure::DeliveryFailed,
    ] {
        let interaction = PresentationInteraction::new(
            &presentation,
            &manifestation,
            "message/input",
            "message/send",
            "patchbay/form",
            UTF8_TEXT_VALUE_KIND,
            b"ok",
            failure as u64,
        )
        .unwrap();
        let mut ledger = PresentationInteractionLedger::new(1, 1).unwrap();
        ledger.admit(interaction).unwrap();
        let evidence = ledger
            .finish_front(PresentationInteractionDisposition::Failed(failure))
            .unwrap();
        assert_eq!(
            evidence.disposition,
            PresentationInteractionDisposition::Failed(failure)
        );
    }
}

#[test]
fn exact_available_interaction_round_trips_and_evidence_omits_plaintext() {
    let (presentation, manifestation) = available_interaction_basis();
    let interaction = PresentationInteraction::new(
        &presentation,
        &manifestation,
        "message/input",
        "message/send",
        "patchbay/form",
        UTF8_TEXT_VALUE_KIND,
        b"hello",
        7,
    )
    .unwrap();
    let decoded = PresentationInteraction::decode(&interaction.encode()).unwrap();
    decoded
        .validate_against(&presentation, &manifestation)
        .unwrap();
    let mut stale_interaction = decoded.clone();
    stale_interaction.manifestation_id = "manifestation/stale".into();
    assert_eq!(
        stale_interaction.validate_against(&presentation, &manifestation),
        Err(PresentationInteractionRefusal::StaleManifestation)
    );
    let mut ledger = PresentationInteractionLedger::new(1, 1).unwrap();
    ledger.admit(decoded).unwrap();
    let evidence = ledger
        .finish_front(PresentationInteractionDisposition::Accepted {
            operation_request_id: "request/7".into(),
        })
        .unwrap();
    assert_eq!(evidence.value_bytes, 5);
    assert!(!format!("{evidence:?}").contains("hello"));
}

#[test]
fn stale_wrong_empty_oversize_malformed_duplicate_and_pressure_refuse_distinctly() {
    let (presentation, manifestation) = available_interaction_basis();
    let make = |value: &[u8], sequence| {
        PresentationInteraction::new(
            &presentation,
            &manifestation,
            "message/input",
            "message/send",
            "patchbay/form",
            UTF8_TEXT_VALUE_KIND,
            value,
            sequence,
        )
    };
    assert_eq!(
        make(b"", 0),
        Err(PresentationInteractionRefusal::EmptyValue)
    );
    assert_eq!(
        make(b"123456789", 0),
        Err(PresentationInteractionRefusal::OversizeValue)
    );
    assert_eq!(
        make(&[0xff], 0),
        Err(PresentationInteractionRefusal::MalformedEncoding)
    );
    assert_eq!(
        PresentationInteraction::new(
            &presentation,
            &manifestation,
            "missing",
            "message/send",
            "patchbay/form",
            UTF8_TEXT_VALUE_KIND,
            b"ok",
            0
        ),
        Err(PresentationInteractionRefusal::UnknownInput)
    );
    let accepted = make(b"ok", 1).unwrap();
    let mut ledger = PresentationInteractionLedger::new(1, 2).unwrap();
    ledger.admit(accepted.clone()).unwrap();
    assert_eq!(
        ledger.admit(accepted),
        Err(PresentationInteractionRefusal::DuplicateDelivery)
    );
    assert_eq!(
        ledger.admit(make(b"next", 2).unwrap()),
        Err(PresentationInteractionRefusal::QueuePressure)
    );
    let mut stale = manifestation.clone();
    stale.presentation_revision += 1;
    assert_eq!(
        PresentationInteraction::new(
            &presentation,
            &stale,
            "message/input",
            "message/send",
            "patchbay/form",
            UTF8_TEXT_VALUE_KIND,
            b"ok",
            3
        ),
        Err(PresentationInteractionRefusal::StaleManifestation)
    );
}
