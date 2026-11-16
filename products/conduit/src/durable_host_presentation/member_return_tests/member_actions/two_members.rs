//! Two actual local pinned TLS carriers through the installed control actor.
//! Native offers/Shows are software contract fixtures, not physical SDL evidence.
//! Fresh state only; this test never discovers or contacts a running Body.
use super::*;
use conduit_body::MembershipCredential;
use conduit_core::{HostAdvertisement, HostId, LinkBindingId, OfferGeneration};
use conduit_face::OwnerFaceSnapshotRequest;
use std::{path::PathBuf, sync::mpsc, thread::JoinHandle};

fn image_host(host_id: HostId) -> StdHost {
    let source = r#"host transport-viewer-owner {
  schema = 1
  target = {architecture: "x86_64", machine: "computer", os: "linux"}
  base = {kind: "timer/monotonic", implementation: "hosted/monotonic-clock@1"}
  capability = {kind: "time/tick", pinned_implementation: "std/kernel-time-tick@2"}
  need = {id: "timer/main", class: "conduit.resource/timer-slot@1", slots: 1, bytes: 512}
  limits = {static_memory_bytes: 16777216, heap_arena_bytes: 67108864, queue_items: 4096, buffered_bytes: 16777216, active_instances: 512, operation_slots: 256, timer_slots: 128, line_sessions: 64, evidence_items: 4096}
}
"#;
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let checked = conduit_host_make::check_host_configuration(
        conduit_host_make::parse_host_configuration_conduit(source).unwrap(),
        &catalog,
        &packages,
    )
    .unwrap();
    let (image, _) = conduit_host_make::build_default_host_image(
        checked.profile().clone(),
        &catalog,
        &packages,
        &conduit_host_make::BuildInputs {
            source_identity: "fixture/two-member-live-transport".into(),
            toolchain_available: true,
        },
    )
    .unwrap();
    StdHost::from_selected_timer_image(
        StdHostConfig {
            host_id,
            boot_id: "boot/two-member-transport-owner".into(),
            offer_generation: OfferGeneration(1),
        },
        &image,
        &catalog,
    )
    .unwrap()
}
struct Guest {
    state: PathBuf,
    ad: HostAdvertisement,
    credential: MembershipCredential,
}
impl Guest {
    fn admit(owner: &Path) -> Self {
        let (manifest, state) = installed::tests::fixture();
        let installation = installed::install_without_start(&manifest, &state).unwrap();
        let mut ad = actual_offer(&installation.host_id.as_str().into());
        ad.boot_id = format!("boot/two-member/{}", ad.host_id.as_str()).into();
        let invitation = control::issue_owned_invitation(owner, 60, None).unwrap();
        let key =
            conduit_body::SpawnInvitationSecret::from_csprng_bytes(invitation.secret).unwrap();
        let request = PortableSpawnAdmissionRequest {
            schema: conduit_body::SPAWN_ADMISSION_REQUEST_SCHEMA.into(),
            invitation_id: invitation.claim.invitation_id.clone(),
            body_id: invitation.claim.body_id.clone(),
            host_advertisement: ad.clone(),
            nonce: invitation.claim.nonce,
            signature: key
                .sign(&invitation.claim.signing_transcript(
                    &ad.host_id,
                    &ad.boot_id,
                    ad.offer_generation,
                ))
                .to_vec(),
            membership_admitted: false,
            plan_created: false,
            play_created: false,
        };
        fs::create_dir_all(state.join("body")).unwrap();
        installed::restrict_directory(&state.join("body")).unwrap();
        installed::write_json_atomic(
            &state.join("body/pending-join.json"),
            &installed::invitation::PendingBodyJoin {
                schema: "conduit.body/pending-join@1".into(),
                invitation,
                request: request.clone(),
            },
        )
        .unwrap();
        let receipt = control::admit_owned_request(owner, request).unwrap();
        installed::membership::complete_body_join_document(receipt.clone(), &state, true).unwrap();
        Self {
            state,
            ad,
            credential: receipt.credential,
        }
    }
    fn request(&self) -> OwnerFaceSnapshotRequest {
        OwnerFaceSnapshotRequest {
            schema: conduit_face::OWNER_FACE_REQUEST_SCHEMA.into(),
            credential_id: self.credential.credential_id.as_str().into(),
            body_id: self.credential.body_id.clone(),
            part_id: self.credential.part_id.clone(),
            host_id: self.credential.host_id.clone(),
            boot_id: self.credential.boot_id.clone(),
            last_seen_revision: None,
            last_seen_identity: None,
        }
    }
}
struct Connected {
    route: member_carrier_client::MemberRoute,
    first: Snapshot,
    session: LinkBindingId,
    server: JoinHandle<Result<(), String>>,
}
fn connect(owner: &Path, guest: &Guest) -> Connected {
    use std::os::unix::fs::PermissionsExt;
    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(vec!["127.0.0.1".into()]).unwrap();
    let certificate = guest.state.join("tls-cert.pem");
    let private_key = guest.state.join("tls-key.pem");
    fs::write(&certificate, cert.pem()).unwrap();
    fs::write(&private_key, signing_key.serialize_pem()).unwrap();
    fs::set_permissions(&private_key, fs::Permissions::from_mode(0o600)).unwrap();
    // The production listener requires an explicitly authorized non-loopback,
    // nonzero binding. Follow the existing pinned-TLS fixture's supported
    // entrance; the only client route below is loopback and pins this new cert.
    let reserved = std::net::TcpListener::bind("0.0.0.0:0").unwrap();
    let bind = reserved.local_addr().unwrap();
    drop(reserved);
    let listener =
        SecureWebSocketListener::bind(bind, &certificate, &private_key, ADMISSION as u32, true)
            .unwrap();
    let address = listener.local_addr().unwrap();
    let encounter = installed::routed_join::MemberEncounter {
        schema: installed::routed_join::MEMBER_ROUTE_SCHEMA.into(),
        body_id: guest.credential.body_id.clone(),
        part_id: guest.credential.part_id.clone(),
        host_id: guest.credential.host_id.clone(),
        owner: control::carrier_receiver(owner).unwrap(),
        candidate: RendezvousCandidate {
            candidate_id: format!("candidate/two-member/{}", guest.ad.host_id.as_str()),
            line_family: RendezvousLineFamily::AuthenticatedTlsStream,
            reachability: format!("wss://127.0.0.1:{}/member", address.port()),
            authentication: RendezvousAuthentication {
                server_identity: "127.0.0.1".into(),
                transport_binding_sha256: listener.certificate_binding_sha256(),
            },
            expires_at_millis: installed::current_time_millis().unwrap() + 60_000,
            maximum_attempts: 1,
            attempt_timeout_millis: 3_000,
        },
    };
    let server_encounter = encounter.clone();
    let state = owner.to_path_buf();
    let (send, receive) = mpsc::sync_channel(1);
    let server = std::thread::spawn(move || {
        let mut line = listener
            .accept_with_timeout(Duration::from_secs(5))
            .map_err(|e| format!("fixture TLS accept:{e:?}"))?;
        let frame = server_receive(&mut line);
        let (member, session) = installed::routed_join::current_member::serve_with(
            &server_encounter,
            &mut line,
            frame,
            |credential, ad, label, expiry| {
                control::begin_carrier(&state, credential, ad, label, expiry, 128)
            },
            |challenge, signature| control::complete_carrier(&state, challenge, signature),
            |session| control::retire_carrier(&state, session),
        )?;
        send.send(session.clone())
            .map_err(|_| "fixture session receiver gone".to_string())?;
        let result =
            installed::owner::serve_member_presence(&state, line, session.clone(), &member);
        let retired = control::retire_carrier(&state, session);
        result.and(retired)
    });
    let (route, first) = member_carrier_client::MemberRoute::connect_current(
        &guest.state,
        &guest.credential,
        &guest.ad,
        &encounter,
    )
    .unwrap();
    let session = receive.recv_timeout(Duration::from_secs(3)).unwrap();
    assert!(first.interactions_admitted && first.route.interaction_line.is_some());
    Connected {
        route,
        first,
        session,
        server,
    }
}
fn navigate(
    route: &member_carrier_client::MemberRoute,
    snapshot: &Snapshot,
    show: &MaskShow,
    intent: &str,
    sequence: u64,
) {
    route
        .begin_action(show, action(snapshot, show, intent, sequence))
        .unwrap();
    wait_action(route).unwrap();
}

#[test]
fn two_selected_image_members_keep_separate_shows_actions_and_retirement_over_pinned_tls() {
    let (owner, state) = installed::owner::member_return_owner_fixture();
    let host_id = owner.host.advertisement().host_id.clone();
    drop(owner);
    control::ensure_secret(&state).unwrap();
    let runtime = control::DurableHostRuntime::new(
        "std/x86_64/computer".into(),
        "fixture/two-member-image".into(),
        image_host(host_id),
    )
    .with_owned_body(&state)
    .unwrap();
    let done = Arc::new(AtomicBool::new(false));
    let client_done = done.clone();
    let client_state = state.clone();
    let client = std::thread::spawn(move || {
        let _stop = Stop(client_done);
        run_two(&client_state);
    });
    let deadline = Instant::now() + Duration::from_secs(75);
    let mut poll = |_: &mut control::DurableHostRuntime| {
        if done.load(Ordering::Acquire) || Instant::now() >= deadline {
            Err("two member transport fixture stopped".into())
        } else {
            Ok(())
        }
    };
    let result = control::serve_with_foreground(&state, runtime, &mut poll);
    let joined = client.join();
    assert_eq!(result.unwrap_err(), "two member transport fixture stopped");
    joined.unwrap();
    fs::remove_dir_all(state).unwrap();
}
fn run_two(owner: &Path) {
    let ready = Instant::now() + Duration::from_secs(5);
    while !owner.join("control.sock").exists() {
        assert!(Instant::now() < ready);
        std::thread::sleep(Duration::from_millis(5));
    }
    // Complete both ordinary admissions first; joining B must not be mistaken
    // for a same-epoch presentation failure on A.
    let a = Guest::admit(owner);
    let b = Guest::admit(owner);
    assert_ne!(a.credential.part_id, b.credential.part_id);
    let before = control::inspect_owned_body(owner).unwrap();
    assert!(before["realization"].is_null());
    let mut ca = connect(owner, &a);
    let mut cb = connect(owner, &b);
    assert_ne!(ca.session, cb.session);
    assert_eq!(
        ca.first.face.identity, cb.first.face.identity,
        "same Body Face, two Masks"
    );
    let a1 = shown(&ca.first, 1);
    let b1 = shown(&cb.first, 1);
    assert_ne!(a1.show_id, b1.show_id);
    ca.route.acknowledge(&a1).unwrap();
    cb.route.acknowledge(&b1).unwrap();
    // The protected actor must independently refuse B's Show on A's actual
    // authenticated session; client-side Show validation is not this proof.
    assert!(control::carrier_show(owner, ca.session.clone(), a.request(), b1).is_err());
    navigate(
        &ca.route,
        &ca.first,
        &a1,
        "conduit.intent/open-plot-library@1",
        1,
    );
    let stale = action(&ca.first, &a1, "conduit.intent/open-plot-library@1", 2);
    ca.route.begin_action(&a1, stale).unwrap();
    assert!(wait_action(&ca.route).is_err());
    let b2 = cb.route.refresh().unwrap();
    assert_context(&b2.face, "library");
    assert_visible_change(&cb.first.face, &b2.face);
    let bs2 = shown(&b2, 2);
    cb.route.acknowledge(&bs2).unwrap();
    navigate(
        &cb.route,
        &b2,
        &bs2,
        "conduit.intent/open-body-overview@1",
        3,
    );
    let a3 = ca.route.refresh().unwrap();
    let b3 = cb.route.refresh().unwrap();
    assert_context(&a3.face, "overview");
    assert_eq!(a3.face.identity, b3.face.identity);
    let as3 = shown(&a3, 3);
    let bs3 = shown(&b3, 3);
    ca.route.acknowledge(&as3).unwrap();
    cb.route.acknowledge(&bs3).unwrap();
    // B's acknowledgement must not replace A's actual current Show custody.
    navigate(
        &ca.route,
        &a3,
        &as3,
        "conduit.intent/open-plot-library@1",
        4,
    );
    let b4 = cb.route.refresh().unwrap();
    assert_context(&b4.face, "library");
    let bs4 = shown(&b4, 4);
    cb.route.acknowledge(&bs4).unwrap();
    ca.route.leave().unwrap();
    ca.server.join().unwrap().unwrap();
    assert!(control::carrier_show(owner, ca.session, a.request(), as3).is_err());
    // Retiring A does not retire B's presence, Show, return Line or action Line.
    navigate(
        &cb.route,
        &b4,
        &bs4,
        "conduit.intent/open-body-overview@1",
        5,
    );
    let final_b = cb.route.refresh().unwrap();
    assert_context(&final_b.face, "overview");
    assert_visible_change(&b4.face, &final_b.face);
    let after = control::inspect_owned_body(owner).unwrap();
    for key in ["biography", "realization", "last_execution"] {
        assert_eq!(before[key], after[key], "presentation cannot mutate {key}");
    }
    cb.route.leave().unwrap();
    cb.server.join().unwrap().unwrap();
    fs::remove_dir_all(a.state).unwrap();
    fs::remove_dir_all(b.state).unwrap();
}
