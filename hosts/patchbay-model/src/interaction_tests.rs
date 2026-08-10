use crate::{
    FormEditor, InteractionDisposition, PatchbayAction, PatchbayGraph, PatchbayInteraction,
    PatchbayInteractionRequest, PatchbayRefusal,
};
use conduit_core::{BootId, ExpandedFormId, HostId};
use conduit_kernel::KernelEventKind;
use std::path::PathBuf;

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
        .execute(Some(&graph), accepted, |_| Ok(()))
        .unwrap();

    let mut stale = subject.clone();
    stale.expanded_form_id = ExpandedFormId::from("expanded/stale");
    let request =
        PatchbayInteractionRequest::select(interaction.next_request_id("select").unwrap(), &stale)
            .unwrap();
    let stale_receipt = interaction
        .execute(Some(&graph), request, |_| Ok(()))
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
        .execute(Some(&graph), request, |_| Ok(()))
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
    let request = PatchbayInteractionRequest::invoke(
        interaction.next_request_id("birth").unwrap(),
        PatchbayAction::Birth,
        "body/count-demo",
    )
    .unwrap();
    let mut invoked = None;
    let receipt = interaction
        .execute(None, request, |invocation| {
            invoked = Some(invocation.clone());
            Ok(())
        })
        .unwrap();
    assert_eq!(receipt.disposition, InteractionDisposition::Succeeded);
    assert_eq!(invoked.unwrap().action, PatchbayAction::Birth);

    let request = PatchbayInteractionRequest::invoke(
        interaction.next_request_id("birth").unwrap(),
        PatchbayAction::Birth,
        "body/count-demo",
    )
    .unwrap();
    let receipt = interaction
        .execute(None, request, |_| Err(PatchbayRefusal::OperationRejected))
        .unwrap();
    assert_eq!(
        receipt.disposition,
        InteractionDisposition::Refused(PatchbayRefusal::OperationRejected)
    );
}

#[test]
fn interaction_values_are_platform_neutral_and_bounded() {
    let request = PatchbayInteractionRequest::invoke(
        crate::PatchbayInteractionRequestId::new("request/1").unwrap(),
        PatchbayAction::ToggleLinearView,
        "form/count-demo",
    )
    .unwrap();
    let debug = format!("{request:?}").to_ascii_lowercase();
    for forbidden in ["wayland", "dom", "pixel", "widget", "socket", "address"] {
        assert!(!debug.contains(forbidden));
    }
    assert!(PatchbayInteractionRequest::invoke(
        crate::PatchbayInteractionRequestId::new("request/2").unwrap(),
        PatchbayAction::Save,
        "x".repeat(crate::interaction::MAX_INTERACTION_ID_BYTES + 1),
    )
    .is_err());
}
