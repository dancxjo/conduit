#![cfg(feature = "form-catalog")]

mod common;

use common::{checked_renderer_form, host, plan_for, presentation, WAYLAND_RESOURCE};
use conduit_core::{bind_active_play, SignId};
use conduit_presentation::{
    GenerativeInteractionDisposition, GenerativeInteractionProposal, GenerativeInteractionRefusal,
    Manifestation, ManifestationLifecycle, Presentation, PresentationAction,
    PresentationActionAvailability, PresentationDisclosureLevel, PresentationInput,
    PresentationInteraction, PresentationInteractionRefusal, ProposedPresentationInteraction,
    ResolvedGenerativeInteraction, UTF8_TEXT_VALUE_KIND,
};

fn basis(available: bool) -> (Presentation, Manifestation) {
    let form = checked_renderer_form();
    let plan = plan_for(
        &form,
        host(
            "language-host",
            "language-boot",
            "renderer-spoken",
            "presentation/renderer-spoken@1",
            "spoken-mask@1",
            "presentation/base/speech@1",
            WAYLAND_RESOURCE,
        ),
    );
    let base = presentation(&form, &plan);
    let availability = if available {
        PresentationActionAvailability::Available
    } else {
        PresentationActionAvailability::Unavailable {
            reason_code: "not-currently-available".into(),
            explanation: "Sending is not currently available".into(),
        }
    };
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
            name: "Send".into(),
            disclosure: PresentationDisclosureLevel::CurrentAction,
            availability,
        }],
        vec![PresentationInput::text(
            "message/input".into(),
            "patchbay/form".into(),
            8,
            false,
            "Message".into(),
            "message/send".into(),
        )
        .unwrap()],
        base.disclosures,
    )
    .unwrap();
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
        plan.fragments[0].placements[0].placement_id.clone(),
        "patchbay/form".into(),
        "speech/0".into(),
        SignId::from("interpretation/prepared"),
    )
    .unwrap()
    .transition(
        ManifestationLifecycle::Available,
        SignId::from("interpretation/available"),
    )
    .unwrap();
    (presentation, manifestation)
}

fn proposal(
    presentation: &Presentation,
    manifestation: &Manifestation,
    disposition: GenerativeInteractionDisposition,
) -> GenerativeInteractionProposal {
    GenerativeInteractionProposal {
        proposal_identity: "proposal/send-message".into(),
        interpretation_run_identity: "interpretation/run-7".into(),
        source_presentation_identity: presentation.identity.as_str().into(),
        source_presentation_revision: presentation.revision,
        manifestation_identity: manifestation.manifestation_id.as_str().into(),
        interpreter_implementation_identity: "implementation/language-interpreter@1".into(),
        provider_identity: "provider/local-model".into(),
        model_identity: "model/current".into(),
        disposition,
    }
}

fn proposed(value_kind: &str, action_id: &str) -> GenerativeInteractionDisposition {
    GenerativeInteractionDisposition::Proposed(ProposedPresentationInteraction {
        input_id: "message/input".into(),
        action_id: action_id.into(),
        target: "patchbay/form".into(),
        value_kind: value_kind.into(),
        value: b"hello".to_vec(),
    })
}

#[test]
fn exact_language_proposal_resolves_to_the_ordinary_interaction_contract() {
    let (presentation, manifestation) = basis(true);
    let resolved = proposal(
        &presentation,
        &manifestation,
        proposed(UTF8_TEXT_VALUE_KIND, "message/send"),
    )
    .resolve(&presentation, &manifestation, 7)
    .unwrap();
    let expected = PresentationInteraction::new(
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
    assert_eq!(
        resolved,
        ResolvedGenerativeInteraction::Interaction(expected)
    );
}

#[test]
fn stale_unknown_unavailable_and_wrong_kind_proposals_refuse_ordinary_law() {
    let (presentation, manifestation) = basis(true);
    let mut stale = proposal(
        &presentation,
        &manifestation,
        proposed(UTF8_TEXT_VALUE_KIND, "message/send"),
    );
    stale.source_presentation_revision += 1;
    assert_eq!(
        stale.resolve(&presentation, &manifestation, 1),
        Err(GenerativeInteractionRefusal::StalePresentation)
    );

    let mut stale_show = proposal(
        &presentation,
        &manifestation,
        proposed(UTF8_TEXT_VALUE_KIND, "message/send"),
    );
    stale_show.manifestation_identity = "manifestation/replaced".into();
    assert_eq!(
        stale_show.resolve(&presentation, &manifestation, 1),
        Err(GenerativeInteractionRefusal::StaleManifestation)
    );

    let wrong_kind = proposal(
        &presentation,
        &manifestation,
        proposed("value/bytes", "message/send"),
    );
    assert_eq!(
        wrong_kind.resolve(&presentation, &manifestation, 1),
        Err(GenerativeInteractionRefusal::Interaction(
            PresentationInteractionRefusal::WrongValueKind
        ))
    );

    let unknown = proposal(
        &presentation,
        &manifestation,
        proposed(UTF8_TEXT_VALUE_KIND, "message/delete-everything"),
    );
    assert_eq!(
        unknown.resolve(&presentation, &manifestation, 1),
        Err(GenerativeInteractionRefusal::Interaction(
            PresentationInteractionRefusal::UnknownAction
        ))
    );

    let (unavailable_presentation, unavailable_manifestation) = basis(false);
    let unavailable = proposal(
        &unavailable_presentation,
        &unavailable_manifestation,
        proposed(UTF8_TEXT_VALUE_KIND, "message/send"),
    );
    assert_eq!(
        unavailable.resolve(&unavailable_presentation, &unavailable_manifestation, 1),
        Err(GenerativeInteractionRefusal::Interaction(
            PresentationInteractionRefusal::UnavailableAction
        ))
    );
}

#[test]
fn ambiguous_language_yields_clarification_without_an_interaction() {
    let (presentation, manifestation) = basis(true);
    let outcome = proposal(
        &presentation,
        &manifestation,
        GenerativeInteractionDisposition::ClarificationRequired {
            reason_code: "ambiguous-target".into(),
        },
    )
    .resolve(&presentation, &manifestation, 1)
    .unwrap();
    assert_eq!(
        outcome,
        ResolvedGenerativeInteraction::ClarificationRequired {
            reason_code: "ambiguous-target".into()
        }
    );
}
