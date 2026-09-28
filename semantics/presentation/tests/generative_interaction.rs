#![cfg(feature = "form-catalog")]

mod common;

use common::{
    available_mask_show, checked_renderer_form, host, plan_for, presentation, WAYLAND_RESOURCE,
};
use conduit_presentation::{
    FaceActionArgument, FaceInteraction, FaceInteractionArgument, FaceInteractionRefusal,
    GenerativeInteractionDisposition, GenerativeInteractionProposal, GenerativeInteractionRefusal,
    MaskShow, Presentation, PresentationAction, PresentationActionAvailability,
    PresentationDisclosureLevel, ProposedFaceInteraction, ResolvedGenerativeInteraction,
    UTF8_TEXT_VALUE_KIND,
};

fn basis(available: bool) -> (Presentation, MaskShow) {
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
            target: "patchbay/form".into(),
            name: "Send".into(),
            arguments: vec![FaceActionArgument::text(
                "message/input".into(),
                "Message".into(),
                1,
                8,
            )
            .unwrap()],
            disclosure: PresentationDisclosureLevel::CurrentAction,
            availability,
        }],
        base.disclosures,
    )
    .unwrap();
    let show = available_mask_show(&presentation);
    (presentation, show)
}

fn proposal(
    presentation: &Presentation,
    show: &MaskShow,
    disposition: GenerativeInteractionDisposition,
) -> GenerativeInteractionProposal {
    GenerativeInteractionProposal {
        proposal_identity: "proposal/send-message".into(),
        interpretation_run_identity: "interpretation/run-7".into(),
        source_face_identity: presentation.identity.as_str().into(),
        source_face_revision: presentation.revision,
        show_identity: show.show_id.as_str().into(),
        interpreter_implementation_identity: "implementation/language-interpreter@1".into(),
        provider_identity: "provider/local-model".into(),
        model_identity: "model/current".into(),
        disposition,
    }
}

fn proposed(value_kind: &str, action_id: &str) -> GenerativeInteractionDisposition {
    GenerativeInteractionDisposition::Proposed(ProposedFaceInteraction {
        action_id: action_id.into(),
        target: "patchbay/form".into(),
        arguments: vec![FaceInteractionArgument {
            name: "message/input".into(),
            value_kind: value_kind.into(),
            value: b"hello".to_vec(),
        }],
    })
}

#[test]
fn exact_language_proposal_resolves_to_the_ordinary_interaction_contract() {
    let (presentation, show) = basis(true);
    let resolved = proposal(
        &presentation,
        &show,
        proposed(UTF8_TEXT_VALUE_KIND, "message/send"),
    )
    .resolve(&presentation, &show, 7)
    .unwrap();
    let expected = FaceInteraction::new(
        &presentation,
        &show,
        "message/send",
        "patchbay/form",
        vec![FaceInteractionArgument {
            name: "message/input".into(),
            value_kind: UTF8_TEXT_VALUE_KIND.into(),
            value: b"hello".to_vec(),
        }],
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
    let (presentation, show) = basis(true);
    let mut stale = proposal(
        &presentation,
        &show,
        proposed(UTF8_TEXT_VALUE_KIND, "message/send"),
    );
    stale.source_face_revision += 1;
    assert_eq!(
        stale.resolve(&presentation, &show, 1),
        Err(GenerativeInteractionRefusal::StaleFace)
    );

    let mut stale_show = proposal(
        &presentation,
        &show,
        proposed(UTF8_TEXT_VALUE_KIND, "message/send"),
    );
    stale_show.show_identity = "show/replaced".into();
    assert_eq!(
        stale_show.resolve(&presentation, &show, 1),
        Err(GenerativeInteractionRefusal::StaleShow)
    );

    let wrong_kind = proposal(
        &presentation,
        &show,
        proposed("value/bytes", "message/send"),
    );
    assert_eq!(
        wrong_kind.resolve(&presentation, &show, 1),
        Err(GenerativeInteractionRefusal::Interaction(
            FaceInteractionRefusal::WrongValueKind
        ))
    );

    let unknown = proposal(
        &presentation,
        &show,
        proposed(UTF8_TEXT_VALUE_KIND, "message/delete-everything"),
    );
    assert_eq!(
        unknown.resolve(&presentation, &show, 1),
        Err(GenerativeInteractionRefusal::Interaction(
            FaceInteractionRefusal::UnknownAction
        ))
    );

    let (unavailable_presentation, unavailable_show) = basis(false);
    let unavailable = proposal(
        &unavailable_presentation,
        &unavailable_show,
        proposed(UTF8_TEXT_VALUE_KIND, "message/send"),
    );
    assert_eq!(
        unavailable.resolve(&unavailable_presentation, &unavailable_show, 1),
        Err(GenerativeInteractionRefusal::Interaction(
            FaceInteractionRefusal::UnavailableAction
        ))
    );
}

#[test]
fn ambiguous_language_yields_clarification_without_an_interaction() {
    let (presentation, show) = basis(true);
    let outcome = proposal(
        &presentation,
        &show,
        GenerativeInteractionDisposition::ClarificationRequired {
            reason_code: "ambiguous-target".into(),
        },
    )
    .resolve(&presentation, &show, 1)
    .unwrap();
    assert_eq!(
        outcome,
        ResolvedGenerativeInteraction::ClarificationRequired {
            reason_code: "ambiguous-target".into()
        }
    );
}
