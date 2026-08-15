use super::*;
use conduit_core::{HostProfileId, OfferGeneration, PROTOCOL_VERSION};
use std::net::SocketAddr;

fn advertisement() -> BrowserAdmissionIngress {
    BrowserAdmissionIngress::Advertise {
        protocol: BROWSER_ADMISSION_PROTOCOL,
        advertisement: HostAdvertisement {
            protocol_version: PROTOCOL_VERSION,
            host_id: HostId::from("browser/frame-host"),
            boot_id: BootId::from("browser/frame-boot"),
            offer_generation: OfferGeneration(1),
            profile: HostProfileId::from("browser/frame-profile"),
            resources: Vec::new(),
            capabilities: Vec::new(),
            planner_capabilities: Vec::new(),
        },
        friendly_label: "Browser".into(),
        verifying_key: vec![7; 32],
        freshness_sequence: 1,
    }
}

#[test]
fn bounded_advertisement_round_trips_without_creating_membership() {
    let encoded = serde_json::to_vec(&advertisement()).unwrap();
    assert_eq!(
        decode_browser_admission_frame(&encoded),
        Ok(advertisement())
    );
}

#[test]
fn malformed_oversized_and_bad_key_frames_are_distinct() {
    assert_eq!(
        decode_browser_admission_frame(b"{"),
        Err(BrowserAdmissionFrameError::Malformed)
    );
    assert_eq!(
        decode_browser_admission_frame(&vec![b'x'; MAX_BROWSER_ADMISSION_FRAME_BYTES + 1]),
        Err(BrowserAdmissionFrameError::Oversized)
    );
    let BrowserAdmissionIngress::Advertise {
        protocol,
        advertisement,
        friendly_label,
        freshness_sequence,
        ..
    } = advertisement()
    else {
        unreachable!()
    };
    let bad_key = BrowserAdmissionIngress::Advertise {
        protocol,
        advertisement,
        friendly_label,
        verifying_key: vec![0; 31],
        freshness_sequence,
    };
    assert_eq!(
        decode_browser_admission_frame(&serde_json::to_vec(&bad_key).unwrap()),
        Err(BrowserAdmissionFrameError::InvalidVerifyingKey)
    );
}

#[test]
fn presence_renewal_round_trips_and_zero_sequence_is_refused() {
    let encoded = br#"{"kind":"presence-renewal","protocol":1,"credential_id":"credential/browser","body_id":"body/browser","part_id":"part/browser","host_id":"host/browser","boot_id":"boot/browser","sequence":2}"#;
    assert!(matches!(
        decode_browser_admission_frame(encoded),
        Ok(BrowserAdmissionIngress::PresenceRenewal { sequence: 2, .. })
    ));
    let stale = br#"{"kind":"presence-renewal","protocol":1,"credential_id":"credential/browser","body_id":"body/browser","part_id":"part/browser","host_id":"host/browser","boot_id":"boot/browser","sequence":0}"#;
    assert_eq!(
        decode_browser_admission_frame(stale),
        Err(BrowserAdmissionFrameError::InvalidSequence)
    );
}

#[test]
fn bounded_webrtc_grant_request_and_reply_round_trip() {
    let request = BrowserAdmissionIngress::WebRtcGrantRequest {
        protocol: BROWSER_ADMISSION_PROTOCOL,
        credential_id: serde_json::from_value(serde_json::json!("credential/browser")).unwrap(),
        body_id: serde_json::from_value(serde_json::json!("body/browser")).unwrap(),
        part_id: serde_json::from_value(serde_json::json!("part/browser")).unwrap(),
        host_id: HostId::from("host/browser"),
        boot_id: BootId::from("boot/browser"),
        index: 0,
    };
    assert_eq!(
        decode_browser_admission_frame(&serde_json::to_vec(&request).unwrap()),
        Ok(request)
    );

    let reply = BrowserAdmissionEgress::WebRtcGrant {
        protocol: BROWSER_ADMISSION_PROTOCOL,
        index: 0,
        total: 1,
        grant: Some(BrowserWebRtcGrant {
            negotiation_id: conduit_core::LinkBindingId::from("binding/browser"),
            role: BrowserWebRtcRole::Source,
            peer_host_id: HostId::from("host/peer"),
            peer_boot_id: BootId::from("boot/peer"),
            session_hello: vec![1, 2, 3],
        }),
    };
    let mut output = [0; MAX_BROWSER_ADMISSION_FRAME_BYTES];
    let length = encode_browser_admission_frame(&reply, &mut output).unwrap();
    assert_eq!(
        serde_json::from_slice::<BrowserAdmissionEgress>(&output[..length]).unwrap(),
        reply
    );
}

#[test]
fn exact_return_frames_decode_and_malformed_proof_remains_distinct() {
    let advertisement = br#"{"kind":"return-advertise","protocol":1,"credential":{"credential_id":"credential/live","body_id":"body/live","part_id":"part/live","host_id":"browser/live","boot_id":"browser-boot/live","issued_at_millis":1000},"advertisement":{"protocol_version":1,"host_id":"browser/live","boot_id":"browser-boot/live","offer_generation":1,"profile":"browser/profile","resources":[],"capabilities":[],"planner_capabilities":[]}}"#;
    assert!(matches!(
        decode_browser_admission_frame(advertisement),
        Ok(BrowserAdmissionIngress::ReturnAdvertise { .. })
    ));
    let malformed_proof = br#"{"kind":"return-proof","protocol":1,"admission_id":"return/live","body_id":"body/live","part_id":"part/live","host_id":"browser/live","boot_id":"browser-boot/live","nonce":[1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1],"signature":[]}"#;
    assert_eq!(
        decode_browser_admission_frame(malformed_proof),
        Err(BrowserAdmissionFrameError::InvalidSignature)
    );
}

#[test]
fn egress_refuses_wrong_protocol_and_too_small_output() {
    let wrong = BrowserAdmissionEgress::Refused {
        protocol: 2,
        code: "wrong-body".into(),
    };
    assert_eq!(
        encode_browser_admission_frame(&wrong, &mut [0; 128]),
        Err(BrowserAdmissionFrameError::WrongProtocol)
    );
    let frame = BrowserAdmissionEgress::Refused {
        protocol: BROWSER_ADMISSION_PROTOCOL,
        code: "invalid-proof".into(),
    };
    assert_eq!(
        encode_browser_admission_frame(&frame, &mut [0; 1]),
        Err(BrowserAdmissionFrameError::OutputTooSmall)
    );
}

#[test]
fn loopback_socket_carries_one_exact_inert_advertisement_and_refusal() {
    let listener = BrowserAdmissionListener::bind_loopback().unwrap();
    let url = listener.url().unwrap();
    let address: SocketAddr = url
        .strip_prefix("ws://")
        .unwrap()
        .strip_suffix("/conduit")
        .unwrap()
        .parse()
        .unwrap();
    let expected = advertisement();
    let client_url = url.clone();
    let client = std::thread::spawn(move || {
        let mut line = NativeWebSocketLine::connect(
            address,
            &client_url,
            MAX_BROWSER_ADMISSION_FRAME_BYTES as u32,
        )
        .unwrap();
        line.send_binary(&serde_json::to_vec(&expected).unwrap())
            .unwrap();
        let mut response = [0; MAX_BROWSER_ADMISSION_FRAME_BYTES];
        let length = line.receive_binary(&mut response).unwrap();
        serde_json::from_slice::<BrowserAdmissionEgress>(&response[..length]).unwrap()
    });
    let mut socket = listener.accept().unwrap();
    assert_eq!(socket.receive().unwrap(), advertisement());
    socket
        .send(&BrowserAdmissionEgress::Refused {
            protocol: BROWSER_ADMISSION_PROTOCOL,
            code: "explicit-operator-refusal".into(),
        })
        .unwrap();
    assert_eq!(
        client.join().unwrap(),
        BrowserAdmissionEgress::Refused {
            protocol: BROWSER_ADMISSION_PROTOCOL,
            code: "explicit-operator-refusal".into(),
        }
    );
}
