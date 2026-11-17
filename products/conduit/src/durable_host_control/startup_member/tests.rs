//! Authenticated product control custody, not live transport or device proof.
use super::*;
use conduit_body::*;
pub(super) mod fixture;
use fixture::*;

#[test]
fn authentic_original_pin_prepares_commits_polls_and_releases_one_actual_member_kernel() {
    let mut f = fixture();
    authenticate(&mut f);
    let scope = f.session.realization().unwrap();
    let mut invalid = scope.plan.clone();
    invalid.plots[0].plan.plan_id = "plan/tampered".into();
    assert!(f
        .runtime
        .startup_member_command(Command::Prepare {
            session_id: f.wire_session.id,
            wake: Box::new(scope.wake.clone()),
            plan: Box::new(invalid),
            global_play: f.challenge.global_play.clone()
        })
        .is_err());
    assert!(!f.runtime.startup_member_custody_retained());
    assert!(f.runtime.startup_member.as_ref().unwrap().scope.is_none());
    assert!(f
        .runtime
        .startup_member
        .as_ref()
        .unwrap()
        .prepared
        .is_none());
    let receipt = prepare(&mut f);
    assert_ne!(
        receipt.member_play.active_play_id,
        receipt.global_play.active_play_id
    );
    assert!(matches!(
        poll(&mut f, &receipt),
        Reply::Progress {
            progress: Progress::Prepared,
            terminal: None,
            ..
        }
    ));
    let before = f.runtime.host.advertisement().clone();
    assert!(f.runtime.attach_owned_body(&f.joined.root).is_err());
    assert_eq!(f.runtime.host.advertisement(), &before);
    assert_eq!(
        f.runtime
            .startup_member
            .as_ref()
            .unwrap()
            .prepared
            .as_ref()
            .unwrap()
            .receipt(),
        &receipt
    );
    assert!(f
        .runtime
        .startup_member_command(Command::Commit {
            session_id: f.wire_session.id,
            preparation_id: receipt.preparation_id.clone(),
            biography: Box::new(f.session.evidence().clone())
        })
        .is_err());
    assert!(matches!(
        poll(&mut f, &receipt),
        Reply::Progress {
            progress: Progress::Prepared,
            terminal: None,
            ..
        }
    ));
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
    assert!(matches!(
        f.runtime
            .startup_member_command(Command::Commit {
                session_id: f.wire_session.id,
                preparation_id: receipt.preparation_id.clone(),
                biography: Box::new(f.session.evidence().clone())
            })
            .unwrap(),
        Reply::Committed { .. }
    ));
    assert!(f.runtime.startup_member.as_ref().unwrap().scope.is_none());
    assert!(f
        .runtime
        .startup_member_command(Command::Commit {
            session_id: f.wire_session.id,
            preparation_id: receipt.preparation_id.clone(),
            biography: Box::new(f.session.evidence().clone())
        })
        .is_err());
    assert_eq!(
        f.runtime
            .startup_member
            .as_ref()
            .unwrap()
            .prepared
            .as_ref()
            .unwrap()
            .receipt(),
        &receipt
    );
    let mut terminal = None;
    for _ in 0..32 {
        if let Reply::Progress {
            terminal: Some(actual),
            ..
        } = poll(&mut f, &receipt)
        {
            terminal = Some(actual);
            break;
        }
    }
    let terminal = terminal.expect("actual bounded Tick kernel terminal");
    assert_eq!(terminal.global_play, receipt.global_play);
    assert_eq!(terminal.member_play, receipt.member_play);
    assert!(terminal.startup_chimes.is_empty());
    assert!(matches!(
        f.runtime
            .startup_member_command(Command::Release {
                session_id: f.wire_session.id,
                preparation_id: receipt.preparation_id
            })
            .unwrap(),
        Reply::Released { .. }
    ));
    assert!(!f.runtime.startup_member_custody_retained());
    assert!(f.runtime.startup_member.is_none());
}
#[test]
fn failed_proof_and_foreign_pin_never_create_a_ticket_or_fence_ordinary_control() {
    let mut f = fixture();
    let (challenge, _) = begin(&mut f);
    assert!(f
        .runtime
        .startup_member_command(Command::Resume {
            session_id: f.wire_session.id,
            challenge,
            owner_signature: Signature([0; 64])
        })
        .is_err());
    assert!(!f.runtime.startup_member_custody_retained());
    let response = super::super::handle(
        Request::BodyLull {
            protocol: PROTOCOL,
            token: vec![23; 32],
        },
        &[23; 32],
        &mut f.runtime,
    );
    assert!(
        !matches!(response,Response::Refused {code,..} if code=="startup-member-custody-retained")
    );
    assert!(f.runtime.startup_member.is_none());
    let mut foreign = f.wire_session.clone();
    foreign.owner.verifying_key = [99; 32];
    assert!(f
        .runtime
        .startup_member_command(Command::BeginResume {
            session: Box::new(foreign),
            challenge: Box::new(f.challenge)
        })
        .is_err());
    assert!(f.runtime.startup_member.is_none());
}

#[test]
fn expired_original_challenge_refuses_without_ticket_then_allows_ordinary_control() {
    let mut f = fixture();
    let (mut challenge, _) = begin(&mut f);
    let now = crate::durable_host::current_time_millis().unwrap();
    challenge.federation.expires_at_millis = now - 1;
    challenge = Box::new(challenge.bind_session().unwrap());
    let signature = f
        .identity
        .sign_startup_challenge(&challenge, now - 2)
        .unwrap();
    if let Authentication::Pending {
        challenge: retained,
        ..
    } = &mut f.runtime.startup_member.as_mut().unwrap().authentication
    {
        *retained = challenge.clone();
    } else {
        panic!("pending original challenge required");
    }
    assert!(f
        .runtime
        .startup_member_command(Command::Resume {
            session_id: f.wire_session.id,
            challenge,
            owner_signature: Signature(signature)
        })
        .is_err());
    assert!(!f.runtime.startup_member_custody_retained());
    let response = super::super::handle(
        Request::BodyLull {
            protocol: PROTOCOL,
            token: vec![23; 32],
        },
        &[23; 32],
        &mut f.runtime,
    );
    assert!(
        !matches!(response,Response::Refused {code,..} if code=="startup-member-custody-retained")
    );
    assert!(f.runtime.startup_member.is_none());
}

#[test]
fn ordinary_control_invalidates_pending_owner_proof_without_creating_execution() {
    let mut f = fixture();
    let (challenge, _) = begin(&mut f);
    let signature = f
        .identity
        .sign_startup_challenge(
            &challenge,
            crate::durable_host::current_time_millis().unwrap(),
        )
        .unwrap();
    let _ = super::super::handle(
        Request::BodyLull {
            protocol: PROTOCOL,
            token: vec![23; 32],
        },
        &[23; 32],
        &mut f.runtime,
    );
    assert!(f.runtime.startup_member.is_none());
    assert!(f
        .runtime
        .startup_member_command(Command::Resume {
            session_id: f.wire_session.id,
            challenge,
            owner_signature: Signature(signature)
        })
        .is_err());
    assert!(!f.runtime.startup_member_custody_retained());
}

#[test]
fn changed_current_member_boot_refuses_original_pending_proof() {
    let mut f = fixture();
    let (challenge, _) = begin(&mut f);
    let signature = f
        .identity
        .sign_startup_challenge(
            &challenge,
            crate::durable_host::current_time_millis().unwrap(),
        )
        .unwrap();
    f.runtime.host = HostSource::Bare(Box::new(test_fixture::selected_host(
        f.wire_session.member.host_id.as_str(),
        "boot/replaced",
        false,
    )));
    assert!(f
        .runtime
        .startup_member_command(Command::Resume {
            session_id: f.wire_session.id,
            challenge,
            owner_signature: Signature(signature)
        })
        .is_err());
    assert!(!f.runtime.startup_member_custody_retained());
}

#[test]
fn explicit_pin_install_refusals_preserve_original_selected_pin() {
    let f = fixture();
    let original = std::fs::read(f.joined.root.join("body/member-contact.json")).unwrap();
    let descriptor = f.joined.root.join("selected-contact.json");
    // The real stopped-member lock prevents contact replacement while a service owns it.
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(f.joined.root.join("host-owner.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    assert!(
        crate::durable_host::install_member_contact(&f.joined.root, &descriptor, true).is_err()
    );
    lock.unlock().unwrap();
    let mut foreign: serde_json::Value = serde_json::from_slice(&original).unwrap();
    foreign["body_id"] = "body/foreign".into();
    std::fs::write(&descriptor, serde_json::to_vec(&foreign).unwrap()).unwrap();
    assert!(
        crate::durable_host::install_member_contact(&f.joined.root, &descriptor, true).is_err()
    );
    std::fs::write(&descriptor, vec![b' '; 16 * 1024 + 1]).unwrap();
    assert!(
        crate::durable_host::install_member_contact(&f.joined.root, &descriptor, true).is_err()
    );
    assert_eq!(
        std::fs::read(f.joined.root.join("body/member-contact.json")).unwrap(),
        original
    );
}

#[test]
fn foreign_begin_preserves_unexpired_pending_proof_and_authenticated_preparation() {
    let mut f = fixture();
    let (challenge, member_signature) = begin(&mut f);
    let mut foreign = f.wire_session.clone();
    foreign.owner.verifying_key = [99; 32];
    assert!(f
        .runtime
        .startup_member_command(Command::BeginResume {
            session: Box::new(foreign.clone()),
            challenge: Box::new(f.challenge.clone())
        })
        .is_err());
    let now = crate::durable_host::current_time_millis().unwrap();
    let signature = f.identity.sign_startup_challenge(&challenge, now).unwrap();
    FederatedReceivingSession::establish(
        challenge.federation.clone(),
        member_signature.0,
        signature,
        now,
    )
    .unwrap();
    assert!(matches!(
        f.runtime
            .startup_member_command(Command::Resume {
                session_id: f.wire_session.id,
                challenge,
                owner_signature: Signature(signature)
            })
            .unwrap(),
        Reply::Resumed { .. }
    ));
    assert!(f
        .runtime
        .startup_member_command(Command::BeginResume {
            session: Box::new(foreign),
            challenge: Box::new(f.challenge.clone())
        })
        .is_err());
    let receipt = prepare(&mut f);
    let result = f
        .runtime
        .startup_member_command(Command::Cancel {
            session_id: f.wire_session.id,
            preparation_id: receipt.preparation_id.clone(),
        })
        .unwrap();
    assert!(matches!(
        result,
        Reply::Progress {
            progress: Progress::Terminal,
            ..
        }
    ));
    assert!(matches!(
        f.runtime
            .startup_member_command(Command::Release {
                session_id: f.wire_session.id,
                preparation_id: receipt.preparation_id
            })
            .unwrap(),
        Reply::Released { .. }
    ));
}
