//! Actual original joined key/pin, mutual proof, complete prepared kernel custody.
//! No network, device, process or audible playback proof.
use super::*;
use conduit_std_host::body_execution::StartupOwnerIdentity;
use sha2::Digest;

pub(super) struct Fixture {
    pub(super) joined: crate::durable_host::StartupJoinedFixture,
    pub(super) runtime: Box<DurableHostRuntime>,
    pub(super) owner: StdHost,
    pub(super) identity: StartupOwnerIdentity,
    pub(super) session: BodyLifecycleSession,
    pub(super) wire_session: Session,
    pub(super) challenge: StartupOwnerChallenge,
}

pub(super) fn fixture() -> Fixture {
    let mut joined = crate::durable_host::startup_joined_fixture();
    let member = test_fixture::selected_host(
        joined.receipt.credential.host_id.as_str(),
        joined.receipt.credential.boot_id.as_str(),
        false,
    );
    let expanded = crate::plot_source::parse(
        "plot clock-demo {\nclock: time/tick(count = 1, period-ms = 0)\n}\n",
    )
    .unwrap()
    .expand_named_for_authoring("clock-demo")
    .unwrap();
    let plan = member.plan_expanded_local(&expanded.expanded).unwrap();
    let resident = ResidentPlot::new(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
    );
    let body = Body::born(
        resident.source_document_id.clone(),
        resident.checked_plot_id.clone(),
        1,
        "sign/product-born".into(),
    )
    .unwrap();
    joined.select_body(&body.body_id);
    crate::durable_host::complete_startup_joined_fixture(&joined).unwrap();
    let owner = test_fixture::selected_host("host/original-owner", "boot/original-owner", false);
    let identity = owner.prepare_startup_owner_identity().unwrap();
    let owner_peer = identity.peer().clone();
    let contact = serde_json::json!({"schema":"conduit.body/member-contact@1","body_id":body.body_id,"part_id":joined.receipt.credential.part_id,"host_id":joined.receipt.credential.host_id,
        "owner":owner_peer,"reachability":"wss://192.0.2.5:443","authentication":{"server_identity":"192.0.2.5","transport_binding_sha256":vec![9;32]},"reconnect_after_millis":1000,"attempt_timeout_millis":3000,"maximum_attempts":2});
    let path = joined.root.join("selected-contact.json");
    std::fs::write(&path, serde_json::to_vec(&contact).unwrap()).unwrap();
    assert!(crate::durable_host::install_member_contact(&joined.root, &path, false).is_err());
    crate::durable_host::install_member_contact(&joined.root, &path, true).unwrap();
    assert_eq!(
        crate::durable_host::current_joined_owner_pin(&joined.root).unwrap(),
        owner_peer
    );
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let mut biography = BodyBiographyEvidence::born(
        body.clone(),
        membership.clone(),
        "Actual prepared member test".into(),
    )
    .unwrap();
    for (index, (part, ad)) in [
        (
            PartId::bind(&body.body_id, owner_peer.host_id.as_str(), 0).unwrap(),
            owner.advertisement(),
        ),
        (
            joined.receipt.credential.part_id.clone(),
            member.advertisement(),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let proof = MembershipProofId::bind(&format!("fixture/proof/{index}")).unwrap();
        membership
            .admit(
                &body.body_id,
                membership.revision,
                part.clone(),
                proof.clone(),
                format!("fixture/admit/{index}").into(),
            )
            .unwrap();
        membership
            .observe_present(
                &body.body_id,
                membership.revision,
                &part,
                AuthenticatedHostObservation {
                    host_id: ad.host_id.clone(),
                    boot_id: ad.boot_id.clone(),
                    offer_generation: ad.offer_generation,
                    proof_id: proof,
                    sequence: 0,
                },
                format!("fixture/present/{index}").into(),
            )
            .unwrap();
    }
    let sequences = membership
        .events
        .iter()
        .enumerate()
        .map(|(i, e)| (e.change_id.clone(), i as u64 + 2))
        .collect::<Vec<_>>();
    biography
        .append_membership_events(membership, &sequences)
        .unwrap();
    let mut session =
        BodyLifecycleSession::open_admitted(biography, &owner_peer.host_id, &owner_peer.boot_id)
            .unwrap();
    session
        .propose(
            vec![BodyPlotPlan {
                plot: resident,
                plan,
            }],
            &owner_peer.host_id,
            &owner_peer.boot_id,
        )
        .unwrap();
    let realization = session.realization().unwrap();
    let global = owner
        .prospective_body_play(&realization.wake, &realization.plan)
        .unwrap();
    let wire_session = Session {
        id: [13; 32],
        member: joined.receipt.credential.clone(),
        owner: owner_peer.clone(),
    };
    let challenge = StartupOwnerChallenge {
        body_id: body.body_id,
        wake_id: realization.wake.wake_id.clone(),
        plan_id: realization.plan.plan_id.clone(),
        global_play: global,
        part_id: wire_session.member.part_id.clone(),
        credential_id: wire_session.member.credential_id.clone(),
        member_offer_digest: sha2::Sha256::digest(
            serde_json::to_vec(member.advertisement()).unwrap(),
        )
        .into(),
        transport_binding_sha256: [17; 32],
        federation: FederationChallenge {
            session_id: "pending".into(),
            line_id: "pending".into(),
            line_security: conduit_core::LineSecurity::AuthenticatedEncrypted,
            initiator: FederationPeer {
                host_id: member.advertisement().host_id.clone(),
                boot_id: member.advertisement().boot_id.clone(),
                offer_generation: member.advertisement().offer_generation,
                verifying_key: joined.verifying_key(),
            },
            responder: owner_peer,
            nonce: [1; 32],
            expires_at_millis: crate::durable_host::current_time_millis().unwrap() + 30_000,
            maximum_frames: 64,
        },
    };
    let mut runtime = Box::new(DurableHostRuntime::new(
        "fixture".into(),
        "fixture".into(),
        member,
    ));
    runtime.birth_root = Some(joined.root.clone());
    Fixture {
        joined,
        runtime,
        owner,
        identity,
        session,
        wire_session,
        challenge,
    }
}

pub(super) fn begin(f: &mut Fixture) -> (Box<StartupOwnerChallenge>, Signature) {
    let Reply::Challenge {
        challenge,
        member_signature,
        ..
    } = f
        .runtime
        .startup_member_command(Command::BeginResume {
            session: Box::new(f.wire_session.clone()),
            challenge: Box::new(f.challenge.clone()),
        })
        .unwrap()
    else {
        panic!("actual member challenge required")
    };
    (challenge, member_signature)
}
pub(super) fn authenticate(f: &mut Fixture) {
    let (challenge, member_signature) = begin(f);
    let now = crate::durable_host::current_time_millis().unwrap();
    let owner_signature = f.identity.sign_startup_challenge(&challenge, now).unwrap();
    FederatedReceivingSession::establish(
        challenge.federation.clone(),
        member_signature.0,
        owner_signature,
        now,
    )
    .unwrap();
    let replay = Command::Resume {
        session_id: f.wire_session.id,
        challenge: challenge.clone(),
        owner_signature: Signature(owner_signature),
    };
    assert!(matches!(
        f.runtime
            .startup_member_command(Command::Resume {
                session_id: f.wire_session.id,
                challenge,
                owner_signature: Signature(owner_signature)
            })
            .unwrap(),
        Reply::Resumed { .. }
    ));
    assert!(f.runtime.startup_member_command(replay).is_err());
}
pub(super) fn prepare(f: &mut Fixture) -> DelegatedBodyPreparationReceipt {
    let scope = f.session.realization().unwrap();
    let Reply::Prepared { receipt } = f
        .runtime
        .startup_member_command(Command::Prepare {
            session_id: f.wire_session.id,
            wake: Box::new(scope.wake.clone()),
            plan: Box::new(scope.plan.clone()),
            global_play: f.challenge.global_play.clone(),
        })
        .unwrap()
    else {
        panic!("actual complete ticket required")
    };
    *receipt
}
pub(super) fn poll(f: &mut Fixture, receipt: &DelegatedBodyPreparationReceipt) -> Reply {
    f.runtime
        .startup_member_command(Command::Poll {
            session_id: f.wire_session.id,
            preparation_id: receipt.preparation_id.clone(),
        })
        .unwrap()
}

pub(crate) struct TerminalControl {
    pub(crate) joined: crate::durable_host::StartupJoinedFixture,
    runtime: Box<DurableHostRuntime>,
    receipt: DelegatedBodyPreparationReceipt,
    session_id: [u8; 32],
}
impl TerminalControl {
    pub(crate) fn release(&mut self) -> Result<(), String> {
        match self.runtime.startup_member_command(Command::Release {
            session_id: self.session_id,
            preparation_id: self.receipt.preparation_id.clone(),
        })? {
            Reply::Released { .. } => Ok(()),
            _ => Err("actual member release did not acknowledge".into()),
        }
    }
    pub(crate) fn retains_ticket(&self) -> bool {
        self.runtime.startup_member_custody_retained()
    }
}
pub(crate) fn completed_custody() -> (
    StdHost,
    BodyLifecycleSession,
    DelegatedBodyTerminalReceipt,
    TerminalControl,
) {
    let (mut f, receipt) = started_custody();
    f.runtime
        .startup_member_command(Command::Commit {
            session_id: f.wire_session.id,
            preparation_id: receipt.preparation_id.clone(),
            biography: Box::new(f.session.evidence().clone()),
        })
        .unwrap();
    let mut terminal = None;
    for _ in 0..32 {
        if let Reply::Progress {
            terminal: Some(actual),
            ..
        } = poll(&mut f, &receipt)
        {
            terminal = Some(*actual);
            break;
        }
    }
    let terminal = terminal.expect("actual prepared Tick terminal");
    (
        f.owner,
        f.session,
        terminal,
        TerminalControl {
            joined: f.joined,
            runtime: f.runtime,
            receipt,
            session_id: f.wire_session.id,
        },
    )
}

fn started_custody() -> (Fixture, DelegatedBodyPreparationReceipt) {
    let mut f = fixture();
    authenticate(&mut f);
    let receipt = prepare(&mut f);
    let scope = f.session.realization().unwrap();
    let (play, wake) = f
        .owner
        .acknowledge_delegated_body_preparation(&scope.wake, &scope.plan, &receipt)
        .unwrap();
    f.session
        .started(
            &f.identity.peer().host_id,
            &f.identity.peer().boot_id,
            play,
            wake,
        )
        .unwrap();
    (f, receipt)
}

pub(crate) fn prepared_wire_custody() -> (
    StdHost,
    BodyLifecycleSession,
    DelegatedBodyPreparationReceipt,
    TerminalControl,
) {
    let (f, receipt) = started_custody();
    (
        f.owner,
        f.session,
        receipt.clone(),
        TerminalControl {
            joined: f.joined,
            runtime: f.runtime,
            receipt,
            session_id: f.wire_session.id,
        },
    )
}
impl TerminalControl {
    pub(crate) fn wire_session_id(&self) -> [u8; 32] {
        self.session_id
    }
    pub(crate) fn dispatch(&mut self, command: Command) -> Result<Reply, String> {
        match super::super::super::handle(
            Request::StartupMember {
                protocol: PROTOCOL,
                token: vec![23; 32],
                command: Box::new(command),
            },
            &[23; 32],
            &mut self.runtime,
        ) {
            Response::StartupMember { reply, .. } => Ok(*reply),
            Response::Refused { code, .. } => Err(code),
            _ => Err("wrong fixture control response".into()),
        }
    }
}
