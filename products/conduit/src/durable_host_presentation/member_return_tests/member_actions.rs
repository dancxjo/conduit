//! Real pinned TLS Resume, presence pump, asynchronous member action, protected
//! Unix IPC and installed owner dispatcher. Native offers/Shows are explicit
//! software contract fixtures, not physical display or human-input evidence.
use super::*;
use crate::durable_host_control as control;
use conduit_face::{FaceInteraction, MaskShow};
use conduit_std_host::{StdHost, StdHostConfig};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

struct Stop(Arc<AtomicBool>);
impl Drop for Stop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

fn actual_offer(host: &conduit_core::HostId) -> conduit_core::HostAdvertisement {
    use conduit_core::*;
    let mut provider = BaseProviderEntry {
        base_id: "fixture/framebuffer".into(),
        provider_instance_id: "fixture/framebuffer/1".into(),
        provider_generation: 1,
        implementation_id: "conduitos/framebuffer@1".into(),
        mechanism_family: "conduitos.base/framebuffer@1".into(),
        enforcement_class: BaseEnforcementClass::ConduitOsKernelEnforced,
        lifecycle: BaseLifecycle::Ready,
        capabilities: vec![],
        resources: vec![
            resource_offer("fixture/surface", conduit_face::SHOW_RESOURCE_CLASS, 1),
            resource_offer(
                "fixture/input",
                "conduit.resource/conduitos-human-input@1",
                1,
            ),
        ],
    };
    provider.resources.sort();
    let mut ad = conduit_conduitos_mask_offer::native_host_advertisement(
        host,
        &"boot/member-actions".into(),
        1,
        &provider,
    );
    provider.capabilities = ad
        .capabilities
        .iter()
        .filter(|cap| {
            [
                conduit_face::SHOW_RESOURCE_SOURCE_KIND,
                conduit_face::RESOURCE_RENDERER_KIND,
                conduit_face::FACE_INTERACTION_KIND,
            ]
            .contains(&cap.kind_id.as_str())
        })
        .cloned()
        .collect();
    ad.capabilities.retain(|cap| {
        cap.kind_id.as_str() == conduit_face::FACE_TEE_KIND || provider.capabilities.contains(cap)
    });
    ad.resources = provider.resources;
    ad.bases[0].capability_ids = provider
        .capabilities
        .iter()
        .map(|cap| cap.capability_id.clone())
        .collect();
    ad
}
fn shown(snapshot: &Snapshot, sequence: u64) -> MaskShow {
    let mask = &snapshot.route.planned_mask;
    let placement = mask.show_placement();
    MaskShow::prepared(
        mask,
        &snapshot.face,
        conduit_core::bind_active_play(
            &mask.plan.plan_id,
            &placement.host_id,
            &placement.boot_id,
            sequence,
        ),
        snapshot.face.subjects[0].identity.clone(),
        "fixture/native".into(),
        format!("sign/member-actions/prepared/{sequence}").into(),
    )
    .unwrap()
    .transition(
        conduit_face::ManifestationLifecycle::Available,
        format!("sign/member-actions/available/{sequence}").into(),
    )
    .unwrap()
}
fn action(snapshot: &Snapshot, shown: &MaskShow, intent: &str, sequence: u64) -> FaceInteraction {
    let offered = snapshot
        .face
        .actions
        .iter()
        .find(|action| action.intent == intent)
        .unwrap();
    assert!(offered.availability.is_available());
    FaceInteraction::new(
        &snapshot.face,
        shown,
        &offered.identity,
        &offered.target,
        vec![],
        sequence,
    )
    .unwrap()
}
fn wait_action(route: &member_carrier_client::MemberRoute) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        if let Some(result) = route.poll_action()? {
            return result;
        }
        if Instant::now() >= deadline {
            return Err("fixture action result timed out".into());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn current_member_transport_accepts_visible_library_navigation_and_refuses_old_show() {
    let (owner, owner_state) = installed::owner::member_return_owner_fixture();
    let owner_id = owner.host.advertisement().host_id.clone();
    drop(owner);
    control::ensure_secret(&owner_state).unwrap();
    let runtime = control::DurableHostRuntime::new(
        "std/x86_64/computer".into(),
        "fixture/image".into(),
        StdHost::new_with_config(StdHostConfig {
            host_id: owner_id,
            boot_id: "boot/member-actions-owner".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        }),
    )
    .with_owned_body(&owner_state)
    .unwrap();
    let done = Arc::new(AtomicBool::new(false));
    let client_done = done.clone();
    let client_owner_state = owner_state.clone();
    let client = std::thread::spawn(move || {
        let _stop = Stop(client_done);
        run_client_and_receiver(&client_owner_state);
    });
    let deadline = Instant::now() + Duration::from_secs(25);
    let mut poll = |_: &mut control::DurableHostRuntime| {
        if done.load(Ordering::Acquire) || Instant::now() >= deadline {
            Err("member action fixture stopped".into())
        } else {
            Ok(())
        }
    };
    let result = control::serve_with_foreground(&owner_state, runtime, &mut poll);
    let joined = client.join();
    assert_eq!(result.unwrap_err(), "member action fixture stopped");
    joined.unwrap();
    fs::remove_dir_all(owner_state).unwrap();
}

fn run_client_and_receiver(owner_state: &Path) {
    let ready = Instant::now() + Duration::from_secs(5);
    while !owner_state.join("control.sock").exists() {
        assert!(Instant::now() < ready);
        std::thread::sleep(Duration::from_millis(5));
    }
    let (manifest, state) = installed::tests::fixture();
    let installation = installed::install_without_start(&manifest, &state).unwrap();
    let ad = actual_offer(&installation.host_id.as_str().into());
    let invitation = control::issue_owned_invitation(owner_state, 60, None).unwrap();
    let key = conduit_body::SpawnInvitationSecret::from_csprng_bytes(invitation.secret).unwrap();
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
    let receipt = control::admit_owned_request(owner_state, request).unwrap();
    installed::membership::complete_body_join_document(receipt.clone(), &state, true).unwrap();
    let before = control::inspect_owned_body(owner_state).unwrap();
    assert!(before["realization"].is_null());
    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(vec!["127.0.0.1".into()]).unwrap();
    let certificate = state.join("tls-cert.pem");
    let private_key = state.join("tls-key.pem");
    fs::write(&certificate, cert.pem()).unwrap();
    fs::write(&private_key, signing_key.serialize_pem()).unwrap();
    let reserved = std::net::TcpListener::bind("0.0.0.0:0").unwrap();
    let bind = reserved.local_addr().unwrap();
    drop(reserved);
    let listener =
        SecureWebSocketListener::bind(bind, &certificate, &private_key, ADMISSION as u32, true)
            .unwrap();
    let encounter = installed::routed_join::MemberEncounter {
        schema: installed::routed_join::MEMBER_ROUTE_SCHEMA.into(),
        body_id: receipt.credential.body_id.clone(),
        part_id: receipt.credential.part_id.clone(),
        host_id: receipt.credential.host_id.clone(),
        owner: control::carrier_receiver(owner_state).unwrap(),
        candidate: RendezvousCandidate {
            candidate_id: "candidate/member-action-fixture".into(),
            line_family: RendezvousLineFamily::AuthenticatedTlsStream,
            reachability: format!("wss://127.0.0.1:{}/member", bind.port()),
            authentication: RendezvousAuthentication {
                server_identity: "127.0.0.1".into(),
                transport_binding_sha256: listener.certificate_binding_sha256(),
            },
            expires_at_millis: installed::current_time_millis().unwrap() + 30_000,
            maximum_attempts: 1,
            attempt_timeout_millis: 3_000,
        },
    };
    let server_encounter = encounter.clone();
    let server_state = owner_state.to_path_buf();
    let server = std::thread::spawn(move || {
        let mut line = listener
            .accept_with_timeout(Duration::from_secs(5))
            .unwrap();
        let frame = server_receive(&mut line);
        let (member, session) = installed::routed_join::current_member::serve_with(
            &server_encounter,
            &mut line,
            frame,
            |credential, ad, label, expiry| {
                control::begin_carrier(&server_state, credential, ad, label, expiry, 128)
            },
            |challenge, signature| control::complete_carrier(&server_state, challenge, signature),
            |session| control::retire_carrier(&server_state, session),
        )
        .unwrap();
        // Actual production branch, including PendingAction::begin_member and
        // the protected IPC actor; no custom Face/action dispatcher is injected.
        let result =
            installed::owner::serve_member_presence(&server_state, line, session.clone(), &member);
        let retired = control::retire_carrier(&server_state, session);
        result.and(retired)
    });
    let (mut route, initial) = member_carrier_client::MemberRoute::connect_current(
        &state,
        &receipt.credential,
        &ad,
        &encounter,
    )
    .unwrap();
    assert!(initial.interactions_admitted);
    assert!(initial.route.interaction_line.is_some());
    let first = shown(&initial, 1);
    route.acknowledge(&first).unwrap();
    let library = action(&initial, &first, "conduit.intent/open-plot-library@1", 1);
    route.begin_action(&first, library.clone()).unwrap();
    assert!(
        route.begin_action(&first, library.clone()).is_err(),
        "one action worker, no duplicate admission while pending"
    );
    wait_action(&route).unwrap();
    // A separately transmitted stale fixture occurrence must not navigate twice.
    route.begin_action(&first, library).unwrap();
    assert!(wait_action(&route).is_err());
    let next = route.refresh().unwrap();
    assert!(next.face.revision > initial.face.revision);
    assert_ne!(next.face.identity, initial.face.identity);
    assert_context(&next.face, "library");
    assert_visible_change(&initial.face, &next.face);
    let second = shown(&next, 2);
    route.acknowledge(&second).unwrap();
    let overview = action(&next, &second, "conduit.intent/open-body-overview@1", 2);
    assert_ne!(overview.action_id, "");
    route.begin_action(&second, overview).unwrap();
    wait_action(&route).unwrap();
    let returned = route.refresh().unwrap();
    assert_context(&returned.face, "overview");
    assert_visible_change(&next.face, &returned.face);
    let after = control::inspect_owned_body(owner_state).unwrap();
    assert_eq!(before["biography"], after["biography"]);
    assert_eq!(before["realization"], after["realization"]);
    assert_eq!(before["last_execution"], after["last_execution"]);
    route.leave().unwrap();
    server.join().unwrap().unwrap();
    fs::remove_dir_all(state).unwrap();
}
fn assert_context(face: &conduit_face::Face, context: &str) {
    assert!(face
        .properties
        .iter()
        .any(|property| property.name == "operator-context"
            && property.value == conduit_face::FacePropertyValue::Text(context.into())));
}
fn assert_visible_change(left: &conduit_face::Face, right: &conduit_face::Face) {
    use conduit_face::{
        admit_visual_expression, VisualDensity, VisualExpression, VisualUserConstraints,
    };
    let mut visual = VisualExpression::phosphor();
    visual.density = VisualDensity::Spacious;
    let visual = admit_visual_expression(
        Ok(visual),
        VisualUserConstraints {
            forced_colors: false,
            reduced_motion: true,
            minimum_text_scale_percent: 150,
            maximum_supported_text_scale_percent: 200,
        },
    )
    .unwrap();
    let a = conduitos::native_face_scene::NativeFaceScene::prepare_visual_fallback(
        left.clone(),
        1024,
        600,
        true,
        &visual,
    )
    .unwrap();
    let b = conduitos::native_face_scene::NativeFaceScene::prepare_visual_fallback(
        right.clone(),
        1024,
        600,
        true,
        &visual,
    )
    .unwrap();
    assert_ne!(
        a.frame().unwrap().scene.commands(),
        b.frame().unwrap().scene.commands(),
        "accepted context metadata alone is not a visible page transition"
    );
}
