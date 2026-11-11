//! Actual wire/selected owner/kernel dispatch proof with inert GPIO only.
use conduit_core::*;
use conduit_kernel::{scheduler::*, *};
use conduit_kernel::{CordId, HostCallId, NodeId, PortId, RequestId};
use conduit_mcu_host_runtime::{execution::StoredExecutionKernel, native_dispatch};
use conduit_plan_lowering::selected_operation::SelectedOperationPlan;
use conduit_tm1637_wire::{owner::*, *};
use std::{cell::Cell, rc::Rc};
#[path = "../../../../../mechanisms/io/tm1637/tests/support/fixture.rs"]
mod fixture;
use super::fixture::selection_fixture;

struct State {
    reads: Cell<u8>,
    released: Cell<[bool; 2]>,
    mode: u8,
}
struct Provider(Rc<State>);
impl OpenDrain for Provider {
    type Error = &'static str;
    fn clock(&mut self, released: bool) -> Result<(), Self::Error> {
        let mut levels = self.0.released.get();
        levels[0] = released;
        self.0.released.set(levels);
        Ok(())
    }
    fn data(&mut self, released: bool) -> Result<(), Self::Error> {
        let mut levels = self.0.released.get();
        levels[1] = released;
        self.0.released.set(levels);
        Ok(())
    }
    fn read_data(&mut self) -> Result<bool, Self::Error> {
        let index = self.0.reads.get();
        self.0.reads.set(index + 1);
        // The first packet is one byte; third ACK is packet 1, byte 1.
        Ok(index == 2)
    }
    fn half_period(&mut self) -> Result<(), Self::Error> {
        if self.0.mode == 2 {
            Err("clock deadline fixture")
        } else {
            Ok(())
        }
    }
}
fn encoded() -> Vec<u8> {
    fn value(
        ty: &StructuredInfoType,
        name: &str,
        lengths: &mut impl Iterator<Item = u8>,
    ) -> StructuredInfoValue {
        match ty.shape() {
            StructuredInfoTypeShape::Leaf(_) => StructuredInfoValue::leaf(
                ty.clone(),
                vec![if name == "length" {
                    lengths.next().unwrap()
                } else {
                    0
                }],
            )
            .unwrap(),
            StructuredInfoTypeShape::Collection { element, length } => {
                StructuredInfoValue::collection(
                    ty.clone(),
                    (0..length).map(|_| value(element, name, lengths)).collect(),
                )
                .unwrap()
            }
            StructuredInfoTypeShape::Record { fields, .. } => StructuredInfoValue::record(
                ty.clone(),
                fields
                    .iter()
                    .map(|field| {
                        StructuredFieldValue::new(
                            field.name(),
                            value(field.value_type(), field.name(), lengths),
                        )
                        .unwrap()
                    })
                    .collect(),
            )
            .unwrap(),
            _ => panic!("exact packet schema"),
        }
    }
    value(
        WriteContract::prepare().unwrap().packets_type(),
        "",
        &mut [1, 7, 1].into_iter(),
    )
    .canonical_bytes()
    .unwrap()
}
struct Request {
    input: ValueRef,
    requested: bool,
}
impl StepBack<1> for Request {
    fn step(&mut self, io: &mut StepIo<1>, _: &StepInputBytes<'_, 1>) -> StepOutcome {
        if !self.requested {
            io.request_host_call(
                RequestId(0),
                HostCallId(0),
                BoundedValueRef::new(self.input, MAXIMUM_BYTES).unwrap(),
            )
            .unwrap();
            self.requested = true;
            StepOutcome::Progress
        } else if let Some((_, outcome)) = io.host_completion() {
            StepOutcome::Fail(outcome.failure.expect("actual failed completion"))
        } else {
            StepOutcome::Await
        }
    }
}
fn kernel(input: &[u8]) -> StoredExecutionKernel<Request, FixedSignLog<64>, 1, 1, 1, 0, 0, 1> {
    let mut values = HostedValueStore::new(4, MAXIMUM_BYTES, MAXIMUM_BYTES * 4).unwrap();
    let input = values.store(input).unwrap();
    let mut routes = FixedRoutes::<0, 0>::new(1);
    routes.seal().unwrap();
    let mut bindings = FixedHostCallBindings::<1>::new(1);
    bindings
        .install(
            NodeId(0),
            HostCallBinding {
                call: HostCallId(0),
                maximum_input_bytes: MAXIMUM_BYTES,
                maximum_output_bytes: 0,
            },
        )
        .unwrap();
    bindings.seal().unwrap();
    StoredExecutionKernel::new_with_active_counts_and_host_calls(
        1,
        0,
        [NodeSpec {
            input_cords: [None],
            maximum_step_fuel: 8,
        }],
        [CordSpec::local(
            CordId(0),
            (NodeId(0), PortId(0)),
            (NodeId(0), PortId(0)),
            CordCapacity {
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: MAXIMUM_BYTES,
                pressure_policy: Default::default(),
            },
        )],
        routes,
        bindings,
        [Request {
            input,
            requested: false,
        }],
        values,
        FixedSignLog::new(64 * core::mem::size_of::<KernelEvent>() as u32).unwrap(),
    )
    .unwrap()
}
#[test]
fn wire_nack_location_and_other_electrical_failures_survive_kernel_dispatch() {
    use native_dispatch::{write_failure as detail, Dispatch};
    for mode in 0..3 {
        let state = Rc::new(State {
            reads: Cell::new(0),
            released: Cell::new([true; 2]),
            mode,
        });
        let (mut owner, node) = fixture::owner(Provider(state.clone()));
        assert_eq!(node, NodeId(0));
        let mut kernel = kernel(&encoded());
        kernel.step().unwrap();
        let Dispatch::Completed { outcome, .. } =
            native_dispatch::write(&mut kernel, &mut owner, || mode == 1).unwrap()
        else {
            panic!("actual bounded write completion");
        };
        assert_eq!(outcome.disposition, HostCallDisposition::Failed);
        let failure = outcome.failure.unwrap();
        assert_eq!(failure.code, FailureCode::HostCallFailed);
        assert_eq!(
            failure.detail,
            match mode {
                0 => detail::NACK | 0x11,
                1 => detail::CANCELLED,
                _ => detail::PROVIDER,
            }
        );
        assert_eq!(state.reads.get(), if mode == 0 { 3 } else { 0 });
        assert_eq!(state.released.get(), [true; 2]);
        assert_eq!(owner.inspection().lifecycle, CapabilityLifecycle::Revoked);
        // Kernel consumes the exact failed Host Call outcome as its node fault.
        assert_eq!(kernel.step(), Err(SchedulerError::BackFailed(failure)));
        assert!(kernel
            .signs()
            .events()
            .any(|event| event.kind == KernelEventKind::BackFailed));
    }
}
