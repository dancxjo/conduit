use conduit_body::{
    AuthenticatedHostObservation, Body, BodyBiographyEvidence, BodyGraduationChoice,
    BodyGraduationEvidence, BodyMembership, MembershipProofId, PartId,
};
use conduit_core::{
    bind_sign, BootId, CheckedFormId, HostId, ImplementationId, OfferGeneration, PlanId, SignId,
    SourceDocumentId,
};
use conduit_presentation::{PresentationAspect, PresentationDepth, PresentationPlace};
use patchbay_model::{PatchbayBodyApplicationEntrance, MAX_PATCHBAY_BODY_EVIDENCE_BYTES};

use crate::{
    gui::GuiAction,
    native_workbench::{NativeBodyWorkbench, NativeBodyWorkbenchSlot, NativeWorkbenchError},
    native_workbench_view::{draw_native_workbench, workbench_lines},
    BACKGROUND,
};

const HOSTED_PLAN: &str = "plan/native-roseau-patchbay";
const HOSTED_IMPLEMENTATION: &str = "patchbay-native/wayland@1";

fn evidence(choice: BodyGraduationChoice) -> BodyBiographyEvidence {
    let host = HostId::from("host/native-roseau");
    let boot = BootId::from("boot/native-roseau");
    let body = Body::born(
        SourceDocumentId::from("source/native-roseau"),
        CheckedFormId::from("checked/native-roseau"),
        1,
        bind_sign(&host, &boot, None, 1).sign_id,
    )
    .unwrap();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let part = PartId::bind(&body.body_id, "part/native-roseau", 1).unwrap();
    let proof = MembershipProofId::bind("proof/native-roseau").unwrap();
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
                offer_generation: OfferGeneration(2),
                proof_id: proof,
                sequence: 3,
            },
            bind_sign(&host, &boot, None, 3).sign_id,
        )
        .unwrap();
    let mut evidence = BodyBiographyEvidence::born(
        body,
        BodyMembership::new(membership.body_id.clone()).unwrap(),
        "Roseau".into(),
        "Morse Network".into(),
    )
    .unwrap();
    evidence
        .append_membership_events(membership, &[(admitted, 2), (joined, 3)])
        .unwrap();
    let (plan, implementation) = match choice {
        BodyGraduationChoice::HostedPatchbay => (
            Some(PlanId::from(HOSTED_PLAN)),
            Some(ImplementationId::from(HOSTED_IMPLEMENTATION)),
        ),
        BodyGraduationChoice::ExternalReader => (None, None),
    };
    evidence
        .graduate(BodyGraduationEvidence {
            body_id: evidence.body_id.clone(),
            sequence: 4,
            sign_id: SignId::from("sign/native-roseau/graduated"),
            choice,
            patchbay_plan_id: plan,
            patchbay_implementation_id: implementation,
        })
        .unwrap();
    evidence
}

fn encoded(choice: BodyGraduationChoice) -> Vec<u8> {
    serde_json::to_vec(&evidence(choice)).unwrap()
}

fn external() -> NativeBodyWorkbench {
    NativeBodyWorkbench::from_serialized(
        1,
        &encoded(BodyGraduationChoice::ExternalReader),
        PatchbayBodyApplicationEntrance::ExternalReader,
    )
    .unwrap()
}

#[test]
fn hosted_and_external_attachments_drive_the_same_body_and_history_models() {
    let hosted = NativeBodyWorkbench::from_serialized(
        7,
        &encoded(BodyGraduationChoice::HostedPatchbay),
        PatchbayBodyApplicationEntrance::Hosted {
            plan_id: PlanId::from(HOSTED_PLAN),
            implementation_id: ImplementationId::from(HOSTED_IMPLEMENTATION),
        },
    )
    .unwrap();
    let external = NativeBodyWorkbench::from_serialized(
        7,
        &encoded(BodyGraduationChoice::HostedPatchbay),
        PatchbayBodyApplicationEntrance::ExternalReader,
    )
    .unwrap();

    assert_eq!(hosted.frame().body_id, external.frame().body_id);
    assert_eq!(hosted.history().entries, external.history().entries);
    assert_eq!(hosted.place(), PresentationPlace::Body);
    assert_eq!(hosted.aspect(), PresentationAspect::Structure);
    assert_eq!(hosted.history().place, PresentationPlace::Body);
    assert_eq!(hosted.history().aspect, PresentationAspect::Signs);
}

#[test]
fn native_destinations_are_program_body_and_body_signs_with_shared_disclosure() {
    let mut workbench = external();
    assert!(workbench_lines(&workbench, false)
        .join("\n")
        .contains("Roseau"));
    workbench.cycle_destination();
    assert!(workbench.is_history());
    assert_eq!(workbench.history().entries.len(), 4);
    workbench.move_history_focus(true);
    workbench.inspect_focused_history();
    assert_eq!(workbench.depth(), PresentationDepth::Detail);
    let detail = workbench_lines(&workbench, false).join("\n");
    assert!(detail.contains("SIGN "));
    assert!(!detail.contains("BODY_BIOGRAPHY body="));
    workbench.toggle_exact();
    let exact = workbench_lines(&workbench, false).join("\n");
    assert!(exact.contains("BODY_BIOGRAPHY body="));
    assert_eq!(
        workbench_lines(&workbench, true)[0],
        "BODY / SIGNS · HISTORY · Exact"
    );
    workbench.cycle_destination();
    assert_eq!(workbench.place(), PresentationPlace::Program);
    assert_eq!(workbench.aspect(), PresentationAspect::Structure);
    assert_eq!(
        workbench_lines(&workbench, true)[0],
        "PROGRAM / STRUCTURE · Primary"
    );
}

#[test]
fn stale_malformed_and_oversized_replacements_clear_friendly_native_state() {
    let mut slot = NativeBodyWorkbenchSlot::default();
    slot.replace_serialized(
        2,
        &encoded(BodyGraduationChoice::ExternalReader),
        PatchbayBodyApplicationEntrance::ExternalReader,
    )
    .unwrap();
    assert_eq!(slot.current().unwrap().frame().friendly_name, "Roseau");
    assert!(matches!(
        slot.replace_serialized(
            1,
            &encoded(BodyGraduationChoice::ExternalReader),
            PatchbayBodyApplicationEntrance::ExternalReader,
        ),
        Err(NativeWorkbenchError::StaleRevision {
            current: 2,
            offered: 1,
        })
    ));
    assert!(slot.current().is_none());
    assert!(slot
        .replace_serialized(3, b"{bad", PatchbayBodyApplicationEntrance::ExternalReader,)
        .is_err());
    assert!(slot.current().is_none());
    assert!(slot
        .replace_serialized(
            4,
            &vec![b' '; MAX_PATCHBAY_BODY_EVIDENCE_BYTES + 1],
            PatchbayBodyApplicationEntrance::ExternalReader,
        )
        .is_err());
    assert!(slot.current().is_none());
    assert!(slot
        .replace_serialized(
            5,
            &encoded(BodyGraduationChoice::HostedPatchbay),
            PatchbayBodyApplicationEntrance::Hosted {
                plan_id: PlanId::from("plan/wrong"),
                implementation_id: ImplementationId::from(HOSTED_IMPLEMENTATION),
            },
        )
        .is_err());
    assert!(slot.current().is_none());
}

#[test]
fn failed_lifecycle_request_and_detach_do_not_mutate_body_evidence() {
    let workbench = external();
    let before = workbench.evidence().clone();
    assert_eq!(
        workbench.request_lifecycle_action(),
        Err(NativeWorkbenchError::LifecycleAuthorityUnavailable)
    );
    assert_eq!(workbench.evidence(), &before);

    let mut slot = NativeBodyWorkbenchSlot::default();
    slot.replace_serialized(
        1,
        &serde_json::to_vec(&before).unwrap(),
        PatchbayBodyApplicationEntrance::ExternalReader,
    )
    .unwrap();
    slot.detach();
    assert!(slot.current().is_none());
    assert_eq!(
        serde_json::from_slice::<BodyBiographyEvidence>(&serde_json::to_vec(&before).unwrap())
            .unwrap(),
        before
    );
}

#[test]
fn graphical_native_body_and_history_are_bounded_and_pointer_addressable() {
    let mut workbench = external();
    let mut pixels = vec![BACKGROUND; 1_100 * 720];
    let body_targets = draw_native_workbench(&mut pixels, 1_100, 720, &workbench);
    assert!(pixels.iter().any(|pixel| *pixel != BACKGROUND));
    assert_eq!(body_targets.len(), 3);
    assert!(body_targets
        .iter()
        .all(|target| matches!(target.action, GuiAction::ShowWorkbench { .. })));

    workbench
        .show(PresentationPlace::Body, PresentationAspect::Signs)
        .unwrap();
    let history_targets = draw_native_workbench(&mut pixels, 1_100, 720, &workbench);
    assert_eq!(history_targets.len(), 3 + workbench.history().entries.len());
    assert!(history_targets
        .iter()
        .any(|target| target.action == GuiAction::InspectHistoryEntry(0)));
}
