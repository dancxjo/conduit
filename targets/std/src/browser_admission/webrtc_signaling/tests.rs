use super::*;
use conduit_body::HostPresenceLease;
use conduit_body::{HostPresenceClock, HostPresenceClockScale};
use conduit_core::{
    bind_active_play, BaseInstanceId, ConnectionId, FragmentId, KindId, LineId, LinkEndpointId,
    LinkLimits, PlanId, PROTOCOL_VERSION,
};
use conduit_wire::{
    encode_session_frame_into, LineAttachment, SessionBinding, SessionEndpointIdentity,
    SessionLimits,
};

fn credential(label: &str) -> MembershipCredential {
    serde_json::from_value(serde_json::json!({
        "credential_id": format!("credential/{label}"),
        "body_id": "body/rendezvous",
        "part_id": format!("part/{label}"),
        "host_id": format!("host/{label}"),
        "boot_id": format!("boot/{label}"),
        "issued_at_millis": 1,
    }))
    .unwrap()
}

fn presence(source: &MembershipCredential, sink: &MembershipCredential) -> HostPresenceTable {
    HostPresenceTable {
        body_id: source.body_id.clone(),
        clock: HostPresenceClock::new(
            "clock/rendezvous/conformance".into(),
            HostPresenceClockScale::Milliseconds,
            1,
            0,
        )
        .unwrap(),
        maximum_lease_millis: 1_000,
        revision: 2,
        dropped_event_count: 0,
        leases: [source, sink]
            .into_iter()
            .enumerate()
            .map(|(index, credential)| HostPresenceLease {
                part_id: credential.part_id.clone(),
                host_id: credential.host_id.clone(),
                boot_id: credential.boot_id.clone(),
                offer_generation: conduit_core::OfferGeneration(1),
                membership_proof_id: serde_json::from_value(serde_json::json!(format!(
                    "proof/{index}"
                )))
                .unwrap(),
                session_binding_id: LinkBindingId::from(format!("presence/{index}")),
                sequence: 1,
                observed_at_millis: 0,
                expires_at_millis: 1_000,
                state: HostPresenceState::Available,
            })
            .collect(),
        events: Vec::new(),
    }
}

fn binding(source: &MembershipCredential, sink: &MembershipCredential) -> SessionBinding {
    let plan_id = PlanId::from("plan/rendezvous");
    SessionBinding {
        protocol_version: PROTOCOL_VERSION,
        plan_id: plan_id.clone(),
        source_fragment_id: FragmentId::from("fragment/source"),
        sink_fragment_id: FragmentId::from("fragment/sink"),
        source_active_play_id: bind_active_play(&plan_id, &source.host_id, &source.boot_id, 0)
            .active_play_id,
        sink_active_play_id: bind_active_play(&plan_id, &sink.host_id, &sink.boot_id, 0)
            .active_play_id,
        connection_id: ConnectionId::from("connection/rendezvous"),
        source: SessionEndpointIdentity {
            host_id: source.host_id.clone(),
            boot_id: source.boot_id.clone(),
        },
        sink: SessionEndpointIdentity {
            host_id: sink.host_id.clone(),
            boot_id: sink.boot_id.clone(),
        },
        value_kind: KindId::from("value/bounded@1"),
        limits: SessionLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: 16,
            maximum_buffered_bytes: 16,
        },
        attachment: LineAttachment {
            line_id: LineId::from("line/rendezvous"),
            link_binding_id: LinkBindingId::from("binding/rendezvous"),
            base: BaseImplementationId::from("conduit.base/webrtc-data-channel@1"),
            contract: browser_webrtc_line_contract(),
            base_instance_id: BaseInstanceId::from("base/rendezvous"),
            source_host_id: source.host_id.clone(),
            source_boot_id: source.boot_id.clone(),
            source_endpoint_id: LinkEndpointId::from("endpoint/source"),
            sink_host_id: sink.host_id.clone(),
            sink_boot_id: sink.boot_id.clone(),
            sink_endpoint_id: LinkEndpointId::from("endpoint/sink"),
            limits: LinkLimits {
                maximum_in_flight_items: 1,
                maximum_payload_bytes: 16,
                maximum_buffered_bytes: 16,
                maximum_frame_bytes: MAX_WEBRTC_SESSION_HELLO_BYTES as u32,
            },
        },
    }
}

fn signal(description: BrowserWebRtcDescription, hello: Vec<u8>) -> BrowserWebRtcSignal {
    BrowserWebRtcSignal {
        negotiation_id: LinkBindingId::from("binding/rendezvous"),
        description,
        session_hello: hello,
        sdp: match description {
            BrowserWebRtcDescription::Offer => "v=0\na=setup:actpass",
            BrowserWebRtcDescription::Answer => "v=0\na=setup:active",
        }
        .into(),
    }
}

#[test]
fn exact_available_pair_routes_offer_then_answer_and_invalidates_on_boot_loss() {
    let source = credential("source");
    let sink = credential("sink");
    let presence = presence(&source, &sink);
    let mut rendezvous = BrowserWebRtcRendezvous::default();
    let granted = binding(&source, &sink);
    let hello = rendezvous.preflight_grants([&granted]).unwrap().remove(0);
    assert_eq!(
        rendezvous.prepare(
            &presence,
            &source,
            sink.host_id.clone(),
            sink.boot_id.clone(),
            signal(BrowserWebRtcDescription::Offer, hello.clone()),
        ),
        Err(BrowserWebRtcRendezvousRefusal::UngrantedSession)
    );
    assert_eq!(rendezvous.replace_grants([&granted]).unwrap()[0], hello);
    let (total, source_grant) = rendezvous.grant_for_endpoint(&source.host_id, &source.boot_id, 0);
    assert_eq!(total, 1);
    let source_grant = source_grant.unwrap();
    assert_eq!(source_grant.role, BrowserWebRtcRole::Source);
    assert_eq!(source_grant.peer_host_id, sink.host_id);
    assert_eq!(source_grant.peer_boot_id, sink.boot_id);
    assert_eq!(source_grant.session_hello, hello);
    let (total, sink_grant) = rendezvous.grant_for_endpoint(&sink.host_id, &sink.boot_id, 0);
    assert_eq!(total, 1);
    assert_eq!(sink_grant.unwrap().role, BrowserWebRtcRole::Sink);
    assert_eq!(
        rendezvous.grant_for_endpoint(&HostId::from("host/absent"), &sink.boot_id, 0),
        (0, None)
    );
    assert_eq!(
        rendezvous.grant(&granted),
        Err(BrowserWebRtcRendezvousRefusal::DuplicateGrant)
    );

    let offered = rendezvous
        .prepare(
            &presence,
            &source,
            sink.host_id.clone(),
            sink.boot_id.clone(),
            signal(BrowserWebRtcDescription::Offer, hello.clone()),
        )
        .unwrap();
    assert!(rendezvous.negotiations.is_empty());
    assert!(rendezvous
        .prepare(
            &presence,
            &source,
            sink.host_id.clone(),
            sink.boot_id.clone(),
            signal(BrowserWebRtcDescription::Offer, hello.clone()),
        )
        .is_ok());
    rendezvous.commit(&offered).unwrap();
    assert_eq!(
        rendezvous.replace_grants(core::iter::empty()),
        Err(BrowserWebRtcRendezvousRefusal::InvalidStage)
    );
    assert_eq!(
        rendezvous.deactivate_grants(),
        vec![LinkBindingId::from("binding/rendezvous")]
    );
    assert_eq!(
        rendezvous.grant_for_endpoint(&source.host_id, &source.boot_id, 0),
        (0, None)
    );
    assert_eq!(
        rendezvous.prepare(
            &presence,
            &source,
            sink.host_id.clone(),
            sink.boot_id.clone(),
            signal(BrowserWebRtcDescription::Offer, hello.clone()),
        ),
        Err(BrowserWebRtcRendezvousRefusal::UngrantedSession)
    );
    let hello = rendezvous.grant(&granted).unwrap();
    let offered = rendezvous
        .prepare(
            &presence,
            &source,
            sink.host_id.clone(),
            sink.boot_id.clone(),
            signal(BrowserWebRtcDescription::Offer, hello.clone()),
        )
        .unwrap();
    rendezvous.commit(&offered).unwrap();
    assert_eq!(offered.target_host_id, sink.host_id);
    let answered = rendezvous
        .prepare(
            &presence,
            &sink,
            source.host_id.clone(),
            source.boot_id.clone(),
            signal(BrowserWebRtcDescription::Answer, hello),
        )
        .unwrap();
    rendezvous.commit(&answered).unwrap();
    assert_eq!(answered.target_host_id, source.host_id);

    assert_eq!(
        rendezvous.invalidate(&sink.host_id, &sink.boot_id),
        vec![LinkBindingId::from("binding/rendezvous")]
    );
    assert_eq!(
        rendezvous.grant_for_endpoint(&source.host_id, &source.boot_id, 0),
        (0, None)
    );
}

#[test]
fn stale_target_wrong_direction_and_duplicate_stage_refuse_without_relay() {
    let source = credential("source");
    let sink = credential("sink");
    let mut presence = presence(&source, &sink);
    let mut rendezvous = BrowserWebRtcRendezvous::default();
    let ungranted = binding(&source, &sink);
    let mut encoded = [0; MAX_WEBRTC_SESSION_HELLO_BYTES];
    let length = encode_session_frame_into(
        ungranted.hello_frame(),
        &mut encoded,
        ungranted.limits.maximum_payload_bytes,
        MAX_WEBRTC_SESSION_HELLO_BYTES as u32,
    )
    .unwrap();
    assert_eq!(
        rendezvous.prepare(
            &presence,
            &source,
            sink.host_id.clone(),
            sink.boot_id.clone(),
            signal(BrowserWebRtcDescription::Offer, encoded[..length].to_vec(),),
        ),
        Err(BrowserWebRtcRendezvousRefusal::UngrantedSession)
    );
    let hello = rendezvous.grant(&binding(&source, &sink)).unwrap();

    assert_eq!(
        rendezvous.prepare(
            &presence,
            &source,
            sink.host_id.clone(),
            BootId::from("boot/stale"),
            signal(BrowserWebRtcDescription::Offer, hello.clone()),
        ),
        Err(BrowserWebRtcRendezvousRefusal::TargetUnavailable)
    );
    assert_eq!(
        rendezvous.prepare(
            &presence,
            &sink,
            source.host_id.clone(),
            source.boot_id.clone(),
            signal(BrowserWebRtcDescription::Offer, hello.clone()),
        ),
        Err(BrowserWebRtcRendezvousRefusal::WrongDirection)
    );
    let offered = rendezvous
        .prepare(
            &presence,
            &source,
            sink.host_id.clone(),
            sink.boot_id.clone(),
            signal(BrowserWebRtcDescription::Offer, hello.clone()),
        )
        .unwrap();
    rendezvous.commit(&offered).unwrap();
    assert_eq!(
        rendezvous.prepare(
            &presence,
            &source,
            sink.host_id.clone(),
            sink.boot_id.clone(),
            signal(BrowserWebRtcDescription::Offer, hello.clone()),
        ),
        Err(BrowserWebRtcRendezvousRefusal::DuplicateNegotiation)
    );
    presence.leases[1].state = HostPresenceState::Unavailable;
    assert_eq!(
        rendezvous.prepare(
            &presence,
            &sink,
            source.host_id.clone(),
            source.boot_id.clone(),
            signal(BrowserWebRtcDescription::Answer, hello),
        ),
        Err(BrowserWebRtcRendezvousRefusal::SourceUnavailable)
    );
}

#[test]
fn failed_candidate_preflight_preserves_exact_prior_grant() {
    let source = credential("source");
    let sink = credential("sink");
    let presence = presence(&source, &sink);
    let mut rendezvous = BrowserWebRtcRendezvous::default();
    let prior = binding(&source, &sink);
    let prior_hello = rendezvous.replace_grants([&prior]).unwrap().remove(0);

    let mut candidate = prior.clone();
    candidate.attachment.link_binding_id = LinkBindingId::from("binding/candidate");
    let candidate_hello = rendezvous.preflight_grants([&candidate]).unwrap().remove(0);
    assert_ne!(candidate_hello, prior_hello);

    assert!(rendezvous
        .prepare(
            &presence,
            &source,
            sink.host_id.clone(),
            sink.boot_id.clone(),
            signal(BrowserWebRtcDescription::Offer, prior_hello),
        )
        .is_ok());
    let mut candidate_signal = signal(BrowserWebRtcDescription::Offer, candidate_hello);
    candidate_signal.negotiation_id = LinkBindingId::from("binding/candidate");
    assert_eq!(
        rendezvous.prepare(
            &presence,
            &source,
            sink.host_id.clone(),
            sink.boot_id.clone(),
            candidate_signal,
        ),
        Err(BrowserWebRtcRendezvousRefusal::UngrantedSession)
    );
}
