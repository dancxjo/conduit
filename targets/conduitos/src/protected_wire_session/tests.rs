use super::*;

use alloc::vec::Vec;
use conduit_core::{
    BaseImplementationId, BaseInstanceId, BootId, ConnectionId, FragmentId, HostId, KindId,
    LineContinuation, LineContract, LineDuplex, LineId, LineOrdering, LineReliability, LineScope,
    LineSecurity, LineTrafficShape, LinkBindingId, LinkEndpointId, LinkLimits, PROTOCOL_VERSION,
    PlanId, bind_active_play,
};
use conduit_wire::{LineAttachment, SessionEndpointIdentity, SessionLimits};
use core::cell::{Cell, RefCell};

const PROVIDER: NetworkProvider = NetworkProvider {
    base_id: "network/virtio-net-0",
    provider_instance_id: "pci/00:02.0/virtio-net",
    provider_generation: 1,
    boot_id: [7; 32],
};

struct ProviderCell(Cell<NetworkProvider>);

impl NetworkProviderTruth for ProviderCell {
    fn current_provider(&self) -> NetworkProvider {
        self.0.get()
    }
}

#[derive(Default)]
struct SharedFrames {
    sent: RefCell<Vec<u8>>,
}

struct FixtureProtected<'a> {
    shared: &'a SharedFrames,
    inbound: Vec<Vec<u8>>,
    next: usize,
    current: [u8; MAXIMUM_CONDUITOS_WIRE_FRAME_BYTES],
    current_len: usize,
}

impl ConduitOsProtectedLineIo for FixtureProtected<'_> {
    fn send(&mut self, payload: &[u8]) -> Result<(), ProtectedLineError> {
        self.shared.sent.replace(payload.to_vec());
        Ok(())
    }

    fn receive(&mut self) -> Result<&[u8], ProtectedLineError> {
        let bytes = self
            .inbound
            .get(self.next)
            .ok_or(ProtectedLineError::OuterCarrierLost)?;
        self.current[..bytes.len()].copy_from_slice(bytes);
        self.current_len = bytes.len();
        self.next += 1;
        Ok(&self.current[..self.current_len])
    }

    fn close(&mut self) -> Result<(), ProtectedLineError> {
        Ok(())
    }
}

fn binding() -> SessionBinding {
    let plan_id = PlanId::from("plan/relay");
    let source_host = HostId::from("host/conduitos");
    let source_boot = BootId::from("boot/conduitos");
    let sink_host = HostId::from("host/std");
    let sink_boot = BootId::from("boot/std");
    let contract = LineContract {
        scope: LineScope::RoutedNetwork,
        traffic_shape: LineTrafficShape::Message,
        duplex: LineDuplex::FullDuplex,
        ordering: LineOrdering::Ordered,
        reliability: LineReliability::Reliable,
        continuation: LineContinuation::None,
        security: LineSecurity::AuthenticatedEncrypted,
    };
    SessionBinding {
        protocol_version: PROTOCOL_VERSION,
        plan_id: plan_id.clone(),
        source_fragment_id: FragmentId::from("fragment/source"),
        sink_fragment_id: FragmentId::from("fragment/sink"),
        source_active_play_id: bind_active_play(&plan_id, &source_host, &source_boot, 0)
            .active_play_id,
        sink_active_play_id: bind_active_play(&plan_id, &sink_host, &sink_boot, 0).active_play_id,
        connection_id: ConnectionId::from("connection/remote"),
        source: SessionEndpointIdentity {
            host_id: source_host.clone(),
            boot_id: source_boot.clone(),
        },
        sink: SessionEndpointIdentity {
            host_id: sink_host.clone(),
            boot_id: sink_boot.clone(),
        },
        value_kind: KindId::from("value/bytes"),
        limits: SessionLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: 64,
            maximum_buffered_bytes: 64,
        },
        attachment: LineAttachment {
            line_id: LineId::from("line/relay"),
            link_binding_id: LinkBindingId::from("binding/relay"),
            base: BaseImplementationId::from("conduit.base/virtio-protected-relay@1"),
            base_instance_id: BaseInstanceId::from("base/virtio/one"),
            contract,
            source_host_id: source_host,
            source_boot_id: source_boot,
            source_endpoint_id: LinkEndpointId::from("endpoint/source"),
            sink_host_id: sink_host,
            sink_boot_id: sink_boot,
            sink_endpoint_id: LinkEndpointId::from("endpoint/sink"),
            limits: LinkLimits {
                maximum_in_flight_items: 1,
                maximum_payload_bytes: 64,
                maximum_buffered_bytes: 64,
                maximum_frame_bytes: 1024,
            },
        },
    }
}

fn encoded(binding: &SessionBinding, message: SessionMessage<'_>) -> Vec<u8> {
    let mut bytes = [0; MAXIMUM_CONDUITOS_WIRE_FRAME_BYTES];
    let length = encode_session_frame_into(
        binding.frame(message),
        &mut bytes,
        binding.limits.maximum_payload_bytes,
        binding.attachment.limits.maximum_frame_bytes,
    )
    .unwrap();
    bytes[..length].to_vec()
}

#[test]
fn canonical_session_frames_cross_the_bounded_protected_line() {
    let frames = SharedFrames::default();
    let source_provider = ProviderCell(Cell::new(PROVIDER));
    let binding = binding();
    let mut source_io = FixtureProtected {
        shared: &frames,
        inbound: alloc::vec![
            encoded(&binding, binding.hello_frame().message),
            encoded(&binding, SessionMessage::Ready),
            encoded(&binding, SessionMessage::Accepted { sequence: 0 }),
            encoded(&binding, SessionMessage::Delivered { sequence: 0 }),
        ],
        next: 0,
        current: [0; MAXIMUM_CONDUITOS_WIRE_FRAME_BYTES],
        current_len: 0,
    };
    let mut source = ConduitOsWireSession::new(
        &mut source_io,
        &source_provider,
        binding.clone(),
        SessionRole::Source,
    )
    .unwrap();

    source.send(binding.hello_frame().message).unwrap();
    source
        .receive(|message| assert!(matches!(message, SessionMessage::Hello(_))))
        .unwrap();
    source.send(SessionMessage::Ready).unwrap();
    source
        .receive(|message| assert_eq!(message, SessionMessage::Ready))
        .unwrap();
    assert!(source.machine().is_active());

    source
        .send(SessionMessage::Offered {
            sequence: 0,
            payload: b"ordinary remote execution frame",
        })
        .unwrap();
    source
        .receive(|message| assert_eq!(message, SessionMessage::Accepted { sequence: 0 }))
        .unwrap();
    source
        .receive(|message| assert_eq!(message, SessionMessage::Delivered { sequence: 0 }))
        .unwrap();
    assert_eq!(source.machine().next_sequence(), 1);
    assert!(frames.sent.borrow().starts_with(b"CNDS"));
}

#[test]
fn provider_replacement_terminally_loses_the_line_without_reconnect() {
    let frames = SharedFrames::default();
    let mut io = FixtureProtected {
        shared: &frames,
        inbound: Vec::new(),
        next: 0,
        current: [0; MAXIMUM_CONDUITOS_WIRE_FRAME_BYTES],
        current_len: 0,
    };
    let provider = ProviderCell(Cell::new(PROVIDER));
    let binding = binding();
    let mut session =
        ConduitOsWireSession::new(&mut io, &provider, binding.clone(), SessionRole::Source)
            .unwrap();
    let mut replacement = PROVIDER;
    replacement.provider_generation += 1;
    provider.0.set(replacement);
    assert_eq!(
        session.send(binding.hello_frame().message),
        Err(ConduitOsWireSessionRefusal::ProviderLost)
    );
    provider.0.set(PROVIDER);
    assert_eq!(
        session.send(binding.hello_frame().message),
        Err(ConduitOsWireSessionRefusal::LineLost)
    );
    assert!(frames.sent.borrow().is_empty());
}

#[test]
fn line_loss_remains_distinct_from_wire_and_provider_failures() {
    let frames = SharedFrames::default();
    let mut io = FixtureProtected {
        shared: &frames,
        inbound: Vec::new(),
        next: 0,
        current: [0; MAXIMUM_CONDUITOS_WIRE_FRAME_BYTES],
        current_len: 0,
    };
    let provider = ProviderCell(Cell::new(PROVIDER));
    let binding = binding();
    let mut session =
        ConduitOsWireSession::new(&mut io, &provider, binding.clone(), SessionRole::Source)
            .unwrap();
    assert_eq!(
        session.receive(|_| ()),
        Err(ConduitOsWireSessionRefusal::LineLost)
    );
    assert_eq!(
        session.send(binding.hello_frame().message),
        Err(ConduitOsWireSessionRefusal::LineLost)
    );
}
