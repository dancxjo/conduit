use super::*;
use conduit_body::{Body, SpawnInvitationClaim};
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
fn spawn_secret_is_fragment_only_and_transport_urls_are_encoded() {
    let target = spawn_target(
        "http://127.0.0.1:8080/index.html",
        "ws://127.0.0.1:9000/chat?line=one",
        "ws://127.0.0.1:9001/admit?body=one",
        "deadbeef",
    );
    let (request, fragment) = target.split_once('#').unwrap();

    assert_eq!(
        request,
        "http://127.0.0.1:8080/index.html?ws=ws%3A%2F%2F127.0.0.1%3A9000%2Fchat%3Fline%3Done"
    );
    assert!(!request.contains("deadbeef"));
    assert_eq!(
        fragment,
        "body=ws%3A%2F%2F127.0.0.1%3A9001%2Fadmit%3Fbody%3Done&spawn_hex=deadbeef"
    );
}

#[test]
fn cancellation_is_explicit_and_idempotently_fail_closed() {
    let mut coordinator = BrowserPartsCoordinator::new("page".into(), "chat".into());
    assert!(!coordinator.cancel());
    assert!(!coordinator.is_pending());
}

#[test]
fn current_plan_derives_only_its_exact_selected_webrtc_session_grant() {
    let identity = patchbay_model::RendererAdapterIdentity {
        host_id: HostId::from("browser/granted-sink"),
        boot_id: BootId::from("browser-boot/granted-sink"),
        target_subject: "subject/renderer".into(),
    };
    let mut exact = patchbay_model::cross_host_renderer_plan(
        HostId::from("browser/granted-source"),
        BootId::from("browser-boot/granted-source"),
        identity,
    )
    .unwrap();
    let connection = exact
        .plan
        .fragments
        .iter_mut()
        .find_map(|fragment| fragment.connections.first_mut())
        .unwrap();
    let selected = connection.selected_line.as_mut().unwrap();
    selected.binding.base =
        conduit_core::BaseImplementationId::from("conduit.base/webrtc-data-channel@1");
    connection.admitted_lines[0] = selected.clone();
    let binding_id = selected.binding.binding_id.clone();

    let grants = planned_webrtc_bindings(&exact.plan).unwrap();
    assert_eq!(grants.len(), 1);
    assert_eq!(grants[0].plan_id, exact.plan.plan_id);
    assert_eq!(
        grants[0].attachment.base,
        conduit_core::BaseImplementationId::from("conduit.base/webrtc-data-channel@1")
    );
    assert_eq!(grants[0].attachment.link_binding_id, binding_id);
    assert_eq!(grants[0].source.host_id.as_str(), "browser/granted-source");
    assert_eq!(grants[0].sink.host_id.as_str(), "browser/granted-sink");
}

#[test]
fn body_spawn_listener_accepts_one_exact_return_on_the_original_url() {
    run_body_spawn_return(None);
}

#[test]
fn exhausted_return_sequence_refuses_before_any_canonical_or_presence_mutation() {
    run_body_spawn_return(Some(ReturnPreflightFault::SequenceOverflow));
}

#[test]
fn available_return_lease_refuses_before_any_canonical_or_presence_mutation() {
    run_body_spawn_return(Some(ReturnPreflightFault::AvailableLease));
}

#[test]
fn drifted_return_lease_refuses_before_any_canonical_or_presence_mutation() {
    run_body_spawn_return(Some(ReturnPreflightFault::DriftedLease));
}

#[test]
fn exhausted_return_worker_capacity_refuses_before_any_mutation() {
    run_body_spawn_return(Some(ReturnPreflightFault::WorkerCapacity));
}

#[test]
fn exhausted_return_session_sequence_refuses_before_any_mutation() {
    run_body_spawn_return(Some(ReturnPreflightFault::SessionOverflow));
}

#[test]
fn exhausted_return_sign_sequence_refuses_before_any_mutation() {
    run_body_spawn_return(Some(ReturnPreflightFault::SignOverflow));
}

#[derive(Clone, Copy)]
pub(super) enum ReturnPreflightFault {
    SequenceOverflow,
    AvailableLease,
    DriftedLease,
    WorkerCapacity,
    SessionOverflow,
    SignOverflow,
}

impl ReturnPreflightFault {
    fn expected_error(self) -> &'static str {
        match self {
            Self::SequenceOverflow => "sequence exhausted",
            Self::AvailableLease => "still available",
            Self::DriftedLease => "identity drifted",
            Self::WorkerCapacity => "worker capacity exhausted",
            Self::SessionOverflow => "session sequence exhausted",
            Self::SignOverflow => "Sign sequence exhausted",
        }
    }
}

fn run_body_spawn_return(fault: Option<ReturnPreflightFault>) {
    let body = Body::born(
        SourceDocumentId::from("source/native-spawn-return"),
        CheckedFormId::from("checked/native-spawn-return"),
        1,
        conduit_core::SignId::from("sign/native-spawn-return/body"),
    )
    .unwrap();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let mut coordinator = BrowserPartsCoordinator::new("page".into(), "chat".into());
    let target = coordinator.begin(&body.body_id).unwrap();
    let fragment = target.split_once('#').unwrap().1;
    let body_url = percent_decode(field(fragment, "body"));
    let envelope_bytes = decode_hex(field(fragment, "spawn_hex"));
    let envelope: serde_json::Value = serde_json::from_slice(&envelope_bytes).unwrap();
    let claim: SpawnInvitationClaim = serde_json::from_value(envelope["claim"].clone()).unwrap();
    let secret: [u8; 32] = envelope["secret"]
        .as_array()
        .unwrap()
        .iter()
        .map(|byte| byte.as_u64().unwrap() as u8)
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    let identity = BrowserAdmissionIdentity::from_csprng_seed(
        HostId::from("browser/native-spawn-return"),
        BootId::from("browser-boot/native-spawn-return"),
        secret,
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
    let client_url = body_url.clone();
    let client = std::thread::spawn(move || {
        let address: SocketAddr = client_url
            .strip_prefix("ws://")
            .unwrap()
            .strip_suffix("/conduit")
            .unwrap()
            .parse()
            .unwrap();
        let mut encoded = [0; MAX_BROWSER_ADMISSION_FRAME_BYTES];
        let mut line = NativeWebSocketLine::connect(
            address,
            &client_url,
            MAX_BROWSER_ADMISSION_FRAME_BYTES as u32,
        )
        .unwrap();
        send(
            &mut line,
            &BrowserAdmissionIngress::Advertise {
                protocol: BROWSER_ADMISSION_PROTOCOL,
                advertisement: advertisement.clone(),
                friendly_label: "Spawned browser".into(),
                verifying_key: identity.verifying_key().to_vec(),
                freshness_sequence: 1,
            },
        );
        let signature = SpawnInvitationSecret::from_csprng_bytes(secret)
            .unwrap()
            .sign(&claim.signing_transcript(
                identity.host_id(),
                identity.boot_id(),
                advertisement.offer_generation,
            ));
        send(
            &mut line,
            &BrowserAdmissionIngress::SpawnProof {
                protocol: BROWSER_ADMISSION_PROTOCOL,
                invitation_id: claim.invitation_id,
                body_id: claim.body_id,
                host_id: identity.host_id().clone(),
                boot_id: identity.boot_id().clone(),
                nonce: claim.nonce.to_vec(),
                signature: signature.to_vec(),
            },
        );
        let admitted = receive(&mut line, &mut encoded);
        let credential = match admitted {
            BrowserAdmissionEgress::Admitted { credential, .. } => credential,
            frame => panic!("spawn was not admitted: {frame:?}"),
        };
        assert!(matches!(
            receive(&mut line, &mut encoded),
            BrowserAdmissionEgress::PresenceAccepted { sequence: 1, .. }
        ));
        drop(line);

        let mut returned = NativeWebSocketLine::connect(
            address,
            &client_url,
            MAX_BROWSER_ADMISSION_FRAME_BYTES as u32,
        )
        .unwrap();
        send(
            &mut returned,
            &BrowserAdmissionIngress::ReturnAdvertise {
                protocol: BROWSER_ADMISSION_PROTOCOL,
                credential: credential.clone(),
                advertisement,
            },
        );
        let BrowserAdmissionEgress::ReturnChallenge { challenge, .. } =
            receive(&mut returned, &mut encoded)
        else {
            panic!("spawn rendezvous did not challenge the return");
        };
        let proof = identity.prove_return(&challenge).unwrap();
        send(
            &mut returned,
            &BrowserAdmissionIngress::ReturnProof {
                protocol: BROWSER_ADMISSION_PROTOCOL,
                admission_id: proof.admission_id,
                body_id: proof.body_id,
                part_id: proof.part_id,
                host_id: proof.host_id,
                boot_id: proof.boot_id,
                nonce: proof.nonce.to_vec(),
                signature: proof.signature.to_vec(),
            },
        );
        receive(&mut returned, &mut encoded)
    });

    let deadline = Instant::now() + Duration::from_secs(5);
    let arrival = loop {
        if let Some(arrival) = coordinator.take_arrival().unwrap() {
            break arrival;
        }
        assert!(Instant::now() < deadline, "spawn did not arrive");
        std::thread::yield_now();
    };
    let credential = coordinator
        .complete(
            arrival,
            &mut membership,
            AdmissionSigns {
                part_admitted: conduit_core::SignId::from("sign/native-spawn-return/part"),
                host_attached: conduit_core::SignId::from("sign/native-spawn-return/host"),
                candidate_admitted: conduit_core::SignId::from(
                    "sign/native-spawn-return/candidate",
                ),
            },
        )
        .unwrap();
    wait_for(
        &deadline,
        || coordinator.poll_presence(&mut membership),
        "session loss",
    );
    wait_for(
        &deadline,
        || coordinator.poll_return(&mut membership),
        "return challenge",
    );
    if let Some(fault) = fault {
        coordinator.inject_return_preflight_fault_for_test(&credential.part_id, &credential, fault);
        let membership_before = membership.clone();
        let state_before = coordinator.atomic_return_state_for_test();
        let error = wait_for_error(
            &deadline,
            || coordinator.poll_return(&mut membership),
            "return preflight refusal",
        );
        assert!(error.contains(fault.expected_error()), "{error}");
        assert_eq!(membership, membership_before);
        assert_eq!(coordinator.atomic_return_state_for_test(), state_before);
        assert!(matches!(
            client.join().unwrap(),
            BrowserAdmissionEgress::Refused { code, .. }
                if code == "return-presence-not-admissible"
        ));
        return;
    }
    let returned = wait_for(
        &deadline,
        || coordinator.poll_return(&mut membership),
        "return proof",
    );
    assert!(returned.contains("fresh presence sequence 2"));
    assert_eq!(membership.parts.len(), 1);
    assert_eq!(membership.parts[0].part_id, credential.part_id);
    assert!(matches!(
        client.join().unwrap(),
        BrowserAdmissionEgress::PresenceAccepted { sequence: 2, .. }
    ));
}

fn wait_for_error(
    deadline: &Instant,
    mut poll: impl FnMut() -> Result<Option<String>, String>,
    context: &str,
) -> String {
    loop {
        if let Err(error) = poll() {
            return error;
        }
        assert!(Instant::now() < *deadline, "{context} was not observed");
        std::thread::yield_now();
    }
}

fn wait_for(
    deadline: &Instant,
    mut poll: impl FnMut() -> Result<Option<String>, String>,
    context: &str,
) -> String {
    loop {
        if let Some(message) = poll().unwrap() {
            return message;
        }
        assert!(Instant::now() < *deadline, "{context} was not observed");
        std::thread::yield_now();
    }
}

fn field<'a>(fragment: &'a str, name: &str) -> &'a str {
    fragment
        .split('&')
        .find_map(|field| field.strip_prefix(&format!("{name}=")))
        .unwrap()
}

fn percent_decode(value: &str) -> String {
    let mut bytes = Vec::new();
    let mut index = 0;
    while index < value.len() {
        if value.as_bytes()[index] == b'%' {
            bytes.push(u8::from_str_radix(&value[index + 1..index + 3], 16).unwrap());
            index += 3;
        } else {
            bytes.push(value.as_bytes()[index]);
            index += 1;
        }
    }
    String::from_utf8(bytes).unwrap()
}

fn decode_hex(value: &str) -> Vec<u8> {
    value
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

fn send(line: &mut NativeWebSocketLine, frame: &BrowserAdmissionIngress) {
    line.send_binary(&serde_json::to_vec(frame).unwrap())
        .unwrap();
}

fn receive(
    line: &mut NativeWebSocketLine,
    encoded: &mut [u8; MAX_BROWSER_ADMISSION_FRAME_BYTES],
) -> BrowserAdmissionEgress {
    let length = line.receive_binary(encoded).unwrap();
    serde_json::from_slice(&encoded[..length]).unwrap()
}
