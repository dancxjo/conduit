use super::*;
use conduit_browser_runtime::membership::BrowserAdmissionIdentity;
use conduit_core::{
    BootId, CheckedFormId, HostAdvertisement, HostId, HostProfileId, OfferGeneration,
    SourceDocumentId,
};
use conduit_std_host::{
    browser_admission::MAX_BROWSER_ADMISSION_FRAME_BYTES, websocket::NativeWebSocketLine,
};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

#[test]
fn ambient_page_admits_then_live_owner_projects_session_loss_offline() {
    let body_id = conduit_body::Body::born(
        SourceDocumentId::from("source/native-ambient-test"),
        CheckedFormId::from("checked/native-ambient-test"),
        1,
        SignId::from("sign/native-ambient/body"),
    )
    .unwrap()
    .body_id;
    let (mut coordinator, url) = AmbientBrowserCoordinator::start(body_id.clone()).unwrap();
    let identity = BrowserAdmissionIdentity::from_csprng_seed(
        HostId::from("browser/native-ambient"),
        BootId::from("browser-boot/native-ambient"),
        [7; 32],
    )
    .unwrap();
    let advertisement = HostAdvertisement {
        protocol_version: conduit_core::PROTOCOL_VERSION,
        host_id: identity.host_id().clone(),
        boot_id: identity.boot_id().clone(),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("browser/host"),
        resources: Vec::new(),
        capabilities: Vec::new(),
        planner_capabilities: Vec::new(),
    };
    let client_url = url.clone();
    let client = std::thread::spawn(move || {
        let address: SocketAddr = client_url
            .strip_prefix("ws://")
            .unwrap()
            .strip_suffix("/conduit")
            .unwrap()
            .parse()
            .unwrap();
        let mut line = NativeWebSocketLine::connect(
            address,
            &client_url,
            MAX_BROWSER_ADMISSION_FRAME_BYTES as u32,
        )
        .unwrap();
        let advertise = BrowserAdmissionIngress::Advertise {
            protocol: BROWSER_ADMISSION_PROTOCOL,
            advertisement,
            friendly_label: "This computer".into(),
            verifying_key: identity.verifying_key().to_vec(),
            freshness_sequence: 1,
        };
        let mut encoded = [0; MAX_BROWSER_ADMISSION_FRAME_BYTES];
        let bytes = serde_json::to_vec(&advertise).unwrap();
        line.send_binary(&bytes).unwrap();
        let length = line.receive_binary(&mut encoded).unwrap();
        let frame: BrowserAdmissionEgress = serde_json::from_slice(&encoded[..length]).unwrap();
        let BrowserAdmissionEgress::Challenge { challenge, .. } = frame else {
            panic!("explicit Admit must send a challenge");
        };
        let proof = identity.prove(&challenge).unwrap();
        let proof = BrowserAdmissionIngress::AmbientProof {
            protocol: BROWSER_ADMISSION_PROTOCOL,
            admission_id: proof.admission_id,
            body_id: proof.body_id,
            host_id: proof.host_id,
            boot_id: proof.boot_id,
            nonce: proof.nonce.to_vec(),
            signature: proof.signature.to_vec(),
        };
        let bytes = serde_json::to_vec(&proof).unwrap();
        line.send_binary(&bytes).unwrap();
        let length = line.receive_binary(&mut encoded).unwrap();
        let admitted =
            serde_json::from_slice::<BrowserAdmissionEgress>(&encoded[..length]).unwrap();
        let length = line.receive_binary(&mut encoded).unwrap();
        let initial_presence =
            serde_json::from_slice::<BrowserAdmissionEgress>(&encoded[..length]).unwrap();
        let BrowserAdmissionEgress::Admitted { credential, .. } = &admitted else {
            panic!("admission credential was not returned");
        };
        let renewal = BrowserAdmissionIngress::PresenceRenewal {
            protocol: BROWSER_ADMISSION_PROTOCOL,
            credential_id: credential.credential_id.clone(),
            body_id: credential.body_id.clone(),
            part_id: credential.part_id.clone(),
            host_id: credential.host_id.clone(),
            boot_id: credential.boot_id.clone(),
            sequence: 2,
        };
        line.send_binary(&serde_json::to_vec(&renewal).unwrap())
            .unwrap();
        let length = line.receive_binary(&mut encoded).unwrap();
        let renewed_presence =
            serde_json::from_slice::<BrowserAdmissionEgress>(&encoded[..length]).unwrap();
        (admitted, initial_presence, renewed_presence)
    });

    let mut inventory = CandidateInventory::new(body_id.clone()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let candidate_id = loop {
        if let Some(candidate) = coordinator.poll_candidate(&mut inventory).unwrap() {
            break candidate;
        }
        assert!(Instant::now() < deadline, "candidate did not arrive");
        std::thread::yield_now();
    };
    assert_eq!(inventory.candidates.len(), 1);
    assert_eq!(
        inventory.candidates[0].state,
        conduit_body::CandidateState::Discovered
    );
    let mut membership = BodyMembership::new(body_id).unwrap();
    assert!(membership.parts.is_empty());
    coordinator
        .admit(
            &mut inventory,
            &candidate_id,
            [9; 32],
            1_000,
            SignId::from("sign/native-ambient/requested"),
        )
        .unwrap();
    let arrival = loop {
        if let Some(arrival) = coordinator.take_proof().unwrap() {
            break arrival;
        }
        assert!(Instant::now() < deadline, "proof did not arrive");
        std::thread::yield_now();
    };
    let admitted = coordinator
        .complete(
            arrival,
            &mut inventory,
            &mut membership,
            1_001,
            AdmissionSigns {
                part_admitted: SignId::from("sign/native-ambient/part"),
                host_attached: SignId::from("sign/native-ambient/host"),
                candidate_admitted: SignId::from("sign/native-ambient/candidate"),
            },
        )
        .unwrap();
    assert_eq!(membership.parts.len(), 1);
    assert_eq!(membership.parts[0].part_id, admitted.credential.part_id);
    let part_id = admitted.credential.part_id.clone();
    let mut presence =
        super::super::browser_presence::BrowserPresenceCoordinator::new(membership.body_id.clone());
    presence
        .register(admitted.socket, admitted.credential, &mut membership)
        .unwrap();
    let admitted_revision = membership.revision;
    loop {
        if presence.poll(&mut membership).unwrap().is_some() {
            break;
        }
        assert!(Instant::now() < deadline, "renewal was not observed");
        std::thread::yield_now();
    }
    assert_eq!(membership.revision, admitted_revision);
    assert_eq!(presence.table().leases[0].sequence, 2);
    let (admitted_frame, presence_frame, renewed_frame) = client.join().unwrap();
    assert!(matches!(
        admitted_frame,
        BrowserAdmissionEgress::Admitted { .. }
    ));
    assert!(matches!(
        presence_frame,
        BrowserAdmissionEgress::PresenceAccepted { sequence: 1, .. }
    ));
    assert!(matches!(
        renewed_frame,
        BrowserAdmissionEgress::PresenceAccepted { sequence: 2, .. }
    ));
    let offline = loop {
        if let Some(message) = presence.poll(&mut membership).unwrap() {
            break message;
        }
        assert!(Instant::now() < deadline, "session loss was not observed");
        std::thread::yield_now();
    };
    assert!(offline.contains("durable membership remains"));
    assert!(membership.parts[0].current.is_none());
    let lease = presence
        .table()
        .leases
        .iter()
        .find(|lease| lease.part_id == part_id)
        .unwrap();
    assert_eq!(lease.state, conduit_body::HostPresenceState::Unavailable);
    assert!(matches!(
        presence.table().events.last().unwrap().kind,
        conduit_body::HostPresenceEventKind::SessionLost
    ));
}
