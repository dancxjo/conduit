use super::*;
use conduit_body::{
    AuthenticatedHostObservation, Body, BodyBiographyEvidence, BodyGraduationEvidence,
    BodyMembership, MembershipProofId,
};
use conduit_core::{bind_sign, OfferGeneration};

const HOSTED_PLAN: &str = "plan/roseau-patchbay";
const HOSTED_IMPLEMENTATION: &str = "browser/patchbay-surface@1";

fn evidence(choice: BodyGraduationChoice) -> BodyBiographyEvidence {
    let host_id = HostId::from("host/roseau-browser");
    let boot_id = BootId::from("boot/roseau-browser");
    let body = Body::born(
        SourceDocumentId::from("source/roseau-morse-network"),
        CheckedFormId::from("checked/roseau-morse-network"),
        1,
        bind_sign(&host_id, &boot_id, None, 1).sign_id,
    )
    .unwrap();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let part_id = PartId::bind(&body.body_id, "part/roseau-browser", 1).unwrap();
    let proof = MembershipProofId::bind("proof/roseau-browser").unwrap();
    let admitted = membership
        .admit(
            &body.body_id,
            membership.revision,
            part_id.clone(),
            proof.clone(),
            bind_sign(&host_id, &boot_id, None, 2).sign_id,
        )
        .unwrap();
    let joined = membership
        .observe_present(
            &body.body_id,
            membership.revision,
            &part_id,
            AuthenticatedHostObservation {
                host_id: host_id.clone(),
                boot_id: boot_id.clone(),
                offer_generation: OfferGeneration(3),
                proof_id: proof,
                sequence: 8,
            },
            bind_sign(&host_id, &boot_id, None, 3).sign_id,
        )
        .unwrap();
    let mut evidence = BodyBiographyEvidence::born(
        body,
        BodyMembership::new(membership.body_id.clone()).unwrap(),
        "Roseau".into(),
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
            sign_id: SignId::from("sign/roseau-graduated"),
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

#[test]
fn hosted_roseau_opens_as_one_lulled_current_body_with_exact_facts() {
    let attachment = PatchbayBodyAttachment::open_serialized(
        &encoded(BodyGraduationChoice::HostedPatchbay),
        PatchbayBodyApplicationEntrance::Hosted {
            plan_id: PlanId::from(HOSTED_PLAN),
            implementation_id: ImplementationId::from(HOSTED_IMPLEMENTATION),
        },
    )
    .unwrap();
    let frame = CurrentBodyFrame::from_attachment(7, &attachment);

    assert_eq!(frame.friendly_name, "Roseau");
    assert_eq!(frame.workload_revision, 0);
    assert_eq!(frame.active_forms.len(), 1);
    assert_eq!(frame.lifecycle, CurrentBodyLifecycle::Lulled);
    assert_eq!(frame.salient_action, CurrentBodyLifecycleAction::Wake);
    assert_eq!(frame.admitted_parts, 1);
    assert_eq!(frame.current_hosts.len(), 1);
    assert_eq!(
        frame.physical_hosts,
        CurrentBodyPhysicalHostSummary::NotEvidenced
    );
    assert!(frame.status_line.contains("workload revision 0"));
    assert!(frame
        .status_line
        .contains("physical Host classification not evidenced"));
    assert_eq!(frame.latest_evidence.sequence, 4);
    assert!(matches!(
        frame.patchbay_reader,
        CurrentBodyPatchbayReader::HostedByBody { .. }
    ));
}

#[test]
fn external_readers_distinguish_hosted_and_unhosted_graduations() {
    let hosted = PatchbayBodyAttachment::open_serialized(
        &encoded(BodyGraduationChoice::HostedPatchbay),
        PatchbayBodyApplicationEntrance::ExternalReader,
    )
    .unwrap();
    let unhosted = PatchbayBodyAttachment::open_serialized(
        &encoded(BodyGraduationChoice::ExternalReader),
        PatchbayBodyApplicationEntrance::ExternalReader,
    )
    .unwrap();

    assert!(matches!(
        CurrentBodyFrame::from_attachment(1, &hosted).patchbay_reader,
        CurrentBodyPatchbayReader::ExternalReadingHostedBody { .. }
    ));
    assert_eq!(
        CurrentBodyFrame::from_attachment(1, &unhosted).patchbay_reader,
        CurrentBodyPatchbayReader::ExternalReadingUnhostedBody
    );
}

#[test]
fn stale_and_malformed_replacements_clear_prior_friendly_content() {
    let mut slot = CurrentBodyFrameSlot::default();
    slot.replace_serialized(
        2,
        &encoded(BodyGraduationChoice::ExternalReader),
        PatchbayBodyApplicationEntrance::ExternalReader,
    )
    .unwrap();
    assert_eq!(slot.current().unwrap().friendly_name, "Roseau");

    assert_eq!(
        slot.replace_serialized(
            1,
            &encoded(BodyGraduationChoice::ExternalReader),
            PatchbayBodyApplicationEntrance::ExternalReader,
        ),
        Err(CurrentBodyFrameError::StaleRevision {
            current: 2,
            offered: 1,
        })
    );
    assert!(slot.current().is_none());

    assert_eq!(
        slot.replace_serialized(3, b"{bad", PatchbayBodyApplicationEntrance::ExternalReader,),
        Err(CurrentBodyFrameError::Entrance(
            PatchbayBodyEntranceError::MalformedEvidence
        ))
    );
    assert!(slot.current().is_none());
}

#[test]
fn an_awake_body_offers_lull_without_inventing_a_physical_host() {
    let mut evidence = evidence(BodyGraduationChoice::ExternalReader);
    let (awake, wake) = evidence
        .body
        .wake(5, SignId::from("sign/roseau-woke"))
        .unwrap();
    let sequence = evidence.records.last().unwrap().sequence + 1;
    evidence.append_wake(awake, wake, sequence).unwrap();
    let attachment = PatchbayBodyAttachment::open_serialized(
        &serde_json::to_vec(&evidence).unwrap(),
        PatchbayBodyApplicationEntrance::ExternalReader,
    )
    .unwrap();
    let frame = CurrentBodyFrame::from_attachment(9, &attachment);

    assert!(matches!(
        frame.lifecycle,
        CurrentBodyLifecycle::Awake { .. }
    ));
    assert_eq!(frame.salient_action, CurrentBodyLifecycleAction::Lull);
    assert_eq!(
        frame.physical_hosts,
        CurrentBodyPhysicalHostSummary::NotEvidenced
    );
}
