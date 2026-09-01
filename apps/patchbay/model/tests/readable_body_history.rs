use conduit_body::{
    AuthenticatedHostObservation, Body, BodyBiographyEvidence, BodyBiographyRecordKind,
    BodyGraduationChoice, BodyGraduationEvidence, BodyMembership, MembershipProofId, PartId,
};
use conduit_core::{
    bind_sign, BootId, CheckedFormId, HostId, ImplementationId, OfferGeneration, PlanId, SignId,
    SourceDocumentId,
};
use conduit_presentation::{PresentationAspect, PresentationDepth, PresentationPlace};
use patchbay_model::{
    BodyHistoryManifestation, BodyHistoryMoment, PatchbayBodyApplicationEntrance,
    PatchbayBodyAttachment, PatchbayBodyEntranceError, ReadableBodyHistory,
    ReadableBodyHistoryError, ReadableBodyHistorySlot, MAX_BODY_BIOGRAPHY_EXPLANATION_BYTES,
    MAX_BODY_HISTORY_LINEAR_BYTES, MAX_BODY_HISTORY_TITLE_BYTES, MAX_PATCHBAY_BODY_EVIDENCE_BYTES,
};

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

fn hosted_attachment() -> PatchbayBodyAttachment {
    PatchbayBodyAttachment::open_serialized(
        &encoded(BodyGraduationChoice::HostedPatchbay),
        PatchbayBodyApplicationEntrance::Hosted {
            plan_id: PlanId::from(HOSTED_PLAN),
            implementation_id: ImplementationId::from(HOSTED_IMPLEMENTATION),
        },
    )
    .unwrap()
}

#[test]
fn hosted_history_is_body_signs_with_four_ordered_friendly_and_exact_entries() {
    let history = ReadableBodyHistory::from_attachment(7, &hosted_attachment()).unwrap();

    assert_eq!(history.place, PresentationPlace::Body);
    assert_eq!(history.aspect, PresentationAspect::Signs);
    assert_eq!(
        history.access.exact_evidence_depth,
        PresentationDepth::Exact
    );
    assert_eq!(
        history.access.alternate_manifestation,
        BodyHistoryManifestation::Linear
    );
    assert_eq!(history.entries.len(), 4);
    assert_eq!(
        history
            .entries
            .iter()
            .map(|entry| entry.exact.record.sequence)
            .collect::<Vec<_>>(),
        vec![1, 2, 3, 4]
    );
    assert_eq!(history.entries[0].title, "Born");
    assert_eq!(history.entries[1].title, "Part admitted");
    assert_eq!(history.entries[2].title, "Host joined");
    assert_eq!(history.entries[3].title, "Graduated from the Crèche");
    assert!(history.entries[3].narrative.contains(HOSTED_PLAN));
    assert_eq!(history.entries[3].inspect.place, PresentationPlace::Body);
    assert_eq!(history.entries[3].inspect.aspect, PresentationAspect::Signs);
    assert_eq!(history.entries[3].inspect.depth, PresentationDepth::Exact);
    assert_eq!(
        history.entries[3].inspect.subject_identity,
        format!("sign/{}", history.entries[3].exact.record.sign_id.as_str())
    );
    assert!(matches!(
        history.entries[2].exact.record.kind,
        BodyBiographyRecordKind::HostJoined { .. }
    ));
    assert!(history.entries.iter().all(|entry| {
        entry.linear.starts_with("BODY_BIOGRAPHY body=")
            && entry.linear.contains(entry.exact.record.sign_id.as_str())
            && entry.title.len() <= MAX_BODY_HISTORY_TITLE_BYTES
            && entry.narrative.len() <= MAX_BODY_BIOGRAPHY_EXPLANATION_BYTES
            && entry.linear.len() <= MAX_BODY_HISTORY_LINEAR_BYTES
    }));
}

#[test]
fn sequence_is_the_only_time_claim_in_the_serialized_contract() {
    let history = ReadableBodyHistory::from_attachment(8, &hosted_attachment()).unwrap();
    assert!(history.entries.iter().enumerate().all(
        |(index, entry)| entry.moment == BodyHistoryMoment::EvidenceSequence(index as u64 + 1)
    ));

    let serialized = serde_json::to_value(&history).unwrap();
    for entry in serialized["entries"].as_array().unwrap() {
        assert!(entry.get("timestamp").is_none());
        assert!(entry.get("clock_time").is_none());
        assert!(entry.get("relative_time").is_none());
        assert!(entry["moment"].get("EvidenceSequence").is_some());
    }
}

#[test]
fn external_graduation_is_exactly_unhosted_and_inspects_its_sign() {
    let attachment = PatchbayBodyAttachment::open_serialized(
        &encoded(BodyGraduationChoice::ExternalReader),
        PatchbayBodyApplicationEntrance::ExternalReader,
    )
    .unwrap();
    let history = ReadableBodyHistory::from_attachment(2, &attachment).unwrap();
    let graduation = history.entries.last().unwrap();
    let hosted = ReadableBodyHistory::from_attachment(2, &hosted_attachment()).unwrap();

    assert_eq!(history.entries[..3], hosted.entries[..3]);
    assert!(graduation.narrative.contains("No Patchbay was hosted"));
    assert!(matches!(
        graduation.exact.record.kind,
        BodyBiographyRecordKind::Graduated {
            choice: BodyGraduationChoice::ExternalReader,
            patchbay_plan_id: None,
            patchbay_implementation_id: None,
        }
    ));
    assert_eq!(graduation.inspect.sign_id, graduation.exact.record.sign_id);
    assert!(graduation.linear.contains("ExternalReader"));
}

#[test]
fn every_rejected_replacement_clears_the_prior_friendly_biography() {
    let mut slot = ReadableBodyHistorySlot::default();
    slot.replace_attachment(1, opened(&encoded(BodyGraduationChoice::ExternalReader)))
        .unwrap();
    assert_eq!(slot.current().unwrap().friendly_name, "Roseau");

    let cases = [
        b"{bad".to_vec(),
        truncated(),
        reordered(),
        duplicate(),
        mismatched_body(),
        unknown_record_kind(),
        vec![b'x'; MAX_PATCHBAY_BODY_EVIDENCE_BYTES + 1],
    ];
    for (offset, invalid) in cases.into_iter().enumerate() {
        assert!(slot
            .replace_attachment(offset as u64 + 2, opened(&invalid),)
            .is_err());
        assert!(slot.current().is_none());
    }

    assert_eq!(
        slot.replace_attachment(8, opened(&encoded(BodyGraduationChoice::ExternalReader)),),
        Err(ReadableBodyHistoryError::StaleRevision {
            current: 8,
            offered: 8,
        })
    );
    assert!(slot.current().is_none());
}

fn truncated() -> Vec<u8> {
    let mut evidence = evidence(BodyGraduationChoice::ExternalReader);
    evidence.records.pop();
    serde_json::to_vec(&evidence).unwrap()
}

fn reordered() -> Vec<u8> {
    let mut evidence = evidence(BodyGraduationChoice::ExternalReader);
    evidence.records.swap(1, 2);
    serde_json::to_vec(&evidence).unwrap()
}

fn duplicate() -> Vec<u8> {
    let mut evidence = evidence(BodyGraduationChoice::ExternalReader);
    evidence.records.insert(2, evidence.records[1].clone());
    serde_json::to_vec(&evidence).unwrap()
}

fn mismatched_body() -> Vec<u8> {
    let mut evidence = evidence(BodyGraduationChoice::ExternalReader);
    evidence.body_id = Body::born(
        SourceDocumentId::from("source/other"),
        CheckedFormId::from("checked/other"),
        1,
        SignId::from("sign/other"),
    )
    .unwrap()
    .body_id;
    serde_json::to_vec(&evidence).unwrap()
}

fn unknown_record_kind() -> Vec<u8> {
    let mut value = serde_json::to_value(evidence(BodyGraduationChoice::ExternalReader)).unwrap();
    value["records"][1]["kind"] = serde_json::json!({"FutureEvent": {"claim": "friendly"}});
    serde_json::to_vec(&value).unwrap()
}

#[test]
fn entrance_errors_remain_machine_readable_through_the_history_slot() {
    let mut slot = ReadableBodyHistorySlot::default();
    assert_eq!(
        slot.replace_attachment(1, opened(b"{bad")),
        Err(ReadableBodyHistoryError::Entrance(
            PatchbayBodyEntranceError::MalformedEvidence
        ))
    );
    assert_eq!(
        slot.replace_attachment(0, opened(&encoded(BodyGraduationChoice::ExternalReader)),),
        Err(ReadableBodyHistoryError::InvalidRevision)
    );
    assert!(slot.current().is_none());
}

fn opened(encoded: &[u8]) -> Result<PatchbayBodyAttachment, PatchbayBodyEntranceError> {
    PatchbayBodyAttachment::open_serialized(
        encoded,
        PatchbayBodyApplicationEntrance::ExternalReader,
    )
}
