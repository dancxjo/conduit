//! Actual Root admission/Start, actual framed IO and a real independent Wire peer.
extern crate std;
use super::super::tests::{self as root_test, docs as documents};
use super::*;
use crate::{
    execution::{PreparedBacks, PreparedFragment},
    execution_workset::{AssignedPreparationProfile, PreparedKernelGroup, PreparedLocalWorkset},
    fore_bridge::{ForeObserver, PrimitiveIngressSpec, StructuredIngressSpec},
    native_dispatch::Dispatch,
    native_runner::Observer,
};
use conduit_kernel::{scheduler::*, *};
use core::{cell::RefCell, convert::Infallible};
use std::{vec, vec::Vec};
#[allow(dead_code)]
#[path = "numeric.rs"]
mod numeric;
#[path = "peer.rs"]
mod peer;
use numeric::TablesFixture;
#[derive(Clone, Copy)]
struct Echo;
impl StepBack<1> for Echo {
    fn step(&mut self, io: &mut StepIo<1>, _: &StepInputBytes<'_, 1>) -> StepOutcome {
        if io.input(PortId(0)).is_some() {
            let value = io.consume(PortId(0)).unwrap();
            io.send(PortId(0), value).unwrap();
            StepOutcome::Progress
        } else {
            StepOutcome::Await
        }
    }
}
struct Backs;
// SAFETY: CPU-only Echo exactly implements this fixture's independently checked
// one-node route and immutable empty terminal report. No hardware is exercised.
unsafe impl PreparedBacks<1, 1> for Backs {
    type Driver = Echo;
    type Calls = ();
    fn terminal_transductions(&self) -> Option<[[Option<AssignedTerminalTransduction>; 1]; 1]> {
        Some([[None]])
    }
    fn retained_backing_bytes(&self) -> Option<usize> {
        Some(0)
    }
    fn bind(self) -> ([Echo; 1], ()) {
        ([Echo], ())
    }
}
type Kernel = crate::execution::ExecutionKernel<Echo, 1, 2, 1, 1, 1, 1, 32>;
struct Existing;
impl Observer<Kernel> for Existing {
    type Error = Infallible;
    fn before_step(&mut self, _: &mut Kernel) -> Result<(), Infallible> {
        Ok(())
    }
    fn completed(&mut self, _: &mut Kernel, _: Dispatch) -> Result<(), Infallible> {
        Ok(())
    }
}
fn bounds() -> ForeCarrierBounds {
    ForeCarrierBounds {
        maximum_lines: 2,
        maximum_frame_bytes: 128,
        maximum_payload_bytes: 8,
        maximum_buffered_bytes: 128,
        maximum_in_flight_items: 1,
        maximum_frames: 64,
    }
}
fn line(output: bool) -> c::AdmittedLine {
    let mut line = root_test::actual_line();
    line.binding.limits = c::LinkLimits {
        maximum_in_flight_items: 1,
        maximum_payload_bytes: 8,
        maximum_buffered_bytes: 8,
        maximum_frame_bytes: 128,
    };
    if output {
        line.line_id = "line/out".into();
        line.binding.binding_id = "binding/out".into();
        core::mem::swap(&mut line.binding.source, &mut line.binding.sink);
    }
    line
}
fn pin(line: &c::AdmittedLine, output: bool) -> c::AssignedLineForeRequirement {
    pin_with_kind(line, output, c::COUNT_INFO_ID)
}
fn pin_with_kind(
    line: &c::AdmittedLine,
    output: bool,
    kind: &str,
) -> c::AssignedLineForeRequirement {
    pin_with_abi(line, output, kind, 8)
}
fn pin_with_abi(
    line: &c::AdmittedLine,
    output: bool,
    kind: &str,
    capacity: u32,
) -> c::AssignedLineForeRequirement {
    let (local, peer) = if output {
        (&line.binding.source, &line.binding.sink)
    } else {
        (&line.binding.sink, &line.binding.source)
    };
    let abi = c::assigned_fore_abi_digest(c::AssignedForeAbi {
        value_kind: kind,
        value_contract: None,
        abnormal_kind: None,
        temporal: "flow-open",
        track: c::ConnectionTrack::Payload,
        quantity_profile: c::QuantityValueProfile::Decimal,
        item_capacity: 1,
        byte_capacity: capacity,
    });
    c::AssignedLineForeRequirement::new(
        c::AssignedForeRequirement::new(
            u16::from(output),
            u16::from(output),
            if output {
                c::AssignedForeDirection::Output
            } else {
                c::AssignedForeDirection::Input
            },
            documents::id(if output { "read_result" } else { "read_index" }),
            abi,
        ),
        c::AssignedRemoteBinding {
            line: documents::id(line.line_id.as_str()),
            local_host: documents::id(local.host_id.as_str()),
            local_boot: documents::id(local.boot_id.as_str()),
            peer_host: documents::id(peer.host_id.as_str()),
            peer_boot: documents::id(peer.boot_id.as_str()),
        },
        documents::id("session"),
        documents::id("serial-profile"),
        c::admitted_line_commitment(line),
    )
}
fn numeric(pins: &[c::AssignedLineForeRequirement; 2]) -> TablesFixture {
    let mut n = TablesFixture::new([true, true]);
    n.lines = pins.to_vec();
    n.local.clear();
    n.bindings = vec![
        c::AssignedRemoteBinding {
            line: documents::id("line"),
            local_host: documents::id("host/1"),
            local_boot: documents::id("boot/1"),
            peer_host: documents::id("issuer/1"),
            peer_boot: documents::id("issuer/boot/1"),
        },
        c::AssignedRemoteBinding {
            line: documents::id("line/out"),
            local_host: documents::id("host/1"),
            local_boot: documents::id("boot/1"),
            peer_host: documents::id("issuer/1"),
            peer_boot: documents::id("issuer/boot/1"),
        },
    ];
    n.records
        .retain(|(tag, _)| *tag != c::ASSIGNED_REMOTE_ENDPOINT);
    for (_, cord) in n
        .records
        .iter_mut()
        .filter(|(tag, _)| *tag == c::ASSIGNED_CORD)
    {
        cord[32..36].copy_from_slice(&8u32.to_le_bytes());
        cord[37..41].copy_from_slice(&8u32.to_le_bytes());
    }
    for pin in pins {
        n.records
            .push((c::ASSIGNED_REMOTE_ENDPOINT, pin.payload().to_vec()));
    }
    n.reseal();
    n
}
#[test]
fn original_bound_root_carrier_exchanges_real_scoped_payloads_then_isolates_io_failure() {
    let mut f = documents::Fixture::new();
    f.host.profile = "config/1".into();
    let seed = root_test::seed(&f);
    let lines = [line(false), line(true)];
    let pins = [pin(&lines[0], false), pin(&lines[1], true)];
    let n = numeric(&pins);
    let inventory: Vec<u8> = pins
        .iter()
        .flat_map(|p| p.fore().payload().iter().copied())
        .collect();
    let document = documents::document(&n.bytes, "body/1", "boot/1", &inventory, &[]);
    let mut root = root_test::root(&f, &seed, &document);
    let peer = RefCell::new(None);
    let mut transport = TestTransport(&peer);
    let mut frame = [0; 128];
    let mut input = [0; 8];
    let mut output = [0; 8];
    let selection = unsafe {
        NativeForeCarrierSelection::from_native_custody(
            &mut transport,
            &mut frame,
            "image/1",
            "config/1",
            &f.issuer,
            n.profile(),
            &lines,
            "session",
            "serial-profile",
            bounds(),
        )
    }
    .unwrap();
    let mut selection = Some(selection);
    let parts = root.execution_parts().unwrap();
    let context = b::NativePreparationContext {
        bootstrap: parts.bootstrap,
        member: parts.member,
        actual: &f.host,
        image_id: "image/1",
        configuration_id: "config/1",
        now_millis: 11,
    };
    let policy = crate::execution::Policy {
        preparation: Some(&root_test::PREP),
        execution: Some(&root_test::EXEC),
        active_worksets: 0,
        maximum_retained_bytes: 65536,
    };
    let prepared=PreparedLocalWorkset::<_,1>::prepare_selected_profiles::<4096,2,ForeIoRefusal>(parts.workspace,context,policy,|_,_|Ok(AssignedPreparationProfile::LineFore(n.preparation())),|approved,fore|{
        let carrier=parts.prepare_fore_carrier::<TestTransport<'_>,1,2>(11,selection.take(),&approved[0],&fore[0],n.preparation()).map_err(|_|ForeIoRefusal::Scope)?;
        let ingress=PrimitiveIngressSpec::checked(&pins[0], c::AssignedForeAbi {value_kind:c::COUNT_INFO_ID, value_contract:None, abnormal_kind:None, temporal:"flow-open",track:c::ConnectionTrack::Payload,quantity_profile:c::QuantityValueProfile::Decimal,item_capacity:1,byte_capacity:8}).unwrap();let egress=EgressSpec::checked(&pins[1],c::AssignedForeAbi{value_kind:c::COUNT_INFO_ID,value_contract:None,abnormal_kind:None,temporal:"flow-open",track:c::ConnectionTrack::Payload,quantity_profile:c::QuantityValueProfile::Decimal,item_capacity:1,byte_capacity:8}).unwrap();
        let bridge=PreparedForeBridge::prepare(*carrier.custody_digest(),ingress,egress,&mut input,&mut output).unwrap();let sessions=carrier.prepare_bridge_sessions(&bridge,ingress,egress)?;*peer.borrow_mut()=Some(peer::Peer::new(sessions.routes.each_ref().map(|r|r.machine.binding().clone())));
        let kernel=PreparedFragment::<Backs,1,2,1,1,1,1,32>::prepare_with_line_fore_terminal_transductions_ref(&approved[0],n.preparation(),&n.layout(),HostedValueStore::new(4,8,32).unwrap(),FixedSignLog::new_with_remote_storage((32*size_of::<KernelEvent>())as u32,8,remote_sign_storage_bytes(8).unwrap()).unwrap(),65536,||Ok::<_,()>(Backs),&[[None]]).map_err(|_|ForeIoRefusal::Bounds)?;
        Ok(carrier.attach(PreparedKernelGroup(kernel,bridge,sessions)))
    }).unwrap();
    assert_eq!(peer.borrow().as_ref().unwrap().writes, 0);
    let mut committed = prepared
        .admit::<ForeIoRefusal>(parts.resources, &[], 0)
        .unwrap();
    let mut started = committed
        .start::<4096, ForeIoRefusal>(parts.workspace, context, policy, parts.resources)
        .unwrap();
    let evidence = started.evidence();
    let (native, observer, mut actor) = started.kernels_mut().activate_actor(&evidence).unwrap();
    let kernel: &mut Kernel = native.0.as_mut();
    let mut observer = ForeObserver::new(Existing, observer);
    for _ in 0..8 {
        if peer.borrow().as_ref().unwrap().queue.is_empty() {
            crate::tests::ready(actor.send_pending()).unwrap();
        } else {
            crate::tests::ready(actor.receive_one()).unwrap();
        }
    }
    assert_eq!(actor.frames_used(), 8);
    peer.borrow_mut().as_mut().unwrap().emit(
        0,
        Message::Message(SessionMessage::Offered {
            sequence: 0,
            payload: &c::encode_count(7),
        }),
    );
    crate::tests::ready(actor.receive_one()).unwrap();
    let frames = actor.frames_used();
    crate::tests::ready(actor.send_pending()).unwrap();
    assert_eq!(actor.frames_used(), frames);
    observer.before_step(kernel).unwrap();
    kernel.step().unwrap();
    observer.before_step(kernel).unwrap();
    // A real successful Accepted write is followed by a contested synchronous
    // APPLY. Keeping this exact pending phase prevents a duplicate frame.
    crate::tests::ready(actor.send_pending()).unwrap();
    let frames_after_send = actor.frames_used();
    let writes_after_send = peer.borrow().as_ref().unwrap().writes;
    assert_eq!(
        observer.with_test_mailbox_loan(|| actor.apply()),
        Ok(ForeIoProgress::Busy)
    );
    assert!(matches!(
        actor.state.pending,
        Pending::Sent(Event::IngressAccepted(0))
    ));
    assert_eq!(
        crate::tests::ready(actor.send_pending()),
        Ok(ForeIoProgress::Progress)
    );
    assert_eq!(actor.frames_used(), frames_after_send);
    assert_eq!(peer.borrow().as_ref().unwrap().writes, writes_after_send);
    for _ in 0..12 {
        if !matches!(actor.state.pending, Pending::None) {
            crate::tests::ready(actor.send_pending()).unwrap();
        } else if !peer.borrow().as_ref().unwrap().queue.is_empty() {
            crate::tests::ready(actor.receive_one()).unwrap();
        } else if actor.port.next_event().unwrap().is_some() {
            crate::tests::ready(actor.send_pending()).unwrap();
        } else {
            break;
        }
        observer.before_step(kernel).unwrap();
    }
    assert_eq!(
        peer.borrow().as_ref().unwrap().delivered,
        [c::encode_count(7).to_vec()]
    );
    assert!(kernel
        .remote_egress_offer(RemoteEndpointId(1), CordId(1))
        .unwrap()
        .is_none());
    peer.borrow_mut().as_mut().unwrap().emit(
        0,
        Message::Message(SessionMessage::Offered {
            sequence: 1,
            payload: &c::encode_count(8),
        }),
    );
    crate::tests::ready(actor.receive_one()).unwrap();
    crate::tests::ready(actor.send_pending()).unwrap();
    observer.before_step(kernel).unwrap();
    kernel.step().unwrap();
    observer.before_step(kernel).unwrap();
    for _ in 0..4 {
        crate::tests::ready(actor.send_pending()).unwrap();
    } // actual ingress Accepted/Delivered plus their APPLYs
    peer.borrow_mut().as_mut().unwrap().fail = true;
    assert_eq!(
        crate::tests::ready(actor.send_pending()),
        Err(ForeIoRefusal::Io)
    );
    assert!(kernel
        .remote_egress_offer(RemoteEndpointId(1), CordId(1))
        .unwrap()
        .is_some());
    observer.before_step(kernel).unwrap();
    kernel.step().unwrap(); // independent kernel remains usable
}
struct TestTransport<'a>(&'a RefCell<Option<peer::Peer>>);
impl MessageTransport for TestTransport<'_> {
    type Error = ();
    async fn receive<'a>(&mut self, output: &'a mut [u8]) -> Result<&'a [u8], ()> {
        let bytes = self
            .0
            .borrow_mut()
            .as_mut()
            .ok_or(())?
            .queue
            .pop_front()
            .ok_or(())?;
        output[..bytes.len()].copy_from_slice(&bytes);
        Ok(&output[..bytes.len()])
    }
    async fn send(&mut self, bytes: &[u8]) -> Result<(), ()> {
        let mut state = self.0.borrow_mut();
        let peer = state.as_mut().ok_or(())?;
        if peer.fail {
            return Err(());
        }
        peer.writes += 1;
        let frame = conduit_wire::decode_external_fore_session_frame(bytes, 8, 128).unwrap();
        let index = peer
            .machines
            .iter()
            .position(|m| m.binding().scope_commitment == frame.scope_commitment)
            .unwrap();
        peer.machines[index].admit_inbound(frame).unwrap();
        match frame.message {
            Message::Hello => peer.emit(index, Message::Hello),
            Message::Message(SessionMessage::Ready) => {
                peer.emit(index, Message::Message(SessionMessage::Ready))
            }
            Message::Message(SessionMessage::Offered { sequence, payload }) => {
                peer.delivered.push(payload.to_vec());
                peer.emit(
                    index,
                    Message::Message(SessionMessage::Accepted { sequence }),
                );
                peer.emit(
                    index,
                    Message::Message(SessionMessage::Delivered { sequence }),
                );
            }
            _ => {}
        }
        Ok(())
    }
}

#[path = "preparation_tests.rs"]
mod preparation;
