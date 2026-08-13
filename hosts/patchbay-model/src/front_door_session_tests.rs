use crate::{
    EntranceAction, EntranceUpdateDisposition, LocalFrontDoor, PartPresentationState,
    PatchbayEntranceState,
};
use conduit_body::{AdmissionSigns, CandidateObservation, DiscoveryProofId};
use conduit_browser_runtime::membership::BrowserAdmissionIdentity;
use conduit_core::{
    process_owned_line_offer_with_limits, BootId, ConnectionBase, HostAdvertisement, HostId,
    HostProfileId, LineAvailability, LineId, LinkBindingId, LinkLimits, OfferGeneration, SignId,
    PROTOCOL_VERSION,
};
use conduit_presentation::PresentationRole;

fn browser_advertisement(identity: &BrowserAdmissionIdentity) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: identity.host_id().clone(),
        boot_id: identity.boot_id().clone(),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("browser/host"),
        resources: Vec::new(),
        capabilities: Vec::new(),
        planner_capabilities: Vec::new(),
    }
}

#[test]
fn fresh_session_projects_only_current_canonical_truth() {
    let session = LocalFrontDoor::with_identity(
        HostId::from("front-door/local"),
        BootId::from("front-door/local/boot-1"),
    )
    .unwrap();
    let projection = session.project().unwrap();
    assert_eq!(projection.parts.parts.len(), 1);
    assert!(projection.parts.wants_to_join.is_empty());
    assert!(projection.presentation.basis.plan_id.is_none());
    assert_eq!(
        projection
            .presentation
            .subjects
            .iter()
            .filter(|subject| subject.role == PresentationRole::Part)
            .count(),
        1
    );
    assert!(projection
        .presentation
        .subjects
        .iter()
        .all(|subject| { !subject.label.contains("Pico") && !subject.label.contains("tab 3") }));
}

#[test]
fn plan_play_restart_and_replan_are_distinct_revisioned_truth() {
    let mut session = LocalFrontDoor::with_identity(
        HostId::from("front-door/local"),
        BootId::from("front-door/local/boot-1"),
    )
    .unwrap();
    let initial = session.project().unwrap();
    let mut entrance = PatchbayEntranceState::enter(&initial.presentation).unwrap();
    let selected = entrance.selected_subject.clone();

    let (first_plan, first_play) = session.plan_and_play().unwrap();
    let playing = session.project().unwrap();
    assert_eq!(playing.presentation.basis.plan_id, Some(first_plan.clone()));
    assert_eq!(playing.presentation.basis.active_play_id, Some(first_play));
    assert!(playing.parts.parts[0].in_plan);
    assert!(playing.parts.parts[0].playing);
    assert_eq!(
        entrance.update(&playing.presentation).unwrap(),
        EntranceUpdateDisposition::SelectionPreserved
    );
    assert_eq!(entrance.selected_subject, selected);

    session
        .observe_local_restart(BootId::from("front-door/local/boot-2"), OfferGeneration(2))
        .unwrap();
    let restarted = session.project().unwrap();
    assert_eq!(
        restarted.presentation.basis.plan_id,
        Some(first_plan.clone())
    );
    assert!(!restarted.parts.parts[0].in_plan);
    assert!(!restarted.parts.parts[0].playing);
    assert_eq!(
        entrance.update(&restarted.presentation).unwrap(),
        EntranceUpdateDisposition::SelectionPreserved
    );
    assert!(restarted.presentation.properties.iter().any(|property| {
        property.name == "boot-id"
            && property.value
                == conduit_presentation::PresentationPropertyValue::Identity(
                    "front-door/local/boot-2".into(),
                )
    }));

    let (replacement, _) = session.plan_and_play().unwrap();
    assert_ne!(replacement, first_plan);
    let replanned = session.project().unwrap();
    assert_eq!(replanned.presentation.basis.plan_id, Some(replacement));
    assert!(replanned.parts.parts[0].in_plan);
    assert!(replanned.parts.parts[0].playing);
    assert_eq!(
        entrance.update(&replanned.presentation).unwrap(),
        EntranceUpdateDisposition::SelectionPreserved
    );
}

#[test]
fn browser_candidate_requires_proof_then_survives_offline_as_a_durable_part() {
    let mut session = LocalFrontDoor::with_identity(
        HostId::from("front-door/local"),
        BootId::from("front-door/local/boot-1"),
    )
    .unwrap();
    let identity = BrowserAdmissionIdentity::from_csprng_seed(
        HostId::from("browser/front-door"),
        BootId::from("browser/front-door/boot-1"),
        [17; 32],
    )
    .unwrap();
    let candidate_id = session
        .observe_candidate(CandidateObservation {
            advertisement: browser_advertisement(&identity),
            friendly_label: "Browser".into(),
            observed_binding_id: LinkBindingId::from("line/browser/front-door"),
            observation_sign_id: SignId::from("front-door/browser/observed"),
            proof_id: DiscoveryProofId::bind("front-door/browser/discovery").unwrap(),
            freshness_sequence: 1,
            encoded_bytes: 256,
        })
        .unwrap();
    session
        .observe_line(process_owned_line_offer_with_limits(
            "line/browser/front-door",
            "binding/browser/front-door",
            ConnectionBase::WebSocket,
            "loopback/browser/front-door",
            &browser_advertisement(&identity),
            session.advertisement(),
            LinkLimits {
                maximum_in_flight_items: 1,
                maximum_payload_bytes: 8_192,
                maximum_buffered_bytes: 16_384,
                maximum_frame_bytes: 8_192,
            },
        ))
        .unwrap();
    let candidate_projection = session.project().unwrap();
    let candidate_subject = format!("candidate/{}", candidate_id.as_str());
    let mut entrance = PatchbayEntranceState::enter(&candidate_projection.presentation).unwrap();
    entrance
        .select(&candidate_projection.presentation, &candidate_subject)
        .unwrap();
    assert_eq!(
        entrance.available_actions,
        vec![
            EntranceAction::Inspect,
            EntranceAction::Admit,
            EntranceAction::Refuse,
        ]
    );

    let challenge = session
        .begin_ambient_admission(
            &candidate_id,
            identity.verifying_key(),
            [19; 32],
            1_000,
            2_000,
            SignId::from("front-door/browser/admission-requested"),
        )
        .unwrap();
    let credential = session
        .complete_ambient_admission(
            &identity.prove(&challenge).unwrap(),
            1_001,
            AdmissionSigns {
                part_admitted: SignId::from("front-door/browser/part-admitted"),
                host_attached: SignId::from("front-door/browser/host-attached"),
                candidate_admitted: SignId::from("front-door/browser/candidate-admitted"),
            },
        )
        .unwrap();
    let attached = session.project().unwrap();
    assert_eq!(attached.parts.parts.len(), 2);
    assert!(attached.parts.wants_to_join.is_empty());
    assert_eq!(
        entrance.update(&attached.presentation).unwrap(),
        EntranceUpdateDisposition::SelectionBecameStale
    );
    let part_subject = format!("part/{}", credential.part_id.as_str());
    entrance
        .select(&attached.presentation, "line/line/browser/front-door")
        .unwrap();

    session
        .observe_line_availability(
            &LineId::from("line/browser/front-door"),
            LineAvailability::Unavailable,
            SignId::from("front-door/browser/line-unavailable"),
        )
        .unwrap();
    let line_lost = session.project().unwrap();
    assert!(line_lost.presentation.subjects.iter().any(|subject| {
        subject.role == PresentationRole::Line && subject.identity == "line/line/browser/front-door"
    }));
    assert!(line_lost.presentation.properties.iter().any(|property| {
        property.subject == "line/line/browser/front-door"
            && property.name == "availability"
            && property.value
                == conduit_presentation::PresentationPropertyValue::Text("Unavailable".into())
    }));
    assert!(line_lost
        .presentation
        .basis
        .sign_ids
        .contains(&SignId::from("front-door/browser/line-unavailable")));
    assert!(line_lost.presentation.subjects.iter().any(|subject| {
        subject.role == PresentationRole::Sign
            && subject.identity == "sign/front-door/browser/line-unavailable"
    }));
    assert_eq!(
        entrance.update(&line_lost.presentation).unwrap(),
        EntranceUpdateDisposition::SelectionPreserved
    );
    entrance
        .select(&line_lost.presentation, &part_subject)
        .unwrap();

    session
        .observe_part_offline(
            &credential.part_id,
            identity.boot_id(),
            SignId::from("front-door/browser/offline"),
        )
        .unwrap();
    let offline = session.project().unwrap();
    assert_eq!(
        entrance.update(&offline.presentation).unwrap(),
        EntranceUpdateDisposition::SelectionPreserved
    );
    assert_eq!(
        entrance.selected_subject.as_deref(),
        Some(part_subject.as_str())
    );
    let durable = offline
        .parts
        .parts
        .iter()
        .find(|part| part.details.part_id == credential.part_id)
        .unwrap();
    assert_eq!(durable.state, PartPresentationState::Offline);
    assert!(!durable.available);
    for expected in [
        "front-door/browser/part-admitted",
        "front-door/browser/host-attached",
        "front-door/browser/offline",
    ] {
        assert!(offline
            .presentation
            .basis
            .sign_ids
            .iter()
            .any(|sign| sign.as_str() == expected));
        assert!(offline.presentation.subjects.iter().any(|subject| {
            subject.role == PresentationRole::Sign && subject.identity == format!("sign/{expected}")
        }));
    }
}

#[test]
fn front_door_projects_a_bounded_pico_profile_fixture_without_promoting_it_to_live_proof() {
    let mut session = LocalFrontDoor::with_identity(
        HostId::from("front-door/local"),
        BootId::from("front-door/local/boot-1"),
    )
    .unwrap();
    let advertisement = conduit_signal::triple::exact_plan()
        .unwrap()
        .pico_advertisement;
    let identity = BrowserAdmissionIdentity::from_csprng_seed(
        advertisement.host_id.clone(),
        advertisement.boot_id.clone(),
        [31; 32],
    )
    .unwrap();
    let candidate = session
        .observe_candidate(CandidateObservation {
            advertisement: advertisement.clone(),
            friendly_label: "Pico W · deterministic CI fixture".into(),
            observed_binding_id: LinkBindingId::from("fixture/pico/usb-cdc"),
            observation_sign_id: SignId::from("fixture/pico/observed"),
            proof_id: DiscoveryProofId::bind("fixture/pico/discovery").unwrap(),
            freshness_sequence: 1,
            encoded_bytes: 512,
        })
        .unwrap();
    let challenge = session
        .begin_ambient_admission(
            &candidate,
            identity.verifying_key(),
            [37; 32],
            1_000,
            2_000,
            SignId::from("fixture/pico/admission-requested"),
        )
        .unwrap();
    session
        .complete_ambient_admission(
            &identity.prove(&challenge).unwrap(),
            1_001,
            AdmissionSigns {
                part_admitted: SignId::from("fixture/pico/part-admitted"),
                host_attached: SignId::from("fixture/pico/host-attached"),
                candidate_admitted: SignId::from("fixture/pico/candidate-admitted"),
            },
        )
        .unwrap();
    let projection = session.project().unwrap();
    let host = format!(
        "host/{}/boot/{}",
        advertisement.host_id.as_str(),
        advertisement.boot_id.as_str()
    );
    assert!(projection.presentation.properties.iter().any(|property| {
        property.subject == host
            && property.name == "profile-id"
            && property.value
                == conduit_presentation::PresentationPropertyValue::Identity(
                    advertisement.profile.as_str().into(),
                )
    }));
    assert_eq!(
        projection
            .presentation
            .subjects
            .iter()
            .filter(|subject| {
                subject.role == PresentationRole::Capability
                    && subject.identity.contains(advertisement.host_id.as_str())
            })
            .count(),
        advertisement.capabilities.len()
    );
}
