//! Codec boundaries on an actual planned/sealed route. Process-owned Lines
//! and membership observations are explicit CPU fixtures, not live transport.
use super::*;
use conduit_body::{
    AuthenticatedHostObservation, Body, BodyBiographyEvidence, BodyLifecycleSession,
    BodyMembership, MembershipProofId, PartId,
};
use conduit_browser_mask_offer::{full_profile, planned_owner_face_show_mask, MASK_SOURCE};
use conduit_core::{
    process_owned_line_offer_with_limits, resource_offer, AuthorityGrantId, BaseImplementationId,
    BootId, CheckedPlotId, CredentialReferenceId, HostProfileId, LineScope, LineSecurity,
    LinkAuthorityReference, LinkCredentialReference, LinkLimits, OfferGeneration, SignId,
    SourceDocumentId, FACE_RESOURCE_CLASS, PROTOCOL_VERSION,
};
use conduit_face::{Face, FaceBasis, FaceRole, FaceSubject, FaceText, RemoteOwnerMaskRouteSeal};

fn profile() -> OwnerFaceEgressProfile<'static> {
    OwnerFaceEgressProfile {
        execution_profile_id: full_profile::PROFILE,
        maximum_face_bytes: 512 * 1024,
        maximum_route_bytes: 64 * 1024,
        maximum_response_bytes: 640 * 1024,
        maximum_frame_bytes: 768 * 1024,
    }
}

fn host(name: &str) -> HostAdvertisement {
    let mut resources = vec![
        resource_offer("test/face", FACE_RESOURCE_CLASS, 1),
        resource_offer(
            full_profile::MEMORY_POOL,
            full_profile::MEMORY_CLASS,
            full_profile::PREPARATION_BYTES * full_profile::PREPARATION_SLOTS as u32,
        ),
    ];
    resources.sort_by(|a, b| a.pool_id.cmp(&b.pool_id));
    HostAdvertisement {
        declared_name: None,
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(format!("host/{name}")),
        boot_id: BootId::from(format!("boot/{name}")),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("test/full-egress"),
        bases: vec![],
        resources,
        capabilities: vec![full_profile::offer()],
        planner_capabilities: vec![],
    }
}

fn frame() -> BrowserAdmissionEgress {
    let owner = host("owner");
    let browser = host("browser");
    let body = Body::born(
        SourceDocumentId::from("source/full-egress"),
        CheckedPlotId::from("checked/full-egress"),
        1,
        SignId::from("sign/born-egress"),
    )
    .unwrap();
    let id = body.body_id.clone();
    let mut membership = BodyMembership::new(id.clone()).unwrap();
    let mut evidence =
        BodyBiographyEvidence::born(body, membership.clone(), "Egress".into()).unwrap();
    let part = PartId::bind(&id, browser.host_id.as_str(), 1).unwrap();
    let proof = MembershipProofId::bind("test/full-egress").unwrap();
    let admitted = membership
        .admit(
            &id,
            membership.revision,
            part.clone(),
            proof.clone(),
            SignId::from("sign/egress-admitted"),
        )
        .unwrap();
    let present = membership
        .observe_present(
            &id,
            membership.revision,
            &part,
            AuthenticatedHostObservation {
                host_id: browser.host_id.clone(),
                boot_id: browser.boot_id.clone(),
                offer_generation: browser.offer_generation,
                proof_id: proof,
                sequence: 1,
            },
            SignId::from("sign/egress-present"),
        )
        .unwrap();
    evidence
        .append_membership_events(membership, &[(admitted, 2), (present, 3)])
        .unwrap();
    let session = BodyLifecycleSession::open(evidence).unwrap();
    let face = Face::new(
        3,
        FaceBasis {
            body_id: Some(id),
            wake_id: None,
            source_document_id: None,
            checked_plot_id: None,
            expanded_plot_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![],
        },
        vec![FaceSubject {
            identity: "reading".into(),
            role: FaceRole::Plot,
            name: "Complete reading".into(),
        }],
        vec![],
        vec![],
        (0..80)
            .map(|i| FaceText {
                subject: "reading".into(),
                text: format!("{i:03} {}", "x".repeat(1000)),
            })
            .collect(),
    )
    .unwrap();
    assert!(serde_json::to_vec(&face).unwrap().len() > 60 * 1024);
    let limits = LinkLimits {
        maximum_in_flight_items: 1,
        maximum_payload_bytes: 512 * 1024,
        maximum_buffered_bytes: 4 * 512 * 1024,
        maximum_frame_bytes: 512 * 1024,
    };
    let line = |name: &str, source: &HostAdvertisement, sink: &HostAdvertisement| {
        let mut line = process_owned_line_offer_with_limits(
            name,
            &format!("binding/{name}"),
            BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
            "test/egress-base",
            source,
            sink,
            limits,
        );
        line.binding.credential = LinkCredentialReference::Opaque(CredentialReferenceId::from(
            "credential/egress-fixture",
        ));
        line.binding.authority =
            LinkAuthorityReference::Grant(AuthorityGrantId::from("grant/egress-fixture"));
        line.contract.scope = LineScope::RoutedNetwork;
        line.contract.security = LineSecurity::PlaintextNetwork;
        line
    };
    let face_line = line("line/face", &owner, &browser);
    let return_line = line("line/return", &browser, &owner);
    let planned = planned_owner_face_show_mask(
        &browser,
        &owner,
        &face_line,
        &return_line,
        MASK_SOURCE,
        "browser-graphical",
    )
    .unwrap();
    let route = RemoteOwnerMaskRouteSeal::seal_current_hosted_presentation(
        &session,
        &face,
        &owner,
        &browser,
        &planned,
        &face_line,
        &return_line,
    )
    .unwrap();
    route.verify_seal().unwrap();
    BrowserAdmissionEgress::FaceSnapshotResponse {
        protocol: BROWSER_ADMISSION_PROTOCOL,
        response: OwnerFaceSnapshotResponse::Snapshot {
            schema: OWNER_FACE_RESPONSE_SCHEMA.into(),
            face: Box::new(face),
            interactions_admitted: false,
            route: Some(Box::new(route)),
        },
    }
}

#[test]
fn exact_full_profile_preserves_complete_large_face_while_default_refuses() {
    let frame = frame();
    let mut output = vec![0; profile().maximum_frame_bytes];
    assert_eq!(
        encode_browser_admission_frame(&frame, &mut output),
        Err(BrowserAdmissionFrameError::InvalidFaceSnapshot)
    );
    let length =
        encode_browser_admission_face_frame_for_profile(&frame, &profile(), &mut output).unwrap();
    assert!(length > 60 * 1024);
    let encoded = serde_json::to_vec(&frame).unwrap();
    assert_eq!(&output[..length], encoded);
}

#[test]
fn wrong_profile_face_identity_and_changed_seal_are_refused() {
    let mut frame = frame();
    let mut output = vec![0; profile().maximum_frame_bytes];
    let mut wrong = profile();
    wrong.execution_profile_id = "browser/owner-face-other@1";
    assert_eq!(
        encode_browser_admission_face_frame_for_profile(&frame, &wrong, &mut output),
        Err(BrowserAdmissionFrameError::InvalidFaceSnapshot)
    );
    let BrowserAdmissionEgress::FaceSnapshotResponse {
        response: OwnerFaceSnapshotResponse::Snapshot {
            route: Some(route), ..
        },
        ..
    } = &mut frame
    else {
        unreachable!()
    };
    let old_revision = route.face_revision;
    route.face_revision += 1;
    assert_eq!(
        encode_browser_admission_face_frame_for_profile(&frame, &profile(), &mut output),
        Err(BrowserAdmissionFrameError::InvalidFaceSnapshot)
    );
    let BrowserAdmissionEgress::FaceSnapshotResponse {
        response: OwnerFaceSnapshotResponse::Snapshot {
            route: Some(route), ..
        },
        ..
    } = &mut frame
    else {
        unreachable!()
    };
    route.face_revision = old_revision;
    route.route_plan_id = conduit_core::PlanId::from("plan/foreign-seal");
    assert_eq!(
        encode_browser_admission_face_frame_for_profile(&frame, &profile(), &mut output),
        Err(BrowserAdmissionFrameError::InvalidFaceSnapshot)
    );
}

#[test]
fn actual_output_extent_is_enforced_without_changing_its_storage() {
    let frame = frame();
    let mut output = vec![0; 128];
    let pointer = output.as_ptr();
    let capacity = output.capacity();
    assert_eq!(
        encode_browser_admission_face_frame_for_profile(&frame, &profile(), &mut output),
        Err(BrowserAdmissionFrameError::OutputTooSmall)
    );
    assert_eq!(output.len(), 128);
    assert_eq!(output.capacity(), capacity);
    assert_eq!(output.as_ptr(), pointer);
}
