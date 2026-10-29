use super::*;
struct UnusedCarrier;
impl p::ProtectedFrameCarrier for UnusedCarrier {
    fn send_frame(&mut self, _: &[u8]) -> std::result::Result<(), p::CarrierFailure> {
        panic!("construction must not send")
    }
    fn receive_frame(
        &mut self,
        _: &mut [u8],
        _: u32,
    ) -> std::result::Result<usize, p::CarrierFailure> {
        panic!("construction must not receive")
    }
    fn close(&mut self) -> std::result::Result<(), p::CarrierFailure> {
        Ok(())
    }
}
pub(super) fn sessions(
    wrong_boot: bool,
) -> (
    p::ProtectedSession,
    p::ProtectedSession,
    ProtectedSessionPolicy,
) {
    let binding = p::SessionBinding {
        initiator: EndpointBinding {
            host_id: "host/collector".into(),
            boot_id: if wrong_boot {
                "boot/foreign"
            } else {
                "boot/collector"
            }
            .into(),
        },
        responder: EndpointBinding {
            host_id: "host/source".into(),
            boot_id: "boot/source".into(),
        },
        negotiation_id: "proof/negotiation".into(),
        line_session_id: "proof/session".into(),
        candidate_binding: "proof/route".into(),
        transport_binding: "proof/protected".into(),
    };
    let policy = ProtectedSessionPolicy {
        traffic: SessionLimits {
            maximum_payload_bytes: 4096,
            maximum_frames_per_direction: 8,
            maximum_bytes_per_direction: 8192,
        },
        maximum_simultaneous_sessions: 1,
        maximum_pending_frames_per_session: 1,
        handshake_work_units: 2,
        handshake_timeout_millis: 1000,
        idle_timeout_millis: 1000,
    };
    let mut a =
        p::ProtectedHandshake::new(Role::Initiator, &binding, policy.traffic, [7; 32], [1; 32])
            .unwrap();
    let mut b =
        p::ProtectedHandshake::new(Role::Responder, &binding, policy.traffic, [7; 32], [2; 32])
            .unwrap();
    let mut frame = vec![0; a.next_message_bytes().unwrap()];
    a.write_message(&mut frame).unwrap();
    b.read_message(&frame).unwrap();
    frame.resize(b.next_message_bytes().unwrap(), 0);
    b.write_message(&mut frame).unwrap();
    a.read_message(&frame).unwrap();
    (a.finish().unwrap(), b.finish().unwrap(), policy)
}
fn protected(role: Role, wrong_boot: bool) -> ProtectedCarrier<UnusedCarrier> {
    let (a, b, policy) = sessions(wrong_boot);
    ProtectedCarrier::new(
        UnusedCarrier,
        if role == Role::Initiator { a } else { b },
        policy,
    )
    .unwrap()
}
#[test]
fn local_role_and_boot_must_match_exact_planned_endpoint() {
    let lines = [
        line_offer("collector", "source", "request", true),
        line_offer("source", "collector", "response", true),
    ];
    let prepared = crate::history_relay_plan::prepare_proof_plan(
        Archive {
            zero: vec![],
            end: vec![],
        },
        &lines,
    )
    .unwrap();
    let connections: Vec<_> = prepared
        .plan
        .fragments
        .iter()
        .flat_map(|f| &f.connections)
        .collect();
    let request = &connections
        .iter()
        .find(|c| c.sink_placement_id == prepared.provider_placement)
        .unwrap()
        .connection_id;
    let response = &connections
        .iter()
        .find(|c| c.source_placement_id == prepared.provider_placement)
        .unwrap()
        .connection_id;
    for (role, collector, wrong_boot, accepted) in [
        (Role::Initiator, true, false, true),
        (Role::Responder, false, false, true),
        (Role::Responder, true, false, false),
        (Role::Initiator, false, false, false),
        (Role::Initiator, true, true, false),
    ] {
        let result = HistoryRelay::from_plan(
            protected(role, wrong_boot),
            &prepared.plan,
            request,
            response,
            collector,
        );
        assert_eq!(
            result.is_ok(),
            accepted,
            "{role:?} collector={collector} wrong_boot={wrong_boot}"
        );
    }
}
