//! CPU conformance of the production mailbox and actual kernel. Activation is
//! an explicit unit fixture, not evidence of real Root/carrier admission.
use super::*;
use crate::{execution::StoredExecutionKernel, native_dispatch::Dispatch};
use conduit_kernel::{scheduler::*, *};
use core::convert::Infallible;
use std::vec;
struct Pins {
    lines: [c::AssignedLineForeRequirement; 2],
}
impl Pins {
    fn new(kind: &str) -> Self {
        let abi = c::assigned_fore_abi_digest(c::AssignedForeAbi {
            value_kind: kind,
            value_contract: None,
            abnormal_kind: None,
            temporal: "flow-open",
            track: c::ConnectionTrack::Payload,
            quantity_profile: c::QuantityValueProfile::Decimal,
            item_capacity: 1,
            byte_capacity: 8,
        });
        let binding = c::AssignedRemoteBinding {
            line: c::AssignedIdentity([1; 16]),
            local_host: c::AssignedIdentity([2; 16]),
            local_boot: c::AssignedIdentity([3; 16]),
            peer_host: c::AssignedIdentity([4; 16]),
            peer_boot: c::AssignedIdentity([5; 16]),
        };
        let make = |endpoint, direction| {
            c::AssignedLineForeRequirement::new(
                c::AssignedForeRequirement::new(
                    endpoint,
                    endpoint,
                    direction,
                    c::AssignedIdentity([6; 16]),
                    abi,
                ),
                binding,
                c::AssignedIdentity([7; 16]),
                c::AssignedIdentity([8; 16]),
                [9; 32],
            )
        };
        Self {
            lines: [
                make(0, c::AssignedForeDirection::Input),
                make(1, c::AssignedForeDirection::Output),
            ],
        }
    }
    fn specs<'a>(&'a self, kind: &'a str) -> (PrimitiveIngressSpec<'a>, EgressSpec<'a>) {
        (
            PrimitiveIngressSpec {
                pin: &self.lines[0],
                value_kind: kind,
                temporal: "flow-open",
                quantity_profile: c::QuantityValueProfile::Decimal,
                item_capacity: 1,
                byte_capacity: 8,
            },
            EgressSpec {
                pin: &self.lines[1],
                byte_capacity: 8,
            },
        )
    }
    fn activation(&self) -> ForeBridgeActivation {
        ForeBridgeActivation {
            custody: [4; 32],
            pins: self
                .lines
                .each_ref()
                .map(|line| c::assigned_plan_payload_digest(line.payload())),
        }
    }
}
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
type Kernel = StoredExecutionKernel<Echo, FixedSignLog<32>, 1, 2, 1, 1, 1, 0, 0>;
fn kernel(remote: u16) -> Kernel {
    let capacity = |slot_start| CordCapacity {
        slot_start,
        item_capacity: 1,
        byte_capacity: 8,
        pressure_policy: Default::default(),
    };
    let cords = [
        CordSpec::remote_ingress(
            CordId(0),
            RemoteEndpointId(0),
            (NodeId(0), PortId(0)),
            capacity(0),
        ),
        CordSpec::remote_egress(
            CordId(1),
            (NodeId(0), PortId(0)),
            RemoteEndpointId(1),
            capacity(1),
        ),
    ];
    let mut routes = FixedRoutes::new(1);
    routes
        .install(
            NodeId(0),
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(1),
                sink: CordEndpoint::Remote(RemoteEndpointId(1)),
            }],
        )
        .unwrap();
    routes.seal().unwrap();
    Kernel::new(
        [NodeSpec {
            input_cords: [Some(CordId(0))],
            maximum_step_fuel: 8,
        }],
        cords,
        routes,
        [Echo],
        HostedValueStore::new_with_slot_capacities(&[8, 8], 16).unwrap(),
        FixedSignLog::new_with_remote_storage(
            (32 * size_of::<KernelEvent>()) as u32,
            remote,
            remote_sign_storage_bytes(remote).unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}
struct Existing;
impl Observer<Kernel> for Existing {
    type Error = Infallible;
    fn before_step(&mut self, _: &mut Kernel) -> Result<(), Self::Error> {
        Ok(())
    }
    fn completed(&mut self, _: &mut Kernel, _: Dispatch) -> Result<(), Self::Error> {
        Ok(())
    }
}
fn delivered_input(port: &CarrierPort<'_, '_, '_>, sequence: u64) {
    assert_eq!(
        port.next_event().unwrap(),
        Some(Event::IngressAccepted(sequence))
    );
    port.sent(Event::IngressAccepted(sequence)).unwrap();
    assert_eq!(
        port.next_event().unwrap(),
        Some(Event::IngressDelivered(sequence))
    );
    port.sent(Event::IngressDelivered(sequence)).unwrap();
}
fn make_bound<'b, 'p>(
    pins: &'p Pins,
    input: &'b mut [u8],
    output: &'b mut [u8],
) -> BoundForeBridge<'b, 'p> {
    let (ingress, egress) = pins.specs(c::COUNT_INFO_ID);
    let prepared = PreparedForeBridge::prepare([4; 32], ingress, egress, input, output).unwrap();
    // Unit fixture: no actual carrier/provider is installed or advertised.
    unsafe { prepared.bind() }
}
#[test]
fn generic_primitive_validation_and_exact_activation_refuse_unproved_inputs() {
    for (kind, valid, invalid) in [
        (c::COUNT_INFO_ID, &[0u8; 8][..], &[0u8; 7][..]),
        ("value/bool", &[1u8][..], &[2u8][..]),
        (c::TEXT_INFO_ID, &b"ok"[..], &[0xffu8][..]),
    ] {
        let pins = Pins::new(kind);
        let (input_spec, output_spec) = pins.specs(kind);
        let mut input = [0; 8];
        let mut output = [0; 8];
        let bound = unsafe {
            PreparedForeBridge::prepare([4; 32], input_spec, output_spec, &mut input, &mut output)
                .unwrap()
                .bind()
        };
        let (_observer, port) = bound.activate(pins.activation()).unwrap();
        assert_eq!(port.stage_ingress(0, invalid), Err(Refusal::InvalidValue));
        port.stage_ingress(0, valid).unwrap();
    }
    let pins = Pins::new(c::COUNT_INFO_ID);
    let (mut input_spec, output_spec) = pins.specs(c::COUNT_INFO_ID);
    input_spec.byte_capacity = 7;
    let mut input = [0; 8];
    let mut output = [0; 8];
    assert!(matches!(
        PreparedForeBridge::prepare([4; 32], input_spec, output_spec, &mut input, &mut output),
        Err(Refusal::UnsupportedAbi)
    ));
    let pins = Pins::new("unsupported/structured");
    let (input_spec, output_spec) = pins.specs("unsupported/structured");
    assert!(matches!(
        PreparedForeBridge::prepare([4; 32], input_spec, output_spec, &mut input, &mut output),
        Err(Refusal::UnsupportedAbi)
    ));
    let pins = Pins::new(c::COUNT_INFO_ID);
    let bound = make_bound(&pins, &mut input, &mut output);
    let mut activation = pins.activation();
    activation.custody[0] ^= 1;
    assert!(matches!(bound.activate(activation), Err(Refusal::Scope)));
    let mut activation = pins.activation();
    activation.pins[1][0] ^= 1;
    assert!(matches!(bound.activate(activation), Err(Refusal::Scope)));
    bound.activate(pins.activation()).unwrap();
    assert!(matches!(
        bound.activate(pins.activation()),
        Err(Refusal::Inactive)
    ));
}
#[test]
fn constrained_abi_refuses_and_preparation_and_pumping_allocate_nothing() {
    let mut pins = Pins::new(c::COUNT_INFO_ID);
    let contract = c::CheckedValueContract::new(
        c::COUNT_INFO_ID.into(),
        8,
        vec![c::ValueConstraint::UnsignedRange {
            minimum: Some(1),
            maximum: Some(3),
            minimum_endpoint: c::IntervalEndpoint::Inclusive,
            maximum_endpoint: c::IntervalEndpoint::Inclusive,
        }],
    )
    .unwrap();
    let digest = c::assigned_fore_abi_digest(c::AssignedForeAbi {
        value_kind: c::COUNT_INFO_ID,
        value_contract: Some(&contract),
        abnormal_kind: None,
        temporal: "flow-open",
        track: c::ConnectionTrack::Payload,
        quantity_profile: c::QuantityValueProfile::Decimal,
        item_capacity: 1,
        byte_capacity: 8,
    });
    let binding = c::AssignedRemoteBinding {
        line: c::AssignedIdentity([1; 16]),
        local_host: c::AssignedIdentity([2; 16]),
        local_boot: c::AssignedIdentity([3; 16]),
        peer_host: c::AssignedIdentity([4; 16]),
        peer_boot: c::AssignedIdentity([5; 16]),
    };
    pins.lines[0] = c::AssignedLineForeRequirement::new(
        c::AssignedForeRequirement::new(
            0,
            0,
            c::AssignedForeDirection::Input,
            c::AssignedIdentity([6; 16]),
            digest,
        ),
        binding,
        c::AssignedIdentity([7; 16]),
        c::AssignedIdentity([8; 16]),
        [9; 32],
    );
    let (input_spec, output_spec) = pins.specs(c::COUNT_INFO_ID);
    let mut input = [0; 8];
    let mut output = [0; 8];
    assert!(matches!(
        PreparedForeBridge::prepare([4; 32], input_spec, output_spec, &mut input, &mut output),
        Err(Refusal::UnsupportedAbi)
    ));
    let pins = Pins::new(c::COUNT_INFO_ID);
    let (bound, cost) = crate::member_return_tests::return_allocation_probe::observe(|| {
        make_bound(&pins, &mut input, &mut output)
    });
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    let (observer, port) = bound.activate(pins.activation()).unwrap();
    let mut observer = ForeObserver::new(Existing, observer);
    let mut kernel = kernel(16);
    let (_, cost) = crate::member_return_tests::return_allocation_probe::observe(|| {
        port.stage_ingress(0, &1u64.to_le_bytes()).unwrap();
        observer.before_step(&mut kernel).unwrap();
        delivered_input(&port, 0);
        kernel.step().unwrap();
        observer.before_step(&mut kernel).unwrap();
        port.sent(Event::EgressOffered {
            sequence: 0,
            length: 8,
        })
        .unwrap();
        port.peer_accepted(0).unwrap();
        port.peer_delivered(0).unwrap();
        observer.before_step(&mut kernel).unwrap();
    });
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
}

#[test]
fn genuine_peer_acknowledgments_drive_kernel_custody_and_pressure_keeps_same_value() {
    let pins = Pins::new(c::COUNT_INFO_ID);
    let mut input = [0; 8];
    let mut output = [0; 8];
    let bound = make_bound(&pins, &mut input, &mut output);
    let (observer, port) = bound.activate(pins.activation()).unwrap();
    let mut observer = ForeObserver::new(Existing, observer);
    let mut kernel = kernel(16);
    port.stage_ingress(0, &1u64.to_le_bytes()).unwrap();
    observer.before_step(&mut kernel).unwrap();
    delivered_input(&port, 0);
    kernel.step().unwrap();
    observer.before_step(&mut kernel).unwrap();
    let event = Event::EgressOffered {
        sequence: 0,
        length: 8,
    };
    assert_eq!(port.next_event().unwrap(), Some(event));
    port.with_egress(0, |bytes| assert_eq!(bytes, &1u64.to_le_bytes()))
        .unwrap();
    port.sent(event).unwrap();
    assert_eq!(port.peer_delivered(0), Err(Refusal::Sequence));
    assert_eq!(port.peer_accepted(1), Err(Refusal::Sequence));
    port.peer_pressure(0).unwrap();
    assert_eq!(port.next_event().unwrap(), Some(event));
    assert_eq!(kernel.cord_usage(CordId(1)).unwrap(), (1, 8));
    port.sent(event).unwrap();
    port.peer_accepted(0).unwrap();
    assert_eq!(port.peer_accepted(0), Err(Refusal::Sequence));
    assert_eq!(kernel.cord_usage(CordId(1)).unwrap(), (1, 8));
    observer.before_step(&mut kernel).unwrap();
    assert_eq!(kernel.cord_usage(CordId(1)).unwrap(), (1, 8));
    port.peer_delivered(0).unwrap();
    assert_eq!(kernel.cord_usage(CordId(1)).unwrap(), (1, 8));
    observer.before_step(&mut kernel).unwrap();
    assert_eq!(kernel.cord_usage(CordId(1)).unwrap(), (0, 0));
    assert_eq!(kernel.values().used_items(), 0);
    assert_eq!(port.peer_delivered(0), Err(Refusal::Sequence));
}
#[test]
fn queue_pressure_retains_ingress_sequence_and_never_claims_delivery() {
    let pins = Pins::new(c::COUNT_INFO_ID);
    let mut input = [0; 8];
    let mut output = [0; 8];
    let bound = make_bound(&pins, &mut input, &mut output);
    let (observer, port) = bound.activate(pins.activation()).unwrap();
    let mut observer = ForeObserver::new(Existing, observer);
    let mut kernel = kernel(16);
    port.stage_ingress(0, &0u64.to_le_bytes()).unwrap();
    observer.before_step(&mut kernel).unwrap();
    delivered_input(&port, 0);
    port.stage_ingress(1, &1u64.to_le_bytes()).unwrap();
    observer.before_step(&mut kernel).unwrap();
    assert_eq!(port.next_event().unwrap(), Some(Event::IngressPressure(1)));
    assert_eq!(
        port.sent(Event::IngressDelivered(1)),
        Err(Refusal::Sequence)
    );
    port.sent(Event::IngressPressure(1)).unwrap();
    kernel.step().unwrap();
    port.stage_ingress(1, &1u64.to_le_bytes()).unwrap();
    observer.before_step(&mut kernel).unwrap();
    delivered_input(&port, 1);
    assert_eq!(kernel.cord_usage(CordId(0)).unwrap(), (1, 8));
}
#[test]
fn sign_pressure_retains_real_delivered_ack_and_retirement_leaves_kernel_value_owned() {
    let pins = Pins::new(c::COUNT_INFO_ID);
    let mut input = [0; 8];
    let mut output = [0; 8];
    let bound = make_bound(&pins, &mut input, &mut output);
    let (observer, port) = bound.activate(pins.activation()).unwrap();
    let mut observer = ForeObserver::new(Existing, observer);
    let mut kernel = kernel(3);
    port.stage_ingress(0, &0u64.to_le_bytes()).unwrap();
    observer.before_step(&mut kernel).unwrap();
    delivered_input(&port, 0);
    kernel.step().unwrap();
    observer.before_step(&mut kernel).unwrap();
    port.sent(Event::EgressOffered {
        sequence: 0,
        length: 8,
    })
    .unwrap();
    port.peer_accepted(0).unwrap();
    port.peer_delivered(0).unwrap();
    observer.before_step(&mut kernel).unwrap();
    observer.before_step(&mut kernel).unwrap();
    assert_eq!(kernel.cord_usage(CordId(1)).unwrap(), (1, 8));
    assert_eq!(kernel.signs().remote_len(), 3);
    // Neither a held mailbox nor IO retirement can cancel the independent kernel.
    {
        let _held = bound.0.shared.borrow_mut();
        observer.before_step(&mut kernel).unwrap();
    }
    port.retire().unwrap();
    assert_eq!(port.stage_ingress(1, &[0; 8]), Err(Refusal::Inactive));
    assert_eq!(port.peer_delivered(0), Err(Refusal::Inactive));
    observer.before_step(&mut kernel).unwrap();
    assert!(kernel
        .remote_egress_offer(RemoteEndpointId(1), CordId(1))
        .unwrap()
        .is_some());
}

#[test]
fn dropping_either_actor_port_fences_the_bridge_even_during_a_mailbox_loan() {
    let pins = Pins::new(c::COUNT_INFO_ID);
    let mut input = [0; 8];
    let mut output = [0; 8];
    let bound = make_bound(&pins, &mut input, &mut output);
    let (observer, port) = bound.activate(pins.activation()).unwrap();
    let mut observer = ForeObserver::new(Existing, observer);
    let mut kernel = kernel(16);
    port.stage_ingress(0, &1u64.to_le_bytes()).unwrap();
    {
        let _held = bound.0.shared.borrow_mut();
        drop(port); // Cancellation fencing needs no RefCell loan or retry.
    }
    observer.before_step(&mut kernel).unwrap();
    assert_eq!(kernel.cord_usage(CordId(0)).unwrap(), (0, 0));
    drop(observer);
    let pins = Pins::new(c::COUNT_INFO_ID);
    let mut input = [0; 8];
    let mut output = [0; 8];
    let bound = make_bound(&pins, &mut input, &mut output);
    let (observer, port) = bound.activate(pins.activation()).unwrap();
    drop(observer);
    assert_eq!(port.stage_ingress(0, &[0; 8]), Err(Refusal::Inactive));
}
