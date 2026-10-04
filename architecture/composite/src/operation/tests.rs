//! Prepared output ownership and exact terminal behavior through erased Backs.
use super::*;
use crate::test_support::{allocation, common, single_child};
use crate::{KernelCompositeError, KernelCompositeHost};
use conduit_core::*;
use conduit_kernel::scheduler::{
    AssignedAbnormalTransduction, AssignedCancellationTransduction,
    AssignedNormalCloseTransduction, AssignedTerminalTransduction,
};
use conduit_kernel::PortId;

const BYTES: usize = 256;
const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;

struct PreparedEcho {
    bytes: [u8; BYTES],
    staged: bool,
    terminal: Option<AssignedTerminalTransduction>,
}
impl StepBack<PORTS> for PreparedEcho {
    fn terminal_transduction(&self) -> Option<AssignedTerminalTransduction> {
        self.terminal
    }
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if io.input(PortId(0)).is_none() || !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let bytes = inputs.input(PortId(0)).unwrap();
        assert_eq!(bytes.len(), BYTES);
        self.bytes.copy_from_slice(bytes);
        self.staged = true;
        io.consume(PortId(0)).unwrap();
        io.send_prepared(PortId(0), BYTES as u32).unwrap();
        StepOutcome::Progress
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0) && self.staged).then_some(&self.bytes)
    }
    fn step_committed(&mut self) {
        self.staged = false;
    }
}

struct Factory {
    implementation: ImplementationId,
    terminal: Option<AssignedTerminalTransduction>,
}
impl KernelOperationFactory for Factory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation
    }
    fn budget(&self, _: &PlannedGear) -> Result<KernelOperationBudget, String> {
        Ok(KernelOperationBudget {
            value_items: 0,
            value_bytes: 0,
            maximum_value_bytes: BYTES as u32,
            host_requests: 0,
            sign_items: 32,
        })
    }
    fn prepare(
        &self,
        _: &PlannedGear,
        _: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<PORTS> + Send>, String> {
        Ok(Box::new(PreparedEcho {
            bytes: [0; BYTES],
            staged: false,
            terminal: self.terminal,
        }))
    }
}

fn definition(terminal: bool) -> crate::KernelCompositeDefinition {
    let mut fragment = common::fragment();
    let placement = &mut fragment.placements[0];
    for port in placement.inputs.iter_mut().chain(&mut placement.outputs) {
        port.value_kind = kind_id("fixture/octet-frame/256");
        if terminal {
            port.temporal = PortTemporal::Flow { closes: true };
        }
    }
    if terminal {
        placement.terminal_transductions = vec![TerminalTransductionProfile {
            input_port_id: placement.inputs[0].port_id.clone(),
            output_port_id: placement.outputs[0].port_id.clone(),
            normal_close: NormalCloseTransduction::PropagateAfterDrain,
            abnormal: AbnormalTerminalTransduction::NotAccepted,
            cancellation: CancellationTransduction::NotCancellable,
        }];
    }
    single_child(fragment, BYTES as u32)
}

fn assigned() -> AssignedTerminalTransduction {
    AssignedTerminalTransduction {
        input: PortId(0),
        output: PortId(0),
        normal_close: AssignedNormalCloseTransduction::PropagateAfterDrain,
        abnormal: AssignedAbnormalTransduction::NotAccepted,
        cancellation: AssignedCancellationTransduction::NotCancellable,
    }
}

fn registry(
    definition: &crate::KernelCompositeDefinition,
    terminal: Option<AssignedTerminalTransduction>,
) -> KernelOperationRegistry {
    let mut registry = KernelOperationRegistry::new();
    registry
        .install(Factory {
            implementation: definition.internal_plan.fragments[0].placements[0]
                .implementation_id
                .clone(),
            terminal,
        })
        .unwrap();
    registry
}

#[test]
fn prepared_frames_survive_pressure_and_private_buffer_reuse_without_allocation() {
    let definition = definition(false);
    let input = definition.boundary.input_fronts[0].external_port.clone();
    let output = definition.boundary.output_fronts[0].external_port.clone();
    let registry = registry(&definition, None);
    let mut host = KernelCompositeHost::prepare(definition, &registry).unwrap();
    let first = ValuePayload {
        value_kind: input.value_kind.clone(),
        encoded: vec![1; BYTES],
    };
    let second = ValuePayload {
        value_kind: input.value_kind.clone(),
        encoded: vec![2; BYTES],
    };
    let mut observed = ValuePayload {
        value_kind: output.value_kind.clone(),
        encoded: Vec::with_capacity(BYTES),
    };
    let allocations = allocation::allocations_during(|| {
        host.start().unwrap();
        host.admit_input(&input.port_id, 0, &first).unwrap();
        host.step().unwrap();
        assert_eq!(
            host.output_into(&output.port_id, &mut observed).unwrap(),
            Some(0)
        );
        assert_eq!(observed, first);
        host.admit_input(&input.port_id, 1, &second).unwrap();
        for _ in 0..1000 {
            host.step().unwrap();
        }
        assert_eq!(
            host.output_into(&output.port_id, &mut observed).unwrap(),
            Some(0)
        );
        assert_eq!(observed, first);
        host.complete_output(&output.port_id, 0).unwrap();
        host.step().unwrap();
        assert_eq!(
            host.output_into(&output.port_id, &mut observed).unwrap(),
            Some(1)
        );
        assert_eq!(observed, second);
        host.complete_output(&output.port_id, 1).unwrap();
    });
    assert_eq!(allocations, 0);
}

#[test]
fn terminal_behavior_must_match_the_exact_plan_in_both_directions() {
    for planned in [false, true] {
        for implemented in [false, true] {
            let definition = definition(planned);
            let registry = registry(&definition, implemented.then_some(assigned()));
            let result = KernelCompositeHost::prepare(definition, &registry);
            if planned == implemented {
                assert!(result.is_ok());
            } else {
                assert!(matches!(
                    result,
                    Err(KernelCompositeError::ChildRefused { .. })
                ));
            }
        }
    }
    let back = BoxedKernelBack::new(Box::new(PreparedEcho {
        bytes: [0; BYTES],
        staged: false,
        terminal: Some(assigned()),
    }));
    assert_eq!(back.terminal_transduction(), Some(assigned()));
    assert_eq!(back.terminal_transductions()[0], Some(assigned()));
}

#[test]
fn plural_terminal_mappings_are_retained_without_a_single_input_fallback() {
    struct Plural;
    impl StepBack<PORTS> for Plural {
        fn terminal_transductions(&self) -> [Option<AssignedTerminalTransduction>; PORTS] {
            let mut contracts = [None; PORTS];
            contracts[0] = Some(assigned());
            contracts[1] = Some(AssignedTerminalTransduction {
                input: PortId(1),
                ..assigned()
            });
            contracts
        }
        fn step(&mut self, _: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
            StepOutcome::Await
        }
    }
    let back = BoxedKernelBack::new(Box::new(Plural));
    assert_eq!(back.terminal_transduction(), None);
    let contracts = back.terminal_transductions();
    assert_eq!(contracts[0], Some(assigned()));
    assert_eq!(contracts[1].unwrap().input, PortId(1));
    assert!(contracts[2..].iter().all(Option::is_none));
}
