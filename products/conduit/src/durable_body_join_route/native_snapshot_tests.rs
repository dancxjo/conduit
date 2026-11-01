use super::*;
use crate::durable_host::{owner::Owner, RetainedPart};
use crate::host_rendezvous::{native_snapshot, native_snapshot_lines, NativeReturn};
use conduit_body::*;
use conduit_core::OfferGeneration;
use conduit_std_host::{StdHost, StdHostConfig};
use std::{
    fs,
    path::PathBuf,
    process::{Child, Command, Stdio},
};

fn host(id: &str, boot: &str, generation: u64) -> StdHost {
    StdHost::new_with_config(StdHostConfig {
        host_id: id.into(),
        boot_id: boot.into(),
        offer_generation: OfferGeneration(generation),
    })
}
struct Service {
    root: PathBuf,
    child: Child,
}
impl Service {
    fn spawn(root: PathBuf, role: &str) -> Self {
        control::ensure_secret(&root).unwrap();
        let mut child=Command::new(std::env::current_exe().unwrap()).args(["--exact","durable_host::routed_join::native_snapshot_encounter::tests::encounter_control_child","--nocapture"])
            .env("CONDUIT_NATIVE_SNAPSHOT_TEST_ROOT",&root).env("CONDUIT_NATIVE_SNAPSHOT_TEST_ROLE",role)
            .stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
        let end = Instant::now() + Duration::from_secs(5);
        loop {
            if root.join("control.sock").exists() && control::current(&root).is_ok() {
                break;
            }
            assert!(
                child.try_wait().unwrap().is_none(),
                "real owner control exited"
            );
            assert!(
                Instant::now() < end,
                "real owner control readiness timed out"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        Self { root, child }
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.root);
    }
}
#[test]
fn encounter_control_child() {
    let Some(root) = std::env::var_os("CONDUIT_NATIVE_SNAPSHOT_TEST_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    let role = std::env::var("CONDUIT_NATIVE_SNAPSHOT_TEST_ROLE").unwrap();
    let h = if role == "owner" {
        host("host/snapshot-owner", "boot/snapshot-owner-service", 1)
    } else {
        host("host/snapshot-client", "boot/snapshot-client-returned", 2)
    };
    let mut runtime =
        control::DurableHostRuntime::new("test/native-snapshot".into(), "00".repeat(32), h);
    if role == "owner" {
        runtime = runtime.with_owned_body(&root).unwrap();
    }
    control::serve(&root, runtime).unwrap();
}
struct Fixture {
    owner: Service,
    client_control: Service,
    client: Option<NativeReturn>,
    route: conduit_body::RendezvousCandidate,
    selected: SessionBinding,
    part: PartId,
}
fn fixture() -> Fixture {
    let root = std::env::temp_dir().join(format!(
        "native-snapshot-{}",
        crate::birth_identity::fresh_uuid().unwrap()
    ));
    fs::create_dir(&root).unwrap();
    let installation = crate::durable_host::Installation {
        schema: crate::durable_host::INSTALL_SCHEMA.into(),
        host_id: "host/snapshot-owner".into(),
        release_source_identity: "source/test".into(),
        release_bundle_sha256: crate::durable_host::digest(b"test-bundle"),
        product_executable: "fixture-unused".into(),
        body_state: None,
        joined_body_state: None,
        selected_speech: None,
        selected_model: None,
        selected_todo_checkpoint: None,
        selected_host_configuration: None,
    };
    crate::durable_host::write_json_atomic(&root.join("installation.json"), &installation).unwrap();
    let plot = crate::plot_source::parse(
        "plot hello {\n show: face/text(\"Message\")\n \"Hello.\" >> show\n}.",
    )
    .unwrap()
    .expand_entry_for_authoring()
    .unwrap();
    let resident = ResidentPlot::new(
        plot.expanded.source_document_id.clone(),
        plot.expanded.checked_plot_id.clone(),
    );
    let mut owner = Owner::open(
        host("host/snapshot-owner", "boot/snapshot-owner-initial", 1),
        resident,
        None,
        "Snapshot Body",
    )
    .unwrap();
    owner.persist(&root).unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let route = RendezvousCandidate {
        candidate_id: "route/native-snapshot".into(),
        line_family: RendezvousLineFamily::AuthenticatedConduitLine,
        reachability: "wss://reviewed-test-relay.invalid:443/route".into(),
        authentication: RendezvousAuthentication {
            server_identity: "reviewed-test-relay.invalid".into(),
            transport_binding_sha256: [8; 32],
        },
        expires_at_millis: now + 60_000,
        maximum_attempts: 1,
        attempt_timeout_millis: 2000,
    };
    let invitation = owner
        .issue_invitation(&root, 60, Some(vec![route.clone()]))
        .unwrap();
    let public = ed25519_dalek::SigningKey::from_bytes(&invitation.secret)
        .verifying_key()
        .to_bytes();
    let old = host("host/snapshot-client", "boot/snapshot-client-initial", 1)
        .advertisement()
        .clone();
    let window = owner
        .browser_authorize_window(old.host_id.as_str(), Some(public), 10_000)
        .unwrap();
    owner
        .browser_begin(
            &window.window_id,
            &"line/initial-test-admission".into(),
            In::InvitedAdvertise {
                protocol: PROTOCOL,
                advertisement: old.clone(),
                claim: invitation.claim.clone(),
                verifying_key: public.to_vec(),
            },
            512,
        )
        .unwrap();
    let signature = SpawnInvitationSecret::from_csprng_bytes(invitation.secret)
        .unwrap()
        .sign(&invitation.claim.signing_transcript(
            &old.host_id,
            &old.boot_id,
            old.offer_generation,
        ));
    let snapshot = owner
        .browser_complete(
            &root,
            &window.window_id,
            In::SpawnProof {
                protocol: PROTOCOL,
                invitation_id: invitation.claim.invitation_id.clone(),
                body_id: invitation.claim.body_id.clone(),
                host_id: old.host_id.clone(),
                boot_id: old.boot_id.clone(),
                nonce: invitation.claim.nonce.to_vec(),
                signature: signature.to_vec(),
            },
        )
        .unwrap();
    let credential = snapshot.credential;
    owner
        .browser_leave(&root, &window.window_id, &credential)
        .unwrap();
    owner
        .browser_cancel_window(&root, &window.window_id)
        .unwrap();
    owner.persist(&root).unwrap();
    drop(owner);
    let doc = serde_json::json!({"schema":"conduit.native/part-custody@1","pending":{"schema":"conduit.body/pending-join@1","invitation":invitation,"request":{"schema":SPAWN_ADMISSION_REQUEST_SCHEMA,"invitation_id":invitation.claim.invitation_id,"body_id":invitation.claim.body_id,"host_advertisement":old,"nonce":invitation.claim.nonce,"signature":signature.to_vec(),"membership_admitted":false,"plan_created":false,"play_created":false}},"receipt":{"schema":SPAWN_ADMISSION_RECEIPT_SCHEMA,"credential":credential,"host_advertisement":old,"current_offers_available":!old.capabilities.is_empty(),"membership_admitted":true,"plan_created":false,"play_created":false}});
    let owner = Service::spawn(root, "owner");
    let client_root = std::env::temp_dir().join(format!(
        "snapshot-client-{}",
        crate::birth_identity::fresh_uuid().unwrap()
    ));
    fs::create_dir(&client_root).unwrap();
    let client_control = Service::spawn(client_root, "client");
    let ad = control::current(&client_control.root)
        .unwrap()
        .advertisement;
    // Exercise the actual installed private custody entrance, not a test-only
    // protocol constructor: original signer/receipt and public binding agree.
    let body_dir = client_control.root.join("body");
    fs::create_dir(&body_dir).unwrap();
    crate::durable_host::restrict_directory(&body_dir).unwrap();
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut private = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(body_dir.join("native-part-private.json"))
        .unwrap();
    private
        .write_all(&serde_json::to_vec(&doc).unwrap())
        .unwrap();
    private.sync_all().unwrap();
    let credential_path = body_dir.join("membership-credential.json");
    crate::durable_host::write_json_atomic(&credential_path, &credential).unwrap();
    let mut client_installation = installation.clone();
    client_installation.host_id = ad.host_id.as_str().into();
    client_installation.joined_body_state =
        Some(crate::durable_host::membership::JoinedBodyBinding {
            body_id: credential.body_id.as_str().into(),
            part_id: credential.part_id.as_str().into(),
            credential_path: credential_path.display().to_string(),
            credential_sha256: crate::durable_host::digest(
                &serde_json::to_vec_pretty(&credential).unwrap(),
            ),
        });
    crate::durable_host::write_json_atomic(
        &client_control.root.join("installation.json"),
        &client_installation,
    )
    .unwrap();
    let retained = RetainedPart::open(&client_control.root, ad).unwrap();
    let owner_ad = control::current(&owner.root).unwrap().advertisement;
    let selected = SessionBinding {
        initiator: conduit_protected_line::EndpointBinding {
            host_id: credential.host_id.as_str().into(),
            boot_id: "boot/snapshot-client-returned".into(),
        },
        responder: conduit_protected_line::EndpointBinding {
            host_id: owner_ad.host_id.as_str().into(),
            boot_id: owner_ad.boot_id.as_str().into(),
        },
        negotiation_id: "negotiation/native-snapshot".into(),
        line_session_id: "session/native-snapshot".into(),
        candidate_binding: route.candidate_id.clone(),
        transport_binding: "selected-test-noise-channel".into(),
    };
    Fixture {
        owner,
        client_control,
        client: Some(NativeReturn::new(retained).unwrap()),
        route,
        selected,
        part: credential.part_id,
    }
}
fn verify_offline(f: &Fixture) {
    let truth = control::inspect_owned_body(&f.owner.root).unwrap();
    let parts = truth["biography"]["membership"]["parts"]
        .as_array()
        .unwrap();
    let found = parts
        .iter()
        .find(|p| p["part_id"] == serde_json::json!(f.part))
        .expect("original Part retained");
    assert!(
        found["current"].is_null(),
        "encounter must retire reachability without deleting Part"
    );
}
#[test]
fn protected_existing_part_returns_and_reads_exact_body_then_retires() {
    let mut f = fixture();
    let verified = control::native_snapshot::start(
        &f.owner.root,
        "host/snapshot-client",
        10_000,
        &f.selected,
        &f.route,
    )
    .unwrap();
    let (client, server) = native_snapshot_lines(f.selected.clone(), 2 * 1024 * 1024);
    std::thread::scope(|scope| {
        let worker = scope.spawn(|| serve(&f.owner.root, &verified, server));
        let (credential, response) =
            native_snapshot(f.client.take().unwrap(), &verified, client).unwrap();
        assert_eq!(credential.part_id, f.part);
        let OwnerFaceSnapshotResponse::Snapshot {
            schema,
            face,
            interactions_admitted,
            route,
        } = response
        else {
            panic!("expected actual canonical snapshot");
        };
        assert_eq!(schema, conduit_face::OWNER_FACE_RESPONSE_SCHEMA);
        assert_eq!(
            face.basis.body_id.as_ref(),
            Some(&verified.authorization().body_id)
        );
        assert!(!interactions_admitted);
        assert!(route.is_none());
        let evidence = worker.join().unwrap().unwrap();
        assert_eq!(evidence.session, f.selected);
        assert_eq!(evidence.window_id, verified.authorization().window_id);
        assert_eq!(evidence.credential_id, credential.credential_id.as_str());
    });
    verify_offline(&f);
}
#[test]
fn actual_wrong_session_and_expired_window_cannot_renew_admission() {
    for expired in [false, true] {
        let mut f = fixture();
        let duration = if expired { 1000 } else { 10_000 };
        let verified = control::native_snapshot::start(
            &f.owner.root,
            "host/snapshot-client",
            duration,
            &f.selected,
            &f.route,
        )
        .unwrap();
        let mut actual = f.selected.clone();
        if expired {
            std::thread::sleep(Duration::from_millis(1100));
        } else {
            actual.responder.boot_id = "boot/wrong-real-session".into();
        }
        let (client, server) = native_snapshot_lines(actual, 2 * 1024 * 1024);
        std::thread::scope(|scope| {
            let worker = scope.spawn(|| serve(&f.owner.root, &verified, server));
            assert!(native_snapshot(f.client.take().unwrap(), &verified, client).is_err());
            assert!(worker.join().unwrap().is_err());
        });
        verify_offline(&f);
    }
}

fn atomic_return(
    f: &mut Fixture,
    verified: &control::native_snapshot::VerifiedSelection,
) -> (conduit_body::MembershipCredential, LinkBindingId) {
    let client = f.client.as_mut().unwrap();
    let first = client.advertise().unwrap();
    let binding = LinkBindingId::from(format!(
        "binding/native-protected-snapshot/{}",
        f.selected.line_session_id
    ));
    let challenge = browser::begin(
        &f.owner.root,
        &verified.authorization().window_id,
        binding.clone(),
        serde_json::from_slice(&first).unwrap(),
        first.len() as u32,
    )
    .unwrap();
    let proof = client
        .challenge(&serde_json::to_vec(&challenge).unwrap())
        .unwrap();
    let BrowserAdmissionCompletion::Active { snapshot } = browser::complete(
        &f.owner.root,
        &verified.authorization().window_id,
        serde_json::from_slice(&proof).unwrap(),
    )
    .unwrap() else {
        panic!("expected authenticated active Return")
    };
    (snapshot.credential, binding)
}
fn request(c: &MembershipCredential) -> conduit_face::OwnerFaceSnapshotRequest {
    conduit_face::OwnerFaceSnapshotRequest {
        schema: conduit_face::OWNER_FACE_REQUEST_SCHEMA.into(),
        credential_id: c.credential_id.as_str().into(),
        body_id: c.body_id.clone(),
        part_id: c.part_id.clone(),
        host_id: c.host_id.clone(),
        boot_id: c.boot_id.clone(),
        last_seen_revision: None,
        last_seen_identity: None,
    }
}
#[test]
fn atomic_face_refuses_wrong_binding_and_original_deadline_after_return() {
    let mut f = fixture();
    let verified = control::native_snapshot::start(
        &f.owner.root,
        "host/snapshot-client",
        1000,
        &f.selected,
        &f.route,
    )
    .unwrap();
    let (credential, binding) = atomic_return(&mut f, &verified);
    assert!(control::native_snapshot::face(
        &f.owner.root,
        &verified.authorization().window_id,
        "binding/wrong-actual-carrier".into(),
        request(&credential)
    )
    .is_err());
    assert!(control::native_snapshot::face(
        &f.owner.root,
        &verified.authorization().window_id,
        binding.clone(),
        request(&credential)
    )
    .is_ok());
    std::thread::sleep(Duration::from_millis(1100));
    assert!(control::native_snapshot::face(
        &f.owner.root,
        &verified.authorization().window_id,
        binding,
        request(&credential)
    )
    .is_err());
    browser::leave(
        &f.owner.root,
        &verified.authorization().window_id,
        credential,
    )
    .unwrap();
    browser::cancel(&f.owner.root, &verified.authorization().window_id).unwrap();
    verify_offline(&f);
}
#[test]
fn actual_small_document_workspace_refuses_without_new_part_or_reachability() {
    let mut f = fixture();
    let verified = control::native_snapshot::start(
        &f.owner.root,
        "host/snapshot-client",
        10_000,
        &f.selected,
        &f.route,
    )
    .unwrap();
    let (client, server) = native_snapshot_lines(f.selected.clone(), 4096);
    std::thread::scope(|scope| {
        let worker = scope.spawn(|| serve(&f.owner.root, &verified, server));
        assert!(native_snapshot(f.client.take().unwrap(), &verified, client).is_err());
        assert!(worker.join().unwrap().is_err());
    });
    verify_offline(&f);
}
#[test]
fn failed_exchange_and_real_cleanup_failure_remain_failure() {
    let mut f = fixture();
    let verified = control::native_snapshot::start(
        &f.owner.root,
        "host/snapshot-client",
        10_000,
        &f.selected,
        &f.route,
    )
    .unwrap();
    let mut wrong = f.selected.clone();
    wrong.responder.boot_id = "boot/wrong-actual-session".into();
    let (client, server) = native_snapshot_lines(wrong, 2 * 1024 * 1024);
    f.owner.child.kill().unwrap();
    f.owner.child.wait().unwrap();
    let error = serve(&f.owner.root, &verified, server)
        .err()
        .expect("failed transport must not become success");
    assert!(error.contains("leave/abort"));
    assert!(error.contains("cancel"));
    drop(client);
}

#[path = "native_snapshot_payload_tests.rs"]
mod payload;
