//! Native pins, authority and the sealed selection are CPU fixtures. The real
//! wire engine and exact canonical Source packet encoder are exercised together.
#![cfg(feature = "host-call")]
extern crate alloc;
use conduit_core::*;
use conduit_kernel::{HostCallId, NodeId, RequestId};
use conduit_plan_lowering::selected_operation::SelectedOperationPlan;
use conduit_tm1637_wire::{owner::*, *};
use std::{cell::Cell, rc::Rc};
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[path = "support/fixture.rs"]
mod fixture;
#[path = "../../../../targets/conduitos/src/machine_membrane/selection_fixture.rs"]
mod selection_fixture;
#[derive(Default)]
struct State {
    edges: Cell<usize>,
    released: Cell<[bool; 2]>,
    nack: Cell<bool>,
    failed: Cell<bool>,
}
struct Provider(Rc<State>);
impl OpenDrain for Provider {
    type Error = &'static str;
    fn clock(&mut self, released: bool) -> Result<(), Self::Error> {
        self.0.edges.set(self.0.edges.get() + 1);
        let mut levels = self.0.released.get();
        levels[0] = released;
        self.0.released.set(levels);
        Ok(())
    }
    fn data(&mut self, released: bool) -> Result<(), Self::Error> {
        self.0.edges.set(self.0.edges.get() + 1);
        let mut levels = self.0.released.get();
        levels[1] = released;
        self.0.released.set(levels);
        Ok(())
    }
    fn read_data(&mut self) -> Result<bool, Self::Error> {
        Ok(self.0.nack.get())
    }
    fn half_period(&mut self) -> Result<(), Self::Error> {
        if self.0.failed.get() {
            Err("provider-lost")
        } else {
            Ok(())
        }
    }
}
fn encoded() -> Vec<u8> {
    conduit_environment_runtime::display_packets::PreparedDisplayPackets::prepare()
        .unwrap()
        .encode_canonical([0x7d, 0x87, 0x7d, 0x71, 0, 0], 3, true)
        .unwrap()
        .to_vec()
}
#[test]
fn exact_source_packets_are_authorized_sequenced_and_allocation_free() {
    let state = Rc::new(State::default());
    let (mut owner, node) = fixture::owner(Provider(state.clone()));
    let bytes = encoded();
    let ((), memory) = allocation_probe::observe(|| {
        assert_eq!(
            owner.invoke(
                NodeId(node.0 + 1),
                HostCallId(0),
                RequestId(0),
                &bytes,
                || false
            ),
            Err(WriteCallRefusal::WrongBinding)
        );
        assert_eq!(
            owner.invoke(node, HostCallId(1), RequestId(0), &bytes, || false),
            Err(WriteCallRefusal::WrongBinding)
        );
        assert!(matches!(
            owner.invoke(
                node,
                HostCallId(0),
                RequestId(0),
                &bytes[..bytes.len() - 1],
                || false
            ),
            Err(WriteCallRefusal::Packets(_))
        ));
        assert_eq!(state.edges.get(), 0);
        owner
            .invoke(node, HostCallId(0), RequestId(0), &bytes, || false)
            .unwrap();
        let edges = state.edges.get();
        assert!(edges > 200);
        assert_eq!(state.released.get(), [true; 2]);
        assert_eq!(
            owner.invoke(node, HostCallId(0), RequestId(0), &bytes, || false),
            Err(WriteCallRefusal::StaleRequest)
        );
        assert_eq!(state.edges.get(), edges);
        assert_eq!(owner.inspection().completed_operations, 1);
        owner.revoke(node, HostCallId(0)).unwrap();
        assert!(matches!(
            owner.invoke(node, HostCallId(0), RequestId(1), &bytes, || false),
            Err(WriteCallRefusal::Capability(_))
        ));
        assert_eq!(state.edges.get(), edges);
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}
#[test]
fn nack_provider_loss_and_cancellation_release_revoke_and_never_retry() {
    let bytes = encoded();
    for mode in 0..3 {
        let state = Rc::new(State::default());
        state.nack.set(mode == 0);
        state.failed.set(mode == 1);
        let (mut owner, node) = fixture::owner(Provider(state.clone()));
        let result = owner.invoke(node, HostCallId(0), RequestId(0), &bytes, || mode == 2);
        match mode {
            0 => assert_eq!(
                result,
                Err(WriteCallRefusal::Electrical(Failure::Nack {
                    packet: 0,
                    byte: 0
                }))
            ),
            1 => assert_eq!(
                result,
                Err(WriteCallRefusal::Electrical(Failure::Provider(
                    "provider-lost"
                )))
            ),
            _ => assert_eq!(
                result,
                Err(WriteCallRefusal::Electrical(Failure::Cancelled))
            ),
        }
        assert_eq!(state.released.get(), [true; 2]);
        assert_eq!(owner.inspection().completed_operations, 1);
        assert_eq!(owner.inspection().in_flight_operations, 0);
        assert_eq!(owner.inspection().lifecycle, CapabilityLifecycle::Revoked);
        let edges = state.edges.get();
        assert!(owner
            .invoke(node, HostCallId(0), RequestId(1), &bytes, || false)
            .is_err());
        assert_eq!(state.edges.get(), edges);
        let mut retained = Some(owner);
        let (returned, attachment) = WriteHostCall::retire_retained(&mut retained).unwrap();
        assert!(retained.is_none());
        assert!(Rc::ptr_eq(&returned.0, &state));
        assert_eq!(attachment.generation, 7);
    }
}
#[test]
fn substituted_play_and_semantic_ports_refuse_before_pin_effects() {
    let (mut fragment, _, mut active, placement) = fixture::selected();
    let state = Rc::new(State::default());
    active.active_play_id = "play/foreign".into();
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(&fragment).unwrap();
    assert!(matches!(
        fixture::bind(
            &fragment,
            &lowered,
            &active,
            &placement,
            Provider(state.clone())
        ),
        Err(WriteCallRefusal::WrongBinding)
    ));
    fragment.placements[0].inputs[0].port_id = port_id("substituted");
    let identity = PlotIdentity {
        source_document_id: fragment.source_document_id.clone(),
        checked_plot_id: fragment.checked_plot_id.clone(),
        expanded_plot_id: fragment.expanded_plot_id.clone(),
    };
    fragment = seal_plan(identity, vec![fragment]).fragments.remove(0);
    active = bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(&fragment).unwrap();
    assert!(matches!(
        fixture::bind(
            &fragment,
            &lowered,
            &active,
            &placement,
            Provider(state.clone())
        ),
        Err(WriteCallRefusal::WrongBinding)
    ));
    assert_eq!(state.edges.get(), 0);
}

#[test]
fn canonical_but_invalid_packet_lengths_and_padding_refuse_before_authorization() {
    let original = encoded();
    let value = validate_canonical_structured_value(&original).unwrap();
    let packet = value
        .record_field("packets")
        .unwrap()
        .unwrap()
        .collection_index(0)
        .unwrap()
        .unwrap();
    let length = packet
        .record_field("length")
        .unwrap()
        .unwrap()
        .primitive_bytes("value/u8")
        .unwrap();
    let padding = packet
        .record_field("wire")
        .unwrap()
        .unwrap()
        .collection_index(6)
        .unwrap()
        .unwrap()
        .primitive_bytes("value/u8")
        .unwrap();
    let offsets = [
        length.as_ptr() as usize - original.as_ptr() as usize,
        padding.as_ptr() as usize - original.as_ptr() as usize,
    ];
    let state = Rc::new(State::default());
    let (mut owner, node) = fixture::owner(Provider(state.clone()));
    for (offset, byte) in [(offsets[0], 0), (offsets[0], 8), (offsets[1], 1)] {
        let mut malformed = original.clone();
        malformed[offset] = byte;
        // Still exact canonical Tm1637Packets; its electrical geometry is invalid.
        validate_canonical_structured_value(&malformed).unwrap();
        assert_eq!(
            owner.invoke(node, HostCallId(0), RequestId(0), &malformed, || false),
            Err(WriteCallRefusal::Packets(PacketRefusal::Geometry))
        );
        assert_eq!(state.edges.get(), 0);
        assert_eq!(owner.inspection().completed_operations, 0);
    }
    owner
        .invoke(node, HostCallId(0), RequestId(0), &original, || false)
        .unwrap();
}
