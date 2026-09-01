use crate::{
    arguments::NativeBodyEntrance,
    native_body_workbench::{
        NativeBodyWorkbench, NativeBodyWorkbenchError, NativeBodyWorkbenchSlot,
        NativeWorkbenchDestination,
    },
};
use conduit_body::{
    AuthenticatedHostObservation, Body, BodyBiographyEvidence, BodyGraduationChoice,
    BodyGraduationEvidence, BodyMembership, MembershipProofId, PartId,
};
use conduit_core::{bind_sign, BootId, HostId, ImplementationId, OfferGeneration, PlanId, SignId};

const PLAN: &str = "plan/native-fixture-patchbay";
const IMPLEMENTATION: &str = "native/patchbay@1";

fn fixture(choice: BodyGraduationChoice) -> (patchbay_model::PatchbayGraph, Vec<u8>) {
    let source = r#"form signal {
    upper: text/upper
    show: presentation/text

    "SOS" > upper > show
}"#;
    let editor =
        patchbay_model::FormEditor::from_source("signal.conduit".into(), source.into()).unwrap();
    let open_form = editor.view().open_form.clone();
    let graph = editor.patchbay_graph_for_authoring(&open_form).unwrap();
    let host = HostId::from("host/native-fixture");
    let boot = BootId::from("boot/native-fixture/1");
    let body = Body::born(
        graph.source_document_id.clone(),
        graph.checked_form_id.clone(),
        1,
        bind_sign(&host, &boot, None, 1).sign_id,
    )
    .unwrap();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let part = PartId::bind(&body.body_id, "roseau/here", 1).unwrap();
    let proof = MembershipProofId::bind("proof/roseau/here").unwrap();
    let admitted = membership
        .admit(
            &body.body_id,
            membership.revision,
            part.clone(),
            proof.clone(),
            bind_sign(&host, &boot, None, 2).sign_id,
        )
        .unwrap();
    let joined = membership
        .observe_present(
            &body.body_id,
            membership.revision,
            &part,
            AuthenticatedHostObservation {
                host_id: host.clone(),
                boot_id: boot.clone(),
                offer_generation: OfferGeneration(1),
                proof_id: proof,
                sequence: 1,
            },
            bind_sign(&host, &boot, None, 3).sign_id,
        )
        .unwrap();
    let mut evidence = BodyBiographyEvidence::born(
        body,
        BodyMembership::new(membership.body_id.clone()).unwrap(),
        "Roseau".into(),
        "Morse relay".into(),
    )
    .unwrap();
    evidence
        .append_membership_events(membership, &[(admitted, 2), (joined, 3)])
        .unwrap();
    let hosted = choice == BodyGraduationChoice::HostedPatchbay;
    evidence
        .graduate(BodyGraduationEvidence {
            body_id: evidence.body_id.clone(),
            sequence: 4,
            sign_id: SignId::from("sign/native-fixture/graduated"),
            choice,
            patchbay_plan_id: hosted.then(|| PlanId::from(PLAN)),
            patchbay_implementation_id: hosted.then(|| ImplementationId::from(IMPLEMENTATION)),
        })
        .unwrap();
    (graph, serde_json::to_vec(&evidence).unwrap())
}

#[test]
fn hosted_and_external_destinations_reuse_program_body_and_body_signs_semantics() {
    let (graph, evidence) = fixture(BodyGraduationChoice::HostedPatchbay);
    let hosted = NativeBodyWorkbench::open(
        1,
        evidence.clone(),
        NativeBodyEntrance::Hosted {
            plan_id: PLAN.into(),
            implementation_id: IMPLEMENTATION.into(),
        },
        &graph,
    )
    .unwrap();
    let mut external = NativeBodyWorkbench::open(
        1,
        evidence.clone(),
        NativeBodyEntrance::ExternalReader,
        &graph,
    )
    .unwrap();
    assert_eq!(hosted.current().body_id, external.current().body_id);
    assert_eq!(hosted.history().entries, external.history().entries);
    assert_eq!(external.destination(), NativeWorkbenchDestination::Body);
    assert_eq!(external.current().friendly_name, "Roseau");
    assert_eq!(external.encoded_evidence(), evidence);
    let body = external.lines(false, false).join("\n");
    assert!(body.contains("SALIENT ACTION Wake"));
    assert!(body.contains("host=host/native-fixture boot=boot/native-fixture/1"));
    assert!(body.contains("LINES not evidenced"));
    external.select(NativeWorkbenchDestination::History);
    assert_eq!(
        external.destination().semantic_cursor(),
        (
            conduit_presentation::PresentationPlace::Body,
            conduit_presentation::PresentationAspect::Signs
        )
    );
    let linear_exact = external.lines(true, true).join("\n");
    assert!(linear_exact.contains("BODY_BIOGRAPHY"));
    assert!(linear_exact.contains(external.current().body_id.as_str()));
    external.select(NativeWorkbenchDestination::Program);
    assert!(external
        .lines(false, false)
        .join("\n")
        .contains("existing native Gear / Port / Cord canvas"));
    assert_eq!(external.detach(), evidence);
}

#[test]
fn pointer_keyboard_and_failed_lifecycle_actions_share_state_without_mutating_evidence() {
    let (graph, evidence) = fixture(BodyGraduationChoice::ExternalReader);
    let mut application = crate::PatchbayApplication::new(crate::Arguments::default()).unwrap();
    application
        .body_workbench
        .replace(
            1,
            evidence.clone(),
            NativeBodyEntrance::ExternalReader,
            &graph,
        )
        .unwrap();
    application
        .handle_gui_action(crate::gui::GuiAction::SelectBodyWorkbench(
            NativeWorkbenchDestination::History,
        ))
        .unwrap();
    assert_eq!(
        application.body_workbench.current().unwrap().destination(),
        NativeWorkbenchDestination::History
    );
    application.modifiers = winit::keyboard::ModifiersState::CONTROL;
    assert!(application
        .handle_body_workbench_key(&winit::keyboard::Key::Named(winit::keyboard::NamedKey::Tab)));
    assert_eq!(
        application.body_workbench.current().unwrap().destination(),
        NativeWorkbenchDestination::Program
    );
    application
        .handle_gui_action(crate::gui::GuiAction::Lifecycle(
            patchbay_model::PatchbayAction::Wake,
        ))
        .unwrap();
    assert!(application.build_birth.body().is_none());
    assert_eq!(
        application
            .body_workbench
            .current()
            .unwrap()
            .encoded_evidence(),
        evidence
    );
    assert_eq!(
        application.interaction_status.current().unwrap().code,
        crate::interaction_status::InteractionStatusCode::Refused
    );
}

#[test]
fn stale_invalid_and_mismatched_replacements_clear_prior_friendly_content() {
    let (graph, evidence) = fixture(BodyGraduationChoice::ExternalReader);
    let mut slot = NativeBodyWorkbenchSlot::default();
    slot.replace(
        1,
        evidence.clone(),
        NativeBodyEntrance::ExternalReader,
        &graph,
    )
    .unwrap();
    assert!(slot.current().is_some());
    assert!(matches!(
        slot.replace(
            1,
            evidence.clone(),
            NativeBodyEntrance::ExternalReader,
            &graph
        ),
        Err(NativeBodyWorkbenchError::StaleRevision { .. })
    ));
    assert!(slot.current().is_none());
    assert!(slot
        .replace(
            2,
            b"not-json".to_vec(),
            NativeBodyEntrance::ExternalReader,
            &graph
        )
        .is_err());
    assert!(slot.current().is_none());
    slot.replace(
        3,
        evidence.clone(),
        NativeBodyEntrance::ExternalReader,
        &graph,
    )
    .unwrap();
    assert_eq!(slot.detach(), Some(evidence));
    assert!(slot.current().is_none());
}
