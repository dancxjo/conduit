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
fn ambient_page_admits_returns_and_projects_final_session_loss_offline() {
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
            advertisement: advertisement.clone(),
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
        drop(line);
        let mut returned = NativeWebSocketLine::connect(
            address,
            &client_url,
            MAX_BROWSER_ADMISSION_FRAME_BYTES as u32,
        )
        .unwrap();
        returned
            .send_binary(
                &serde_json::to_vec(&BrowserAdmissionIngress::ReturnAdvertise {
                    protocol: BROWSER_ADMISSION_PROTOCOL,
                    credential: credential.clone(),
                    advertisement,
                })
                .unwrap(),
            )
            .unwrap();
        let length = returned.receive_binary(&mut encoded).unwrap();
        let BrowserAdmissionEgress::ReturnChallenge { challenge, .. } =
            serde_json::from_slice(&encoded[..length]).unwrap()
        else {
            panic!("native owner did not challenge exact browser return");
        };
        let proof = identity.prove_return(&challenge).unwrap();
        returned
            .send_binary(
                &serde_json::to_vec(&BrowserAdmissionIngress::ReturnProof {
                    protocol: BROWSER_ADMISSION_PROTOCOL,
                    admission_id: proof.admission_id,
                    body_id: proof.body_id,
                    part_id: proof.part_id,
                    host_id: proof.host_id,
                    boot_id: proof.boot_id,
                    nonce: proof.nonce.to_vec(),
                    signature: proof.signature.to_vec(),
                })
                .unwrap(),
            )
            .unwrap();
        let length = returned.receive_binary(&mut encoded).unwrap();
        let returned_presence: BrowserAdmissionEgress =
            serde_json::from_slice(&encoded[..length]).unwrap();
        let renewal = BrowserAdmissionIngress::PresenceRenewal {
            protocol: BROWSER_ADMISSION_PROTOCOL,
            credential_id: credential.credential_id.clone(),
            body_id: credential.body_id.clone(),
            part_id: credential.part_id.clone(),
            host_id: credential.host_id.clone(),
            boot_id: credential.boot_id.clone(),
            sequence: 4,
        };
        returned
            .send_binary(&serde_json::to_vec(&renewal).unwrap())
            .unwrap();
        let length = returned.receive_binary(&mut encoded).unwrap();
        let returned_renewal =
            serde_json::from_slice::<BrowserAdmissionEgress>(&encoded[..length]).unwrap();
        (
            admitted,
            initial_presence,
            renewed_presence,
            returned_presence,
            returned_renewal,
        )
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
        super::super::browser_presence::BrowserPresenceCoordinator::new(membership.body_id.clone())
            .unwrap();
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
    loop {
        if presence.poll(&mut membership).unwrap().is_some() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "first session loss was not observed"
        );
        std::thread::yield_now();
    }
    assert!(membership.parts[0].current.is_none());
    let returned = loop {
        coordinator.poll_candidate(&mut inventory).unwrap();
        if let Some(returned) = coordinator.take_return() {
            break returned;
        }
        assert!(
            Instant::now() < deadline,
            "return advertisement did not arrive"
        );
        std::thread::yield_now();
    };
    assert_eq!(inventory.candidates.len(), 1);
    let (expected, offer_generation) = presence
        .return_identity(&returned.credential)
        .map(|(credential, offer)| (credential.clone(), offer))
        .unwrap();
    let proof_receiver = super::super::browser_return::begin(
        coordinator.manager_mut(),
        &membership,
        returned,
        &expected,
        offer_generation,
        [11; 32],
        1_002,
    )
    .unwrap();
    let proof = loop {
        match proof_receiver.try_recv() {
            Ok(result) => break result.unwrap(),
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(error) => panic!("return proof worker failed: {error:?}"),
        }
        assert!(Instant::now() < deadline, "return proof did not arrive");
        std::thread::yield_now();
    };
    let mut return_sign_sequence = 0;
    let (returned_part_id, returned_sequence) =
        super::super::browser_return::complete_with_presence(
            coordinator.manager_mut(),
            &mut membership,
            &mut presence,
            proof,
            1_003,
            &mut return_sign_sequence,
        )
        .unwrap();
    assert_eq!(returned_part_id, expected.part_id);
    assert_eq!(membership.parts.len(), 1);
    assert_eq!(returned_sequence, 3);
    loop {
        if presence.poll(&mut membership).unwrap().is_some() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "returned renewal was not observed"
        );
        std::thread::yield_now();
    }
    assert_eq!(presence.table().leases[0].sequence, 4);
    let (admitted_frame, presence_frame, renewed_frame, returned_frame, returned_renewal) =
        client.join().unwrap();
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
    assert!(matches!(
        returned_frame,
        BrowserAdmissionEgress::PresenceAccepted { sequence: 3, .. }
    ));
    assert!(matches!(
        returned_renewal,
        BrowserAdmissionEgress::PresenceAccepted { sequence: 4, .. }
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
