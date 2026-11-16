//! CPU conformance of the production mailbox and actual kernel. Activation is
//! an explicit unit fixture, not evidence of real Root/carrier admission.
use super::*;
use crate::{execution::StoredExecutionKernel, native_dispatch::Dispatch};
use conduit_kernel::{scheduler::*, *};
use core::convert::Infallible;
use std::vec;
fn ingress_abi(kind: &str, byte_capacity: u32) -> c::AssignedForeAbi<'_> {
    c::AssignedForeAbi {
        value_kind: kind,
        value_contract: None,
        abnormal_kind: None,
        temporal: "flow-open",
        track: c::ConnectionTrack::Payload,
        quantity_profile: c::QuantityValueProfile::Decimal,
        item_capacity: 1,
        byte_capacity,
    }
}
struct Pins {
    lines: [c::AssignedLineForeRequirement; 2],
}
impl Pins {
    fn new(kind: &str) -> Self {
        Self::with_capacity(kind, 8)
    }
    fn with_capacity(kind: &str, byte_capacity: u32) -> Self {
        Self::from_abi(ingress_abi(kind, byte_capacity))
    }
    fn from_abi(abi: c::AssignedForeAbi<'_>) -> Self {
        let abi = c::assigned_fore_abi_digest(abi);

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
            PrimitiveIngressSpec::checked(
                &self.lines[0],
                c::AssignedForeAbi {
                    value_kind: kind,
                    value_contract: None,
                    abnormal_kind: None,
                    temporal: "flow-open",
                    track: c::ConnectionTrack::Payload,
                    quantity_profile: c::QuantityValueProfile::Decimal,
                    item_capacity: 1,
                    byte_capacity: 8,
                },
            )
            .unwrap(),
            EgressSpec::checked(
                &self.lines[1],
                c::AssignedForeAbi {
                    value_kind: kind,
                    value_contract: None,
                    abnormal_kind: None,
                    temporal: "flow-open",
                    track: c::ConnectionTrack::Payload,
                    quantity_profile: c::QuantityValueProfile::Decimal,
                    item_capacity: 1,
                    byte_capacity: 8,
                },
            )
            .unwrap(),
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
        } else if io.input_closed(PortId(0)) {
            io.consume_closed(PortId(0)).unwrap();
            StepOutcome::Complete
        } else {
            StepOutcome::Await
        }
    }
}
type ProfileKernel<
    const M: usize,
    const F: usize,
    const U: usize,
    const Z: usize,
    L = FixedSignLog<32>,
> = StoredExecutionKernel<Echo, L, 1, 2, 1, 1, 1, 0, 1, HostedValueStore, 2, M, F, U, Z>;
type Kernel = ProfileKernel<1, 100, 1, 2>;
fn kernel(remote: u16) -> Kernel {
    kernel_with_capacity(remote, 8)
}
fn kernel_with_capacity(remote: u16, byte_capacity: u32) -> Kernel {
    profile_kernel(remote, byte_capacity)
}
fn profile_kernel<const M: usize, const F: usize, const U: usize, const Z: usize>(
    remote: u16,
    byte_capacity: u32,
) -> ProfileKernel<M, F, U, Z> {
    profile_kernel_with_signs(
        byte_capacity,
        FixedSignLog::new_with_remote_storage(
            (32 * size_of::<KernelEvent>()) as u32,
            remote,
            remote_sign_storage_bytes(remote).unwrap(),
        )
        .unwrap(),
    )
}
fn profile_kernel_with_signs<
    const M: usize,
    const F: usize,
    const U: usize,
    const Z: usize,
    L: SignSink,
>(
    byte_capacity: u32,
    signs: L,
) -> ProfileKernel<M, F, U, Z, L> {
    let capacity = |slot_start| CordCapacity {
        slot_start,
        item_capacity: 1,
        byte_capacity,
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
    static TERMINALS: [[Option<AssignedTerminalTransduction>; 1]; 1] = [[None]];
    let prepared = ProfileKernel::prepare_boxed_with_linked_table_writer(
        HostedValueStore::new_with_slot_capacities(&[byte_capacity; 2], 2 * byte_capacity).unwrap(),
        signs,
        usize::MAX,
        &TERMINALS,
        &TERMINALS,
        |e| e,
        |nodes, written_cords, written_routes, calls| {
            nodes[0] = NodeSpec {
                input_cords: [Some(CordId(0))],
                maximum_step_fuel: 8,
            };
            *written_cords = cords;
            *written_routes = routes;
            calls.seal()?;
            Ok((1, 2))
        },
    )
    .unwrap();
    *prepared.bind([Echo])
}
struct Existing;
impl<const M: usize, const F: usize, const U: usize, const Z: usize, L: SignSink>
    Observer<ProfileKernel<M, F, U, Z, L>> for Existing
{
    type Error = Infallible;
    fn before_step(&mut self, _: &mut ProfileKernel<M, F, U, Z, L>) -> Result<(), Self::Error> {
        Ok(())
    }
    fn completed(
        &mut self,
        _: &mut ProfileKernel<M, F, U, Z, L>,
        _: Dispatch,
    ) -> Result<(), Self::Error> {
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
    assert!(matches!(
        PrimitiveIngressSpec::checked(&pins.lines[0], ingress_abi(c::COUNT_INFO_ID, 7)),
        Err(Refusal::UnsupportedAbi)
    ));
    let pins = Pins::new("unsupported/structured");
    assert!(matches!(
        PrimitiveIngressSpec::checked(&pins.lines[0], ingress_abi("unsupported/structured", 8)),
        Err(Refusal::UnsupportedAbi)
    ));
    let mut input = [0; 8];
    let mut output = [0; 8];
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
    assert!(matches!(
        PrimitiveIngressSpec::checked(&pins.lines[0], ingress_abi(c::COUNT_INFO_ID, 8)),
        Err(Refusal::UnsupportedAbi)
    ));
    let mut input = [0; 8];
    let mut output = [0; 8];
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
    exercise_acknowledgments(kernel(16));
}
#[test]
fn compact_profile_fore_observer_preserves_typed_delivery_pressure_and_retirement() {
    exercise_acknowledgments(profile_kernel::<0, 0, 0, 1>(16, 8));
}
fn exercise_acknowledgments<const M: usize, const F: usize, const U: usize, const Z: usize>(
    mut kernel: ProfileKernel<M, F, U, Z>,
) {
    let pins = Pins::new(c::COUNT_INFO_ID);
    let mut input = [0; 8];
    let mut output = [0; 8];
    let bound = make_bound(&pins, &mut input, &mut output);
    let (observer, port) = bound.activate(pins.activation()).unwrap();
    let mut observer = ForeObserver::new(Existing, observer);
    let (_, cost) = crate::member_return_tests::return_allocation_probe::observe(|| {
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
    });
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
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

#[path = "tests/egress_policy.rs"]
mod egress_policy;

#[path = "tests/structured_ingress.rs"]
mod structured_ingress;

#[test]
fn hosted_finite_duplex_window_repeats_ack_then_pressures_without_spending_local_closure() {
    let budget = crate::execution_sign_budget::DuplexSignBudget::select(16, 26, 24576).unwrap();
    assert_eq!(budget.ingress_items(), 2);
    exercise_finite_budget(budget);
}
#[test]
fn source55_local_reserve_and64_ceiling_fund_one_probe_and_both_normal_closures() {
    let budget = crate::execution_sign_budget::DuplexSignBudget::select(55, 64, 24576).unwrap();
    assert_eq!(
        (
            budget.ingress_items(),
            budget.remote_items(),
            budget.total_items()
        ),
        (1, 6, 61)
    );
    exercise_finite_budget(budget);
}
fn exercise_finite_budget(budget: crate::execution_sign_budget::DuplexSignBudget) {
    let count = u64::from(budget.ingress_items());
    let pins = Pins::new(c::COUNT_INFO_ID);
    let mut input = [0; 8];
    let mut output = [0; 8];
    let bound = make_bound(&pins, &mut input, &mut output);
    let (observer, port) = bound.activate(pins.activation()).unwrap();
    let mut observer = ForeObserver::with_duplex_evidence_window(Existing, observer, budget);
    let mut kernel = profile_kernel_with_signs::<0, 0, 0, 1, _>(8, budget.prepare().unwrap());
    let (_, cost) = crate::member_return_tests::return_allocation_probe::observe(|| {
        for sequence in 0..count {
            port.stage_ingress(sequence, &(sequence + 10).to_le_bytes())
                .unwrap();
            observer.before_step(&mut kernel).unwrap();
            delivered_input(&port, sequence);
            kernel.step().unwrap();
            observer.before_step(&mut kernel).unwrap();
            let event = Event::EgressOffered {
                sequence,
                length: 8,
            };
            assert_eq!(port.next_event().unwrap(), Some(event));
            port.with_egress(sequence, |bytes| {
                assert_eq!(bytes, &(sequence + 10).to_le_bytes())
            })
            .unwrap();
            port.sent(event).unwrap();
            port.peer_accepted(sequence).unwrap();
            observer.before_step(&mut kernel).unwrap();
            port.peer_delivered(sequence).unwrap();
            observer.before_step(&mut kernel).unwrap();
            assert_eq!(kernel.values().used_items(), 0);
        }
        let before = kernel.signs().events().count();
        let remote_before = kernel
            .signs()
            .events()
            .filter(|e| kernel.signs().remote_identity(e.sequence).is_some())
            .count();
        assert_eq!(remote_before, count as usize * 4);
        port.stage_ingress(count, &(count + 10).to_le_bytes())
            .unwrap();
        observer.before_step(&mut kernel).unwrap();
        assert_eq!(
            port.next_event().unwrap(),
            Some(Event::IngressPressure(count))
        );
        assert_eq!(kernel.signs().events().count(), before);
        assert_eq!(kernel.cord_usage(CordId(0)).unwrap(), (0, 0));
        assert_eq!(kernel.values().used_items(), 0);
        kernel
            .signs()
            .ensure_capacity(budget.local_items())
            .unwrap();
        kernel.signs().ensure_remote_capacity(2).unwrap();
        // Explicit trusted terminal input, distinct from accepting the pressured
        // request. Both closure sidecars remain available; no cancellation/rearm.
        kernel
            .close_remote_input(RemoteEndpointId(0), CordId(0))
            .unwrap();
        kernel.step().unwrap();
        assert!(kernel
            .signs()
            .events()
            .any(|e| e.kind == KernelEventKind::BackCompleted));
        assert_eq!(
            kernel
                .signs()
                .events()
                .filter(|e| kernel.signs().remote_identity(e.sequence).is_some())
                .count(),
            count as usize * 4 + 2
        );
        for sequence in 0..count {
            for kind in [
                KernelEventKind::RemoteInputAdmitted,
                KernelEventKind::RemoteValueOffered,
                KernelEventKind::RemoteValueAccepted,
                KernelEventKind::RemoteValueDelivered,
            ] {
                assert!(kernel.signs().events().any(|e| e.kind == kind
                    && kernel
                        .signs()
                        .remote_identity(e.sequence)
                        .is_some_and(|id| id.sequence == sequence)));
            }
        }
        assert_eq!(
            port.next_event().unwrap(),
            Some(Event::IngressPressure(count)),
            "never ACK an unaccepted request"
        );
    });
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    assert!(
        kernel.try_retire().is_ok(),
        "normal terminal/drained retirement needs no evidence reset"
    );
}
