use conduit_protected_line::{
    OpaqueRelayService, RelayAttachmentDisposition, RelayEndpointRole, RelayEnvelope,
    RelayServiceError, RelayServiceLimits, RelaySlotDescriptor, RelaySlotDisposition,
    RELAY_SERVICE_IMPLEMENTATION_ID,
};

fn limits() -> RelayServiceLimits {
    RelayServiceLimits {
        maximum_slots: 1,
        maximum_protected_frame_bytes: 128,
        maximum_queued_frames_per_direction: 1,
        maximum_queued_bytes_per_direction: 256,
        maximum_attachment_attempts_per_slot: 4,
        maximum_idle_millis: 1_000,
        maximum_active_millis: 5_000,
    }
}

fn descriptor(route: &str, first_capability: [u8; 32]) -> RelaySlotDescriptor {
    RelaySlotDescriptor::new(
        route.into(),
        "negotiation/one".into(),
        "binding/first".into(),
        "binding/second".into(),
        10_000,
        first_capability,
        [8; 32],
    )
    .unwrap()
}

fn envelope(route: &str, protected: &[u8]) -> Vec<u8> {
    let mut encoded = vec![0; 256];
    let length = RelayEnvelope {
        route_id: route,
        protected_frame: protected,
    }
    .encode(&mut encoded)
    .unwrap();
    encoded.truncate(length);
    encoded
}

#[test]
fn exact_capability_pairs_two_endpoints_and_forwards_only_opaque_envelopes() {
    let mut relay = OpaqueRelayService::new(limits()).unwrap();
    relay
        .install_slot(descriptor("route/one", [7; 32]), 100)
        .unwrap();
    assert_eq!(
        relay.attach(
            "route/one",
            RelayEndpointRole::First,
            "binding/first",
            vec![7; 32],
            11,
            200,
        ),
        Ok(RelayAttachmentDisposition::WaitingForPeer)
    );
    assert_eq!(
        relay.forward(
            "route/one",
            RelayEndpointRole::First,
            11,
            &envelope("route/one", &[0xa5; 48]),
            250,
        ),
        Err(RelayServiceError::PeerWaiting)
    );
    assert_eq!(
        relay.attach(
            "route/one",
            RelayEndpointRole::Second,
            "binding/second",
            vec![8; 32],
            12,
            300,
        ),
        Ok(RelayAttachmentDisposition::Paired)
    );

    let protected = [0xa5; 48];
    let carried = envelope("route/one", &protected);
    relay
        .forward("route/one", RelayEndpointRole::First, 11, &carried, 400)
        .unwrap();
    assert_eq!(
        relay
            .receive("route/one", RelayEndpointRole::Second, 12, 400,)
            .unwrap(),
        Some(carried)
    );
    let evidence = relay
        .close("route/one", RelaySlotDisposition::Closed)
        .unwrap();
    assert_eq!(evidence.implementation_id, RELAY_SERVICE_IMPLEMENTATION_ID);
    assert_eq!(evidence.attached_endpoints, 2);
    assert_eq!(evidence.forwarded_frames, 1);
    assert_eq!(evidence.forwarded_bytes, protected.len() as u64);
    assert_eq!(evidence.disposition, RelaySlotDisposition::Closed);
    let serialized_debug = format!("{evidence:?}");
    assert!(!serialized_debug.contains("a5a5"));
    assert!(!serialized_debug.contains("070707"));
}

#[test]
fn authentication_binding_capacity_pressure_and_lifetime_refuse_distinctly() {
    let mut relay = OpaqueRelayService::new(limits()).unwrap();
    relay
        .install_slot(descriptor("route/one", [7; 32]), 100)
        .unwrap();
    assert_eq!(
        relay.install_slot(descriptor("route/two", [8; 32]), 100),
        Err(RelayServiceError::SlotCapacity)
    );
    assert_eq!(
        relay.attach(
            "route/one",
            RelayEndpointRole::First,
            "binding/first",
            vec![9; 32],
            11,
            200,
        ),
        Err(RelayServiceError::AuthenticationFailed)
    );
    assert_eq!(
        relay.attach(
            "route/one",
            RelayEndpointRole::First,
            "binding/wrong",
            vec![7; 32],
            11,
            200,
        ),
        Err(RelayServiceError::WrongEndpointBinding)
    );
    relay
        .attach(
            "route/one",
            RelayEndpointRole::First,
            "binding/first",
            vec![7; 32],
            11,
            200,
        )
        .unwrap();
    relay
        .attach(
            "route/one",
            RelayEndpointRole::Second,
            "binding/second",
            vec![8; 32],
            12,
            300,
        )
        .unwrap();
    let carried = envelope("route/one", &[0xa5; 48]);
    relay
        .forward("route/one", RelayEndpointRole::First, 11, &carried, 400)
        .unwrap();
    assert_eq!(
        relay.forward("route/one", RelayEndpointRole::First, 11, &carried, 400,),
        Err(RelayServiceError::QueuePressure)
    );
    assert_eq!(
        relay.receive("route/one", RelayEndpointRole::Second, 99, 400,),
        Err(RelayServiceError::ConnectionMismatch)
    );
    assert_eq!(
        relay.receive("route/one", RelayEndpointRole::Second, 12, 5_301,),
        Err(RelayServiceError::RelayConnectionLost)
    );
}

#[test]
fn idle_and_expiring_slots_do_not_become_a_mailbox_or_directory() {
    let mut relay = OpaqueRelayService::new(limits()).unwrap();
    relay
        .install_slot(descriptor("route/one", [7; 32]), 100)
        .unwrap();
    assert_eq!(
        relay.attach(
            "route/one",
            RelayEndpointRole::First,
            "binding/first",
            vec![7; 32],
            11,
            1_101,
        ),
        Err(RelayServiceError::ExpiredCapability)
    );
    assert_eq!(
        relay.receive("route/unknown", RelayEndpointRole::First, 11, 200),
        Err(RelayServiceError::UnknownRoute)
    );
}
