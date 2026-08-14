use crate::{
    FormEditor, InteractionDisposition, PatchbayAction, PatchbayEdit, PatchbayEditBasis,
    PatchbayGraph, PatchbayInteraction, PatchbayInteractionRequest, PatchbayInvocationOutcome,
    PatchbayRefusal,
};
use conduit_core::{BootId, ConfigurationValue, ExpandedFormId, HostId};
use conduit_kernel::KernelEventKind;
use std::path::PathBuf;

fn invocation_presentation(
    action: PatchbayAction,
    target: &str,
) -> conduit_presentation::Presentation {
    conduit_presentation::Presentation::new_with_semantics(
        17,
        conduit_presentation::PresentationBasis {
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
        vec![conduit_presentation::PresentationSubject {
            identity: target.into(),
            role: conduit_presentation::PresentationRole::Form,
            label: "Target".into(),
            accessibility_name: "Invocation target".into(),
        }],
        vec![],
        vec![],
        vec![],
        vec![conduit_presentation::PresentationAction {
            identity: format!("action/{}/current", action.as_str()),
            intent: action.presentation_intent().into(),
            target: target.into(),
            label: action.as_str().into(),
            disclosure: conduit_presentation::PresentationDisclosureLevel::CurrentAction,
            availability: conduit_presentation::PresentationActionAvailability::Available,
        }],
        vec![],
    )
    .unwrap()
}

fn count_graph() -> PatchbayGraph {
    let editor = FormEditor::from_source(
        PathBuf::from("count.conduit"),
        include_str!("../../../examples/count.conduit").into(),
    )
    .unwrap();
    PatchbayGraph::from_expanded(&editor.expand_form("count-demo").unwrap()).unwrap()
}

fn interaction() -> PatchbayInteraction {
    PatchbayInteraction::new(HostId::from("host/test"), BootId::from("boot/test"))
}

#[test]
fn exact_selection_is_checked_planned_and_played_before_state_changes() {
    let graph = count_graph();
    let subject = graph
        .subject_ref(graph.subject_identities().nth(1).unwrap())
        .unwrap();
    let mut interaction = interaction();
    let request_id = interaction.next_request_id("select").unwrap();
    let request = PatchbayInteractionRequest::select(request_id, &subject).unwrap();

    let receipt = interaction
        .execute(Some(&graph), request.clone(), |_| {
            panic!("selection is not invocation")
        })
        .unwrap();
    assert_eq!(receipt.request, request);
    assert_eq!(receipt.disposition, InteractionDisposition::Succeeded);
    assert_eq!(interaction.selected(), Some(&subject));
    assert!(!receipt.source_document_id.as_str().is_empty());
    assert!(!receipt.checked_form_id.as_str().is_empty());
    assert!(!receipt.expanded_form_id.as_str().is_empty());
    assert!(!receipt.plan_id.as_str().is_empty());
    assert!(!receipt.active_play_id.as_str().is_empty());
    assert_eq!(receipt.plan.fragments.len(), 1);
    assert_eq!(receipt.plan.fragments[0].placements.len(), 2);
    assert_eq!(receipt.plan.fragments[0].connections.len(), 1);
    assert!(receipt
        .signs
        .iter()
        .any(|event| event.kind == KernelEventKind::HostOperationRequested));
    assert!(receipt
        .signs
        .iter()
        .any(|event| event.kind == KernelEventKind::HostOperationCompleted));
    let inspection = interaction.lines().join("\n");
    assert!(inspection.contains("kind=interaction/select"));
    assert!(inspection.contains("gears=request,apply"));
    assert!(inspection.contains("port=request:interaction/request@1"));
    assert!(inspection.contains("plan="));
    assert!(inspection.contains("play="));
    assert!(inspection.contains("signs="));
}

#[test]
fn stale_and_unknown_selection_refuse_without_replacing_canonical_selection() {
    let graph = count_graph();
    let subject = graph
        .subject_ref(graph.subject_identities().next().unwrap())
        .unwrap();
    let mut interaction = interaction();
    let accepted = PatchbayInteractionRequest::select(
        interaction.next_request_id("select").unwrap(),
        &subject,
    )
    .unwrap();
    interaction
        .execute(Some(&graph), accepted, |_| {
            PatchbayInvocationOutcome::Succeeded
        })
        .unwrap();

    let mut stale = subject.clone();
    stale.expanded_form_id = ExpandedFormId::from("expanded/stale");
    let request =
        PatchbayInteractionRequest::select(interaction.next_request_id("select").unwrap(), &stale)
            .unwrap();
    let stale_receipt = interaction
        .execute(Some(&graph), request, |_| {
            PatchbayInvocationOutcome::Succeeded
        })
        .unwrap();
    assert_eq!(
        stale_receipt.disposition,
        InteractionDisposition::Refused(PatchbayRefusal::StalePresentation)
    );
    assert_eq!(interaction.selected(), Some(&subject));

    let mut unknown = subject.clone();
    unknown.subject_identity = "gear/unknown".into();
    let request = PatchbayInteractionRequest::select(
        interaction.next_request_id("select").unwrap(),
        &unknown,
    )
    .unwrap();
    let unknown_receipt = interaction
        .execute(Some(&graph), request, |_| {
            PatchbayInvocationOutcome::Succeeded
        })
        .unwrap();
    assert_eq!(
        unknown_receipt.disposition,
        InteractionDisposition::Refused(PatchbayRefusal::UnknownSubject)
    );
    assert_eq!(interaction.selected(), Some(&subject));
}

#[test]
fn lifecycle_invocation_uses_the_same_play_and_preserves_refusal() {
    let mut interaction = interaction();
    let presentation = invocation_presentation(PatchbayAction::BeBorn, "body/count-demo");
    let action_id = presentation.actions[0].identity.clone();
    let request = PatchbayInteractionRequest::invoke(
        interaction.next_request_id("be-born").unwrap(),
        &presentation,
        &action_id,
    )
    .unwrap();
    let control = request.control_request().unwrap().unwrap();
    assert_eq!(control.presentation_id, presentation.identity.as_str());
    assert_eq!(control.presentation_revision, 17);
    assert_eq!(control.action_id, action_id);
    assert_eq!(control.action, PatchbayAction::BeBorn);
    assert_eq!(control.target_identity, "body/count-demo");
    let mut invoked = None;
    let receipt = interaction
        .execute_presentation(&presentation, request, |request| {
            invoked = Some(request.clone());
            PatchbayInvocationOutcome::Succeeded
        })
        .unwrap();
    assert_eq!(receipt.disposition, InteractionDisposition::Succeeded);
    assert!(matches!(
        invoked.unwrap(),
        PatchbayInteractionRequest::Invoke { invocation, .. }
            if invocation.action == PatchbayAction::BeBorn
    ));

    let request = PatchbayInteractionRequest::invoke(
        interaction.next_request_id("be-born").unwrap(),
        &presentation,
        &action_id,
    )
    .unwrap();
    let receipt = interaction
        .execute_presentation(&presentation, request, |_| {
            PatchbayInvocationOutcome::Refused(PatchbayRefusal::OperationRejected)
        })
        .unwrap();
    assert_eq!(
        receipt.disposition,
        InteractionDisposition::Refused(PatchbayRefusal::OperationRejected)
    );

    let request = PatchbayInteractionRequest::invoke(
        interaction.next_request_id("be-born").unwrap(),
        &presentation,
        &action_id,
    )
    .unwrap();
    let receipt = interaction
        .execute_presentation(&presentation, request, |_| {
            PatchbayInvocationOutcome::Failed
        })
        .unwrap();
    assert_eq!(receipt.disposition, InteractionDisposition::Failed);
    assert!(receipt
        .signs
        .iter()
        .any(|event| event.kind == KernelEventKind::HostOperationCompleted));
}

#[test]
fn semantic_invocation_refusals_are_exact_and_do_not_reach_the_host_operation() {
    let mut interaction = interaction();
    let presentation = invocation_presentation(PatchbayAction::OpenBack, "seed/example");
    let action_id = presentation.actions[0].identity.clone();

    let unknown = PatchbayInteractionRequest::invoke(
        interaction.next_request_id("open").unwrap(),
        &presentation,
        "action/unknown",
    );
    assert!(matches!(
        unknown,
        Err(crate::InteractionError::Action(
            conduit_presentation::PresentationActionRefusal::UnknownAction
        ))
    ));

    let mut unavailable_presentation = presentation.clone();
    unavailable_presentation.actions[0].availability =
        conduit_presentation::PresentationActionAvailability::Unavailable {
            reason_code: "front-door/not-current".into(),
            explanation: "The action is not current.".into(),
        };
    let unavailable = PatchbayInteractionRequest::invoke(
        interaction.next_request_id("open").unwrap(),
        &unavailable_presentation,
        &action_id,
    )
    .unwrap();
    let mut invoked = false;
    let receipt = interaction
        .execute_presentation(&unavailable_presentation, unavailable, |_| {
            invoked = true;
            PatchbayInvocationOutcome::Succeeded
        })
        .unwrap();
    assert_eq!(
        receipt.disposition,
        InteractionDisposition::Refused(PatchbayRefusal::ActionUnavailable)
    );
    assert!(!invoked);

    for mutation in [
        "presentation-id",
        "presentation-revision",
        "action-id",
        "target",
    ] {
        let mut request = PatchbayInteractionRequest::invoke(
            interaction.next_request_id("open").unwrap(),
            &presentation,
            &action_id,
        )
        .unwrap();
        let PatchbayInteractionRequest::Invoke { invocation, .. } = &mut request else {
            unreachable!()
        };
        let expected = match mutation {
            "presentation-id" => {
                invocation.presentation_id = "presentation/stale".into();
                PatchbayRefusal::StalePresentation
            }
            "presentation-revision" => {
                invocation.presentation_revision += 1;
                PatchbayRefusal::StalePresentation
            }
            "action-id" => {
                invocation.action_id = "action/unknown".into();
                PatchbayRefusal::UnknownAction
            }
            "target" => {
                invocation.target_identity = "seed/other".into();
                PatchbayRefusal::WrongTarget
            }
            _ => unreachable!(),
        };
        let mut invoked = false;
        let receipt = interaction
            .execute_presentation(&presentation, request, |_| {
                invoked = true;
                PatchbayInvocationOutcome::Succeeded
            })
            .unwrap();
        assert_eq!(
            receipt.disposition,
            InteractionDisposition::Refused(expected),
            "mutation {mutation}"
        );
        assert!(!invoked, "mutation {mutation} reached the host operation");
    }
}

#[test]
fn duplicate_semantic_delivery_is_refused_before_a_second_host_operation() {
    let mut interaction = interaction();
    let presentation = invocation_presentation(PatchbayAction::OpenBack, "seed/example");
    let request = PatchbayInteractionRequest::invoke(
        interaction.next_request_id("open").unwrap(),
        &presentation,
        &presentation.actions[0].identity,
    )
    .unwrap();
    let duplicate = request.clone();
    let mut calls = 0;
    interaction
        .execute_presentation(&presentation, request, |_| {
            calls += 1;
            PatchbayInvocationOutcome::Succeeded
        })
        .unwrap();
    let receipt = interaction
        .execute_presentation(&presentation, duplicate, |_| {
            calls += 1;
            PatchbayInvocationOutcome::Succeeded
        })
        .unwrap();
    assert_eq!(calls, 1);
    assert_eq!(
        receipt.disposition,
        InteractionDisposition::Refused(PatchbayRefusal::DuplicateDelivery)
    );
}

#[test]
fn interaction_values_are_platform_neutral_and_bounded() {
    let presentation = invocation_presentation(PatchbayAction::ToggleLinearView, "form/count-demo");
    let request = PatchbayInteractionRequest::invoke(
        crate::PatchbayInteractionRequestId::new("request/1").unwrap(),
        &presentation,
        &presentation.actions[0].identity,
    )
    .unwrap();
    let debug = format!("{request:?}").to_ascii_lowercase();
    for forbidden in ["wayland", "dom", "pixel", "widget", "socket", "address"] {
        assert!(!debug.contains(forbidden));
    }
    assert!(PatchbayInteractionRequest::invoke(
        crate::PatchbayInteractionRequestId::new("request/2").unwrap(),
        &presentation,
        "x".repeat(crate::interaction::MAX_INTERACTION_ID_BYTES + 1)
            .as_str(),
    )
    .is_err());
}

#[test]
fn typed_edit_round_trips_through_form_plan_kernel_and_binary_value_without_packing() {
    let graph = count_graph();
    let basis = PatchbayEditBasis::new(
        graph.source_document_id.clone(),
        7,
        graph.expanded_form_id.clone(),
    )
    .unwrap();
    let edit = PatchbayEdit::ConfigureGear {
        basis,
        subject_identity: "gear/count-demo/counter".into(),
        key: "label".into(),
        value: ConfigurationValue::Text("literal@delimiter:is-data".into()),
    };
    let mut interaction = interaction();
    let request = PatchbayInteractionRequest::edit(
        interaction.next_request_id("configure-gear").unwrap(),
        edit.clone(),
    )
    .unwrap();
    let receipt = interaction
        .execute(Some(&graph), request.clone(), |planned| {
            assert_eq!(planned, &request);
            PatchbayInvocationOutcome::Succeeded
        })
        .unwrap();

    assert_eq!(receipt.disposition, InteractionDisposition::Succeeded);
    assert_eq!(receipt.request, request);
    assert!(matches!(
        receipt.request,
        PatchbayInteractionRequest::Edit { edit: decoded, .. } if decoded == edit
    ));
    assert!(interaction
        .lines()
        .join("\n")
        .contains("kind=interaction/edit"));
}
