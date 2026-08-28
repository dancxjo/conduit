use conduit_core::{
    bind_active_play, ArtifactId, AuthorityGrantId, BaseImplementationId, BaseInstanceId, BootId,
    CapabilityId, ConnectionId, FragmentId, HostAdvertisement, HostId, HostProfileId,
    ImplementationId, KindId, LineId, LinkBindingId, LinkEndpointId, LinkLimits, OfferGeneration,
    PlanId, PROTOCOL_VERSION,
};
use conduit_observatory::{HostReport, OperationalState};
use conduit_system_continuity::{
    delegated_reboot_face, delegated_reboot_offer, DelegatedRebootGrant,
    DelegatedRebootTransaction, HostInstance, LineLossDisposition, RebootDecision, RebootDenial,
    RebootPendingState, RebootProgressError, RebootRequest, RebootRequestId,
};
use conduit_wire::{LineAttachment, SessionBinding, SessionEndpointIdentity, SessionLimits};

fn host(host: &str, boot: &str) -> HostInstance {
    HostInstance {
        host_id: HostId::from(host),
        boot_id: BootId::from(boot),
    }
}

fn advertisement(instance: &HostInstance, supports_reboot: bool) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: instance.host_id.clone(),
        boot_id: instance.boot_id.clone(),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("fixture/lifecycle"),
        resources: vec![],
        capabilities: if supports_reboot {
            vec![delegated_reboot_offer(
                CapabilityId::from("capability/reboot"),
                ImplementationId::from("fixture/reboot-v1"),
                ArtifactId::from("fixture/reboot-artifact"),
            )]
        } else {
            vec![]
        },
        planner_capabilities: vec![],
    }
}

fn session(controller: &HostInstance, target: &HostInstance) -> SessionBinding {
    let plan_id = PlanId::from("plan/delegated-reboot");
    SessionBinding {
        protocol_version: PROTOCOL_VERSION,
        source_fragment_id: FragmentId::from("fragment/controller"),
        sink_fragment_id: FragmentId::from("fragment/target"),
        source_active_play_id: bind_active_play(
            &plan_id,
            &controller.host_id,
            &controller.boot_id,
            0,
        )
        .active_play_id,
        sink_active_play_id: bind_active_play(&plan_id, &target.host_id, &target.boot_id, 0)
            .active_play_id,
        plan_id,
        connection_id: ConnectionId::from("connection/reboot-control"),
        source: SessionEndpointIdentity {
            host_id: controller.host_id.clone(),
            boot_id: controller.boot_id.clone(),
        },
        sink: SessionEndpointIdentity {
            host_id: target.host_id.clone(),
            boot_id: target.boot_id.clone(),
        },
        value_kind: KindId::from("lifecycle/reboot-request"),
        limits: SessionLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: 256,
            maximum_buffered_bytes: 256,
        },
        attachment: LineAttachment {
            line_id: LineId::from("line/controller-to-target"),
            link_binding_id: LinkBindingId::from("link/controller-to-target"),
            base: BaseImplementationId::from("conduit.proof/frame@1"),
            contract: conduit_core::LineContract {
                scope: conduit_core::LineScope::LocalNetwork,
                traffic_shape: conduit_core::LineTrafficShape::Message,
                duplex: conduit_core::LineDuplex::FullDuplex,
                ordering: conduit_core::LineOrdering::Ordered,
                reliability: conduit_core::LineReliability::Reliable,
                continuation: conduit_core::LineContinuation::None,
                security: conduit_core::LineSecurity::PlaintextNetwork,
            },
            base_instance_id: BaseInstanceId::from("base/reboot-fixture"),
            source_host_id: controller.host_id.clone(),
            source_boot_id: controller.boot_id.clone(),
            source_endpoint_id: LinkEndpointId::from("endpoint/controller"),
            sink_host_id: target.host_id.clone(),
            sink_boot_id: target.boot_id.clone(),
            sink_endpoint_id: LinkEndpointId::from("endpoint/target"),
            limits: LinkLimits {
                maximum_in_flight_items: 1,
                maximum_payload_bytes: 256,
                maximum_buffered_bytes: 256,
                maximum_frame_bytes: 1024,
            },
        },
    }
}

fn grant(controller: &HostInstance, target: &HostInstance) -> DelegatedRebootGrant {
    DelegatedRebootGrant {
        grant_id: AuthorityGrantId::from("grant/controller-may-reboot-target-once"),
        controller: controller.clone(),
        subject: target.clone(),
        capability_id: CapabilityId::from("capability/reboot"),
        selected_line_id: LineId::from("line/controller-to-target"),
        maximum_transitions: 1,
        proof_window_ticks: 2,
        sign_sequence_base: 40,
    }
}

fn request(controller: &HostInstance, target: &HostInstance, id: &str) -> RebootRequest {
    RebootRequest {
        request_id: RebootRequestId::from(id),
        controller: controller.clone(),
        target: target.clone(),
        required_face: delegated_reboot_face(),
        selected_line_id: LineId::from("line/controller-to-target"),
    }
}

#[test]
fn authorized_reboot_separates_acceptance_line_loss_and_completed_proof() {
    let controller = host("host/controller", "boot/controller-1");
    let target = host("host/target", "boot/target-1");
    let mut transaction = DelegatedRebootTransaction::new(grant(&controller, &target));
    let request = request(&controller, &target, "request/reboot-1");
    let accepted = match transaction.submit(
        &request,
        &advertisement(&target, true),
        &session(&controller, &target),
    ) {
        RebootDecision::Accepted(receipt) => receipt,
        RebootDecision::Denied(receipt) => panic!("unexpected denial: {:?}", receipt.reason),
    };
    assert_eq!(accepted.attempts_remaining, 0);
    assert_eq!(transaction.state(), RebootPendingState::Accepted);
    assert_eq!(
        transaction.control_line_lost(),
        LineLossDisposition::IntentionalTransitionPending
    );

    transaction
        .old_boot_terminated(
            &request.request_id,
            conduit_core::SignId::from("sign/old-boot-terminal"),
        )
        .unwrap();
    assert_eq!(transaction.state(), RebootPendingState::AwaitingReplacement);

    let replacement = host("host/target", "boot/target-2");
    let proof = transaction
        .observe_replacement(
            &request.request_id,
            &HostReport {
                advertisement: advertisement(&replacement, true),
                state: OperationalState::Available,
                capabilities: vec![],
            },
            conduit_core::SignId::from("sign/new-boot-report"),
        )
        .unwrap();
    assert_ne!(proof.acceptance.target.boot_id, proof.new_boot);
    assert_eq!(transaction.state(), RebootPendingState::Completed);
}

#[test]
fn support_authority_reachability_boot_and_replay_fail_independently() {
    let controller = host("host/controller", "boot/controller-1");
    let intruder = host("host/intruder", "boot/intruder-1");
    let target = host("host/target", "boot/target-1");

    let mut unsupported_transaction = DelegatedRebootTransaction::new(grant(&controller, &target));
    let unsupported = unsupported_transaction.submit(
        &request(&controller, &target, "request/unsupported"),
        &advertisement(&target, false),
        &session(&controller, &target),
    );
    assert!(matches!(
        unsupported,
        RebootDecision::Denied(receipt) if receipt.reason == RebootDenial::Unsupported
    ));
    assert!(matches!(
        unsupported_transaction.submit(
            &request(&controller, &target, "request/second-attempt"),
            &advertisement(&target, true),
            &session(&controller, &target)
        ),
        RebootDecision::Denied(receipt) if receipt.reason == RebootDenial::AttemptLimitReached
    ));

    let unauthorized = DelegatedRebootTransaction::new(grant(&controller, &target)).submit(
        &request(&intruder, &target, "request/unauthorized"),
        &advertisement(&target, true),
        &session(&intruder, &target),
    );
    assert!(matches!(
        unauthorized,
        RebootDecision::Denied(receipt) if receipt.reason == RebootDenial::Unauthorized
    ));

    let mut wrong_session = session(&controller, &target);
    wrong_session.attachment.line_id = LineId::from("line/other");
    let unreachable = DelegatedRebootTransaction::new(grant(&controller, &target)).submit(
        &request(&controller, &target, "request/wrong-session"),
        &advertisement(&target, true),
        &wrong_session,
    );
    assert!(matches!(
        unreachable,
        RebootDecision::Denied(receipt) if receipt.reason == RebootDenial::SessionMismatch
    ));

    let stale = host("host/target", "boot/target-old");
    let stale_request = request(&controller, &stale, "request/stale");
    let stale_result = DelegatedRebootTransaction::new(grant(&controller, &target)).submit(
        &stale_request,
        &advertisement(&target, true),
        &session(&controller, &stale),
    );
    assert!(matches!(
        stale_result,
        RebootDecision::Denied(receipt) if receipt.reason == RebootDenial::StaleTargetBoot
    ));

    let replay_request = request(&controller, &target, "request/replay");
    let mut replay = DelegatedRebootTransaction::new(grant(&controller, &target));
    let accepted = replay.submit(
        &replay_request,
        &advertisement(&target, true),
        &session(&controller, &target),
    );
    let replayed = replay.submit(
        &replay_request,
        &advertisement(&target, true),
        &session(&controller, &target),
    );
    let (RebootDecision::Accepted(accepted), RebootDecision::Denied(replayed)) =
        (accepted, replayed)
    else {
        panic!("expected accepted request followed by replay denial");
    };
    assert_eq!(replayed.reason, RebootDenial::Replay);
    assert_ne!(accepted.sign_id, replayed.sign_id);
    assert_eq!(replay.attempts_used(), 1);
}

#[test]
fn proof_window_expires_to_unknown_without_fabricating_transport_failure() {
    let controller = host("host/controller", "boot/controller-1");
    let target = host("host/target", "boot/target-1");
    let request = request(&controller, &target, "request/timeout");
    let mut transaction = DelegatedRebootTransaction::new(grant(&controller, &target));
    assert!(matches!(
        transaction.submit(
            &request,
            &advertisement(&target, true),
            &session(&controller, &target)
        ),
        RebootDecision::Accepted(_)
    ));
    transaction.tick_proof_window();
    transaction.tick_proof_window();
    assert_eq!(
        transaction.state(),
        RebootPendingState::UnknownProofWindowExpired
    );
    assert_eq!(
        transaction.control_line_lost(),
        LineLossDisposition::OrdinaryTransportFailure
    );
    assert_eq!(
        transaction
            .old_boot_terminated(&request.request_id, conduit_core::SignId::from("sign/late")),
        Err(RebootProgressError::ProofWindowExpired)
    );
}

#[test]
fn malformed_request_is_denied_with_machine_readable_sign() {
    let controller = host("host/controller", "boot/controller-1");
    let target = host("host/target", "boot/target-1");
    let malformed = request(&controller, &target, "");
    let denied = DelegatedRebootTransaction::new(grant(&controller, &target)).submit(
        &malformed,
        &advertisement(&target, true),
        &session(&controller, &target),
    );
    let RebootDecision::Denied(receipt) = denied else {
        panic!("malformed request was accepted");
    };
    assert_eq!(receipt.reason, RebootDenial::MalformedRequest);
    assert!(!receipt.sign_id.as_str().is_empty());
}

#[test]
fn equal_face_realization_is_compatible_but_does_not_bypass_exact_grant() {
    let controller = host("host/controller", "boot/controller-1");
    let target = host("host/target", "boot/target-1");
    let mut renamed = advertisement(&target, true);
    renamed.capabilities[0].kind_id = KindId::from("vendor/maintenance-cycle");
    renamed.capabilities[0].kind_contract_revision =
        conduit_core::KindContractRevision::from("vendor/maintenance-cycle@9");
    assert_eq!(
        renamed.capabilities[0].checked_face(),
        delegated_reboot_face()
    );

    let accepted = DelegatedRebootTransaction::new(grant(&controller, &target)).submit(
        &request(&controller, &target, "request/equal-face"),
        &renamed,
        &session(&controller, &target),
    );
    assert!(matches!(accepted, RebootDecision::Accepted(_)));

    let mut wrong_grant = grant(&controller, &target);
    wrong_grant.capability_id = CapabilityId::from("capability/not-selected");
    let denied = DelegatedRebootTransaction::new(wrong_grant).submit(
        &request(&controller, &target, "request/wrong-grant"),
        &renamed,
        &session(&controller, &target),
    );
    assert!(matches!(
        denied,
        RebootDecision::Denied(receipt) if receipt.reason == RebootDenial::Unauthorized
    ));
}
