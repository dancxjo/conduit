//! Local kernel/model/Show fixtures; no page Host acknowledgement or Body install.
use super::*;
use conduit_body::BodyId;
use conduit_core::{ActivePlayId, CheckedPlotId, ExpandedPlotId, PlanId, SourceDocumentId};
use conduit_face::*;
use patchbay_application::*;

#[path = "../../../../../semantics/face/tests/common/mod.rs"]
mod mask_fixture;

fn owner() -> (PatchbayApplicationPort, BodyId, FaceContributionBasis) {
    let (startup, profile) = crate::installed_browser::catalogs().unwrap();
    let checked = conduit_plot::check_syntax_document(&conduit_plot::parse_syntax_document(
        "plot inspected {\n value: text/literal(\"Source wording\")\n show: face/text(\"Message\")\n value >> show\n}\n"
    ), &startup).unwrap();
    let expanded = conduit_plot::expand_canonical_plot(&checked, "inspected", &profile).unwrap();
    let port = PatchbayApplicationPort::open(
        &expanded,
        "plan/inspected".into(),
        "body-plan/current".into(),
    )
    .unwrap();
    let body = serde_json::from_str("\"body/current\"").unwrap();
    let basis = FaceContributionBasis {
        checked_plot_id: CheckedPlotId::from("checked/owner"),
        plan_id: PlanId::from("plan/owner"),
        active_play_id: ActivePlayId::from("play/owner"),
        required_interaction_context: None,
    };
    (port, body, basis)
}
fn face(port: &PatchbayApplicationPort, body: &BodyId, basis: &FaceContributionBasis) -> Face {
    let mut part = port.face_fragment(basis.clone(), body, true).unwrap();
    part.subjects.extend([
        FaceSubject {
            identity: format!("body/{}", body.as_str()),
            role: FaceRole::Body,
            name: "Owner".into(),
        },
        FaceSubject {
            identity: format!("plot/{}", port.graph().checked_plot_id.as_str()),
            role: FaceRole::Plot,
            name: "Inspected".into(),
        },
    ]);
    Face::new_with_semantics(
        u64::from(port.revision()),
        FaceBasis {
            body_id: Some(body.clone()),
            wake_id: None,
            source_document_id: Some(SourceDocumentId::from("source/owner")),
            checked_plot_id: Some(basis.checked_plot_id.clone()),
            expanded_plot_id: Some(ExpandedPlotId::from("expanded/owner")),
            plan_id: Some(basis.plan_id.clone()),
            active_play_id: Some(basis.active_play_id.clone()),
            sign_ids: vec![],
        },
        part.subjects,
        part.relationships,
        part.properties,
        part.text,
        part.actions,
        part.disclosures,
    )
    .unwrap()
}
fn prepared_transport() -> PreparedPatchbayIngress {
    let mut host =
        crate::installed_browser::advertisement("ingress-host".into(), "ingress-boot".into());
    host.capabilities.push(offer());
    PreparedPatchbayIngress::prepare(PlannedPatchbayIngress::prepare(&host).unwrap()).unwrap()
}
fn transport(bytes: &[u8]) -> PreparedPatchbayIngress {
    let mut transport = prepared_transport();
    transport.admit(bytes).unwrap();
    transport
}

fn edit(face: &Face, show: &MaskShow, subject: &str) -> Vec<u8> {
    let action = face
        .actions
        .iter()
        .find(|a| a.identity == EDIT_CURRENT_ACTION_ID)
        .unwrap();
    FaceInteraction::new(
        face,
        show,
        &action.identity,
        &action.target,
        vec![FaceInteractionArgument {
            name: "subject".into(),
            value_kind: UTF8_TEXT_VALUE_KIND.into(),
            value: subject.as_bytes().to_vec(),
        }],
        1,
    )
    .unwrap()
    .encode()
}

#[test]
fn actual_kernel_call_preserves_typed_edit_request_and_refuses_replay() {
    let (mut owner, body, basis) = owner();
    let initial_revision = owner.revision();
    let current = face(&owner, &body, &basis);
    let show = mask_fixture::available_mask_show(&current);
    let subject = owner
        .graph()
        .subject_identities()
        .next()
        .unwrap()
        .to_owned();
    let input = edit(&current, &show, &subject);
    let mut ingress = transport(&input);
    let receipt =
        dispatch_to_owner(&mut ingress, &mut owner, &body, &basis, &current, &show).unwrap();
    assert_eq!(receipt.ingress_plan_id, ingress.planned().plan().plan_id);
    assert_eq!(
        receipt.owner_result,
        Ok(Some(PatchbayApplicationRequest::EditCurrent {
            expanded_plot_id: owner.graph().expanded_plot_id.clone(),
            subject_identity: subject,
        }))
    );
    receipt.transport_completion.unwrap();
    assert!(ingress.is_terminal());
    assert_eq!(owner.revision(), initial_revision + 1);
    assert!(dispatch_to_owner(&mut ingress, &mut owner, &body, &basis, &current, &show).is_err());
    let before = owner.observe_fragment(basis.clone(), &body).unwrap();
    let receipt = dispatch_to_owner(
        &mut transport(&input),
        &mut owner,
        &body,
        &basis,
        &current,
        &show,
    )
    .unwrap();
    assert_eq!(
        receipt.owner_result,
        Err(PatchbayCommandRefusal::StaleState)
    );
    assert!(receipt.transport_completion.is_err());
    assert_eq!(owner.observe_fragment(basis, &body).unwrap(), before);
}

#[test]
fn emitted_malformed_and_replaced_show_requests_refuse_without_owner_changes() {
    let (mut owner, body, basis) = owner();
    let current = face(&owner, &body, &basis);
    let show = mask_fixture::available_mask_show(&current);
    let before = owner.observe_fragment(basis.clone(), &body).unwrap();
    let receipt = dispatch_to_owner(
        &mut transport(b"{}"),
        &mut owner,
        &body,
        &basis,
        &current,
        &show,
    )
    .unwrap();
    assert_eq!(
        receipt.owner_result,
        Err(PatchbayCommandRefusal::InvalidInteraction(
            FaceInteractionRefusal::MalformedEncoding
        ))
    );
    assert!(receipt.transport_completion.is_err());
    let subject = owner.graph().subject_identities().next().unwrap();
    let input = edit(&current, &show, subject);
    let replaced = show
        .transition(ManifestationLifecycle::Replaced, "sign/replaced".into())
        .unwrap();
    let receipt = dispatch_to_owner(
        &mut transport(&input),
        &mut owner,
        &body,
        &basis,
        &current,
        &replaced,
    )
    .unwrap();
    assert!(matches!(
        receipt.owner_result,
        Err(PatchbayCommandRefusal::InvalidInteraction(_))
    ));
    assert!(receipt.transport_completion.is_err());
    assert_eq!(owner.observe_fragment(basis, &body).unwrap(), before);
}

#[test]
fn revoked_show_transport_never_dispatches_pending_bytes_to_owner() {
    let (mut owner, body, basis) = owner();
    let current = face(&owner, &body, &basis);
    let show = mask_fixture::available_mask_show(&current);
    let subject = owner.graph().subject_identities().next().unwrap();
    let input = edit(&current, &show, subject);
    let before = owner.observe_fragment(basis.clone(), &body).unwrap();
    let mut pending = transport(&input);
    let capacity = pending.value_capacities();
    pending.retire().unwrap();
    pending.retire().unwrap();
    assert!(pending.is_terminal());
    assert!(pending.pending_input().is_err());
    assert!(pending.admit(&input).is_err());
    assert!(pending
        .complete(conduit_kernel::HostCallOutcome {
            disposition: conduit_kernel::HostCallDisposition::Completed,
            output: None,
            failure: None,
        })
        .is_err());
    assert!(dispatch_to_owner(&mut pending, &mut owner, &body, &basis, &current, &show).is_err());
    assert_eq!(pending.value_capacities(), capacity);
    let mut host =
        crate::installed_browser::advertisement("ingress-host".into(), "ingress-boot".into());
    host.capabilities.push(offer());
    let mut ready =
        PreparedPatchbayIngress::prepare(PlannedPatchbayIngress::prepare(&host).unwrap()).unwrap();
    ready.retire().unwrap();
    assert!(ready.is_terminal());
    assert!(ready.admit(&input).is_err());
    assert_eq!(owner.observe_fragment(basis, &body).unwrap(), before);
}

#[test]
fn owner_turn_retains_request_through_retirement_and_transfer_without_reopening() {
    let (mut owner, body, basis) = owner();
    let current = face(&owner, &body, &basis);
    let show = mask_fixture::available_mask_show(&current);
    let subject = owner
        .graph()
        .subject_identities()
        .next()
        .unwrap()
        .to_owned();
    let input = edit(&current, &show, &subject);
    let mut turn =
        PatchbayOwnerTurn::bind(body, basis, current, show, prepared_transport()).unwrap();
    assert!(turn.can_submit());
    let initial_revision = owner.revision();
    turn.submit(&mut owner, &input).unwrap();
    assert_eq!(owner.revision(), initial_revision + 1);
    assert!(!turn.can_submit());
    turn.retire().unwrap();
    assert!(turn.receipt().unwrap().transport_completion.is_ok());
    assert_eq!(
        turn.receipt().unwrap().owner_result,
        Ok(Some(PatchbayApplicationRequest::EditCurrent {
            expanded_plot_id: owner.graph().expanded_plot_id.clone(),
            subject_identity: subject,
        }))
    );
    let retained = turn.take_receipt().unwrap();
    assert!(retained.owner_result.is_ok());
    assert!(turn.take_receipt().is_none());
    assert!(turn.submit(&mut owner, &input).is_err());
    assert_eq!(owner.revision(), initial_revision + 1);
}

#[test]
fn owner_turn_refuses_wrong_scope_or_unavailable_show_and_revokes_before_dispatch() {
    let (mut owner, body, basis) = owner();
    let current = face(&owner, &body, &basis);
    let show = mask_fixture::available_mask_show(&current);
    let mut foreign_basis = basis.clone();
    foreign_basis.active_play_id = "play/foreign".into();
    assert!(PatchbayOwnerTurn::bind(
        body.clone(),
        foreign_basis,
        current.clone(),
        show.clone(),
        prepared_transport()
    )
    .is_err());
    let replaced = show
        .transition(
            ManifestationLifecycle::Replaced,
            "sign/replaced-turn".into(),
        )
        .unwrap();
    assert!(PatchbayOwnerTurn::bind(
        body.clone(),
        basis.clone(),
        current.clone(),
        replaced,
        prepared_transport()
    )
    .is_err());
    let subject = owner.graph().subject_identities().next().unwrap();
    let input = edit(&current, &show, subject);
    let before = owner.observe_fragment(basis.clone(), &body).unwrap();
    let mut turn = PatchbayOwnerTurn::bind(
        body.clone(),
        basis.clone(),
        current,
        show,
        prepared_transport(),
    )
    .unwrap();
    turn.retire().unwrap();
    assert!(!turn.can_submit());
    assert!(turn.submit(&mut owner, &input).is_err());
    assert!(turn.receipt().is_none());
    assert_eq!(owner.observe_fragment(basis, &body).unwrap(), before);
}
