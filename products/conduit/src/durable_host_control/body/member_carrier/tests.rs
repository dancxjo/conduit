use super::*;
use crate::durable_host_control::{handle, Request, Response, PROTOCOL};
use conduit_std_host::StdHost;
fn requests(token: u8) -> Vec<Request> {
    let ad = StdHost::new().advertisement().clone();
    let credential: MembershipCredential = serde_json::from_value(serde_json::json!({
        "credential_id":"fixture/credential", "body_id":"fixture/body", "part_id":"fixture/part",
        "host_id":ad.host_id, "boot_id":ad.boot_id, "issued_at_millis":1
    }))
    .unwrap();
    let peer = conduit_body::FederationPeer {
        host_id: ad.host_id.clone(),
        boot_id: ad.boot_id.clone(),
        offer_generation: ad.offer_generation,
        verifying_key: [7; 32],
    };
    let challenge = FederationChallenge {
        session_id: "fixture/session".into(),
        line_id: "fixture/line".into(),
        line_security: LineSecurity::AuthenticatedEncrypted,
        initiator: peer.clone(),
        responder: peer,
        nonce: [9; 32],
        expires_at_millis: 100,
        maximum_frames: 2,
    };
    vec![
        Request::BodyMemberCarrierReceiver {
            protocol: PROTOCOL,
            token: vec![token; 32],
        },
        Request::BodyAttestAdmissionReceipt {
            protocol: PROTOCOL,
            token: vec![token; 32],
            receipt: Box::new(
                crate::durable_host::owner::MembershipReceiptDocument::Spawn(
                    conduit_body::PortableAdmissionReceipt {
                        schema: conduit_body::SPAWN_ADMISSION_RECEIPT_SCHEMA.into(),
                        credential: credential.clone(),
                        host_advertisement: ad.clone(),
                        membership_admitted: true,
                        current_offers_available: !ad.capabilities.is_empty(),
                        plan_created: false,
                        play_created: false,
                    },
                ),
            ),
            expected_owner: Box::new(challenge.responder.clone()),
        },
        Request::BodyAttestAdmissionReceipt {
            protocol: PROTOCOL,
            token: vec![token; 32],
            receipt: Box::new(
                crate::durable_host::owner::MembershipReceiptDocument::Return(
                    conduit_body::MemberReturnAcknowledgement::new(credential.clone(), ad.clone()),
                ),
            ),
            expected_owner: Box::new(challenge.responder.clone()),
        },
        Request::BodyMemberRecoverReturnBegin {
            protocol: PROTOCOL,
            token: vec![token; 32],
            credential: credential.clone(),
            advertisement: Box::new(ad.clone()),
            presentation: Box::new(conduit_body::MemberReturnImagePresentation {
                kind: "Advertise".into(),
                schema: conduit_body::MEMBER_RETURN_IMAGES_SCHEMA.into(),
                credential: credential.clone(),
                projection: conduit_body::HostAdvertisementImageProjection {
                    context: {
                        let mut context = ad.clone();
                        context.capabilities.clear();
                        context
                    },
                    capability_images: Vec::new(),
                    authority_grants: Vec::new(),
                },
                signature: vec![0; 64],
            }),
            images: Vec::new(),
        },
        Request::BodyMemberCarrierBegin {
            protocol: PROTOCOL,
            token: vec![token; 32],
            credential,
            advertisement: Box::new(ad),
            line: "fixture/line".into(),
            security: LineSecurity::AuthenticatedEncrypted,
            expires_at_millis: 100,
            maximum_frames: 2,
        },
        Request::BodyMemberCarrierComplete {
            protocol: PROTOCOL,
            token: vec![token; 32],
            challenge: Box::new(challenge),
            signature: vec![0; 64],
        },
        Request::BodyMemberPresenceRenew {
            protocol: PROTOCOL,
            token: vec![token; 32],
            session: "fixture/session".into(),
            sequence: 1,
        },
        Request::BodyMemberCarrierInspect {
            protocol: PROTOCOL,
            token: vec![token; 32],
            session: "fixture/session".into(),
        },
        Request::BodyMemberCarrierRetire {
            protocol: PROTOCOL,
            token: vec![token; 32],
            session: "fixture/session".into(),
        },
    ]
}
#[test]
fn previous_local_control_revision_refuses_before_selecting_a_carrier_owner() {
    let mut runtime = DurableHostRuntime::new(
        "fixture/target".into(),
        "fixture/image".into(),
        StdHost::new(),
    );
    let before = runtime.truth().advertisement;
    let request = Request::BodyMemberCarrierReceiver {
        protocol: PROTOCOL - 1,
        token: vec![9; 32],
    };
    assert!(
        matches!(handle(request,&[9;32],&mut runtime),Response::Refused{code,..} if code=="protocol")
    );
    assert_eq!(runtime.truth().advertisement, before);
}

#[test]
fn member_carrier_control_requires_authenticated_local_caller_and_real_body_owner() {
    let mut runtime = DurableHostRuntime::new(
        "fixture/target".into(),
        "fixture/image".into(),
        StdHost::new(),
    );
    let original = runtime.truth().advertisement;
    for token in [8, 9] {
        for request in requests(token) {
            // Actual typed wire round-trip; neither malformed credentials
            // nor a caller's transport description can create an owner.
            let encoded = serde_json::to_vec(&request).unwrap();
            let request = serde_json::from_slice(&encoded).unwrap();
            let recovery = matches!(&request, Request::BodyMemberRecoverReturnBegin { .. });
            let Response::Refused { code, .. } = handle(request, &[9; 32], &mut runtime) else {
                panic!("a control token cannot manufacture a live Body owner");
            };
            if token == 8 {
                assert_eq!(code, "unauthorized");
            } else if recovery {
                assert_eq!(code, "installed Host does not own a live Body session");
            } else {
                assert!(code.contains("live installed owner"), "{code}");
            }
        }
    }
    assert_eq!(runtime.truth().advertisement, original);
}

fn active_runtime(
    image: bool,
    issuer: bool,
) -> (
    DurableHostRuntime,
    std::path::PathBuf,
    Option<conduit_body::FederationPeer>,
) {
    use conduit_host_make::*;
    use conduit_std_host::StdHostConfig;
    let config = StdHostConfig {
        host_id: "host/active-carrier".into(),
        boot_id: "boot/active-carrier".into(),
        offer_generation: conduit_core::OfferGeneration(1),
    };
    let host = if image {
        let source = include_str!(
            "../../../../../../targets/std/profiles/linux-computer.host.conduit"
        )
        .replace("\"clock/monotonic\"", "\"timer/monotonic\"")
        .replace(
            "  limits =",
            r#"  capability = {kind: "time/tick", pinned_implementation: "std/kernel-time-tick@2"}
  need = {id: "timer/main", class: "conduit.resource/timer-slot@1", slots: 8, bytes: 512}
  limits ="#,
        );
        let mut configuration = parse_host_configuration_conduit(&source).unwrap();
        configuration.limits.heap_arena_bytes = 64 * 1024 * 1024;
        let catalog = conduit_workspace_make::catalog();
        let packages = conduit_workspace_make::package_set();
        let checked = check_host_configuration(configuration, &catalog, &packages).unwrap();
        let (image, _) = build_default_host_image(
            checked.profile().clone(),
            &catalog,
            &packages,
            &BuildInputs {
                source_identity: "fixture/active-carrier".into(),
                toolchain_available: true,
            },
        )
        .unwrap();
        StdHost::from_selected_timer_image(config, &image, &catalog).unwrap()
    } else {
        StdHost::new_with_config(config)
    };
    let root = std::env::temp_dir().join(crate::durable_host::fresh_identity(
        "member-carrier-active",
        "control",
    ));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("installation.json"),serde_json::to_vec(&serde_json::json!({"schema":"conduit.install/durable-host@1","host_id":"host/active-carrier","release_source_identity":"source/test","release_bundle_sha256":format!("sha256:{}","1".repeat(64)),"product_executable":"fixture-unused","body_state":null,"joined_body_state":null})).unwrap()).unwrap();
    let text = "plot active-carrier { clock: time/tick(count = 1, period-ms = 5000) }";
    let source = crate::plot_source::parse(text)
        .unwrap()
        .expand_entry_for_authoring()
        .unwrap();
    let resident = conduit_body::ResidentPlot::new(
        source.expanded.source_document_id.clone(),
        source.expanded.checked_plot_id.clone(),
    );
    let mut owner =
        crate::durable_host::owner::Owner::open(host, resident, None, "Carrier active fixture")
            .unwrap();
    owner.set_resident_plot_name(&source).unwrap();
    let peer = issuer.then(|| owner.prepare_current_member_carrier_receiver().unwrap());
    owner.persist(&root).unwrap();
    std::fs::write(root.join("body/source.conduit"), text).unwrap();
    let mut runtime =
        DurableHostRuntime::new("test/target".into(), "sha256:test".into(), StdHost::new());
    let worker = owner.start_service_run(&root, 2000).unwrap();
    runtime.host = HostSource::Body {
        owner: Box::new(owner),
        root: root.clone(),
        running: Some(super::super::super::body_run::OwnedRunWorker::General(
            worker,
        )),
    };
    let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        runtime.progress_owned_body().unwrap();
        if matches!(&runtime.host,HostSource::Body{owner,..} if owner.current_play_id().is_some()) {
            break;
        }
        assert!(
            std::time::Instant::now() < until,
            "actual Body worker did not Start"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    (runtime, root, peer)
}
fn finish_active(mut runtime: DurableHostRuntime, root: std::path::PathBuf) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while runtime.host.body_is_running() {
        runtime.progress_owned_body().unwrap();
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    drop(runtime);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn actual_running_image_receiver_reuses_issuer_and_carrier_calls_reach_original_authentication() {
    let carrier_requests = requests(9);
    let admission=serde_json::from_value(serde_json::json!({
        "schema":conduit_body::SPAWN_ADMISSION_REQUEST_SCHEMA,
        "invitation_id":"fixture/invitation", "body_id":"fixture/body",
        "host_advertisement":StdHost::new().advertisement(),"nonce":vec![0;32],"signature":vec![0;64],
        "membership_admitted":false,"plan_created":false,"play_created":false,
    })).unwrap();
    let (mut runtime, root, peer) = active_runtime(true, true);
    let before = runtime.owned_body_truth().unwrap()["biography"]["membership"].clone();
    let Response::BodyMemberCarrierReceiver { receiver, .. } = handle(
        Request::BodyMemberCarrierReceiver {
            protocol: PROTOCOL,
            token: vec![9; 32],
        },
        &[9; 32],
        &mut runtime,
    ) else {
        panic!("existing receiver must be readable during actual Play");
    };
    assert_eq!(*receiver, peer.unwrap());
    for mut request in carrier_requests.into_iter().filter(|r| {
        matches!(
            r,
            Request::BodyMemberCarrierBegin { .. } | Request::BodyMemberCarrierComplete { .. }
        )
    }) {
        if let Request::BodyMemberCarrierBegin {
            expires_at_millis, ..
        } = &mut request
        {
            *expires_at_millis = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64
                + 1000;
        }
        let Response::Refused { code, .. } = handle(request, &[9; 32], &mut runtime) else {
            panic!("invalid member must remain unauthenticated");
        };
        assert_ne!(code, "body-play-active");
        assert_ne!(
            code,
            "active member carrier requires original IMAGE custody"
        );
        assert!(
            code.contains("member") || code.contains("carrier"),
            "{code}"
        );
    }
    for request in [
        Request::BodyWake {
            protocol: PROTOCOL,
            token: vec![9; 32],
        },
        Request::BodyAdmit {
            protocol: PROTOCOL,
            token: vec![9; 32],
            request: Box::new(admission),
        },
        Request::BodyExportSeed {
            protocol: PROTOCOL,
            token: vec![9; 32],
            destination: root.join("forbidden.zip"),
        },
        Request::BodyInvite {
            protocol: PROTOCOL,
            token: vec![9; 32],
            ttl_seconds: 30,
            candidates: None,
        },
    ] {
        assert!(
            matches!(handle(request,&[9;32],&mut runtime),Response::Refused{code,..} if code=="body-play-active")
        );
    }
    assert!(!root.join("forbidden.zip").exists());
    assert_eq!(
        runtime.owned_body_truth().unwrap()["biography"]["membership"],
        before
    );
    finish_active(runtime, root);
}
#[test]
fn actual_running_reference_owner_never_initializes_issuer_or_enters_unfunded_carrier_lane() {
    let carrier_requests = requests(9);
    let (mut runtime, root, _) = active_runtime(false, false);
    assert!(
        matches!(handle(Request::BodyMemberCarrierReceiver{protocol:PROTOCOL,token:vec![9;32]},&[9;32],&mut runtime),Response::Refused{code,..} if code=="member carrier owner key absent")
    );
    for request in carrier_requests.into_iter().filter(|r| {
        matches!(
            r,
            Request::BodyMemberCarrierBegin { .. } | Request::BodyMemberCarrierComplete { .. }
        )
    }) {
        assert!(
            matches!(handle(request,&[9;32],&mut runtime),Response::Refused{code,..} if code=="active member carrier requires original IMAGE custody")
        );
    }
    let HostSource::Body { owner, .. } = &runtime.host else {
        unreachable!()
    };
    assert_eq!(
        owner.member_carrier_peer().unwrap_err(),
        "member carrier owner key absent"
    );
    finish_active(runtime, root);
}

#[test]
fn operator_wake_keeps_local_authentication_protocol_and_real_owner_requirements() {
    let mut runtime = DurableHostRuntime::new(
        "fixture/target".into(),
        "fixture/image".into(),
        StdHost::new(),
    );
    let before = runtime.truth().advertisement;
    for (protocol, token, expected) in [
        (PROTOCOL, 8, "unauthorized"),
        (PROTOCOL - 1, 9, "protocol"),
        (
            PROTOCOL,
            9,
            "installed Host does not own a live Body session",
        ),
    ] {
        let request = Request::BodyWake {
            protocol,
            token: vec![token; 32],
        };
        let bytes = serde_json::to_vec(&request).unwrap();
        let request = serde_json::from_slice(&bytes).unwrap();
        assert!(
            matches!(handle(request,&[9;32],&mut runtime), Response::Refused { code, .. } if code==expected)
        );
    }
    assert_eq!(runtime.truth().advertisement, before);
}
