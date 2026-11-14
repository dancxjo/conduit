//! Exact terminal-law parity for the linked immutable storage profile.
use super::*;
use crate::FixedValueStore;
use core::{cell::Cell, mem::size_of};
use std::{boxed::Box, rc::Rc};
const fn law(input: u16) -> Option<AssignedTerminalTransduction> {
    Some(AssignedTerminalTransduction {
        input: PortId(input),
        output: PortId(0),
        normal_close: AssignedNormalCloseTransduction::PropagateWhenAllClose,
        abnormal: AssignedAbnormalTransduction::PropagateAfterDrain,
        cancellation: AssignedCancellationTransduction::NotCancellable,
    })
}
static CONTRACTS: [[Option<AssignedTerminalTransduction>; 2]; 1] = [[law(0), law(1)]];
struct Back {
    first_closed: bool,
    changed_law: bool,
    finalizes: bool,
}
impl StepBack<2> for Back {
    fn terminal_transductions(&self) -> [Option<AssignedTerminalTransduction>; 2] {
        let mut contracts = CONTRACTS[0];
        if self.changed_law {
            contracts[0].as_mut().unwrap().normal_close = AssignedNormalCloseTransduction::Consume;
        }
        if self.finalizes {
            contracts[0].as_mut().unwrap().abnormal =
                AssignedAbnormalTransduction::FinalizeThenPropagate(
                    AssignedFiniteTerminalEmission {
                        maximum_items: 1,
                        maximum_bytes: 1,
                    },
                );
        }
        contracts
    }
    fn step(&mut self, io: &mut StepIo<2>, _: &StepInputBytes<'_, 2>) -> StepOutcome {
        for p in [PortId(0), PortId(1)] {
            if let Some(terminal) = io.input_abnormal(p) {
                io.consume_abnormal(p).unwrap();
                return StepOutcome::Abnormal {
                    port: PortId(0),
                    terminal,
                };
            }
        }
        if io.input_closed(PortId(0)) {
            io.consume_closed(PortId(0)).unwrap();
            self.first_closed = true;
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(1)) {
            io.consume_closed(PortId(1)).unwrap();
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }
}
type Kernel<const M: usize, const B: usize = 100> =
    FixedScheduler<Back, FixedValueStore<2, 8>, FixedSignLog<64>, 1, 2, 2, 2, 1, 1, 0, 1, M, B>;
type Prepared<const M: usize, const B: usize = 100> = PreparedKernelStorage<Back, Kernel<M, B>, 1>;
fn write(
    nodes: &mut [NodeSpec<2>; 1],
    cords: &mut [CordSpec; 2],
    routes: &mut FixedRoutes<1, 1>,
    calls: &mut FixedHostCallBindings<0>,
) -> Result<(usize, usize), KernelPreparationError> {
    nodes[0] = NodeSpec {
        input_cords: [Some(CordId(0)), Some(CordId(1))],
        maximum_step_fuel: 8,
    };
    for (i, cord) in cords.iter_mut().enumerate() {
        *cord = CordSpec::remote_ingress(
            CordId(i as u16),
            RemoteEndpointId(i as u16),
            (NodeId(0), PortId(i as u16)),
            CordCapacity {
                slot_start: i as u16,
                item_capacity: 1,
                byte_capacity: 4,
                pressure_policy: Default::default(),
            },
        );
    }
    routes.seal()?;
    calls.seal()?;
    Ok((1, 2))
}
fn signs() -> FixedSignLog<64> {
    FixedSignLog::new_with_remote_storage(
        (64 * size_of::<crate::KernelEvent>()) as u32,
        16,
        crate::remote_sign_storage_bytes(16).unwrap(),
    )
    .unwrap()
}
fn prepare<const M: usize>(
    linked: bool,
    maximum: usize,
    actual: &[[Option<AssignedTerminalTransduction>; 2]; 1],
) -> Result<Prepared<M>, KernelPreparationError> {
    prepare_profile::<M, 100>(linked, maximum, actual)
}
fn prepare_profile<const M: usize, const B: usize>(
    linked: bool,
    maximum: usize,
    actual: &[[Option<AssignedTerminalTransduction>; 2]; 1],
) -> Result<Prepared<M, B>, KernelPreparationError> {
    let values = FixedValueStore::new(16).unwrap();
    if linked {
        Kernel::<M, B>::prepare_boxed_with_linked_table_writer(
            values,
            signs(),
            maximum,
            &CONTRACTS,
            actual,
            |e| e,
            write,
        )
    } else {
        Kernel::<M, B>::prepare_boxed_with_table_writer(
            values,
            signs(),
            maximum,
            &CONTRACTS,
            actual,
            |e| e,
            write,
        )
    }
}
fn bound<const M: usize>(linked: bool) -> Box<Kernel<M>> {
    prepare::<M>(linked, usize::MAX, &CONTRACTS)
        .unwrap()
        .bind([Back {
            first_closed: false,
            changed_law: false,
            finalizes: false,
        }])
}

#[path = "phase_terminal_tests.rs"]
mod phase_terminal_tests;
#[test]
fn linked_terminal_normal_and_abnormal_execution_matches_inline_without_allocating() {
    let mut inline = bound::<1>(false);
    let mut linked = bound::<0>(true);
    assert_eq!(inline.terminal_transductions, CONTRACTS);
    assert!(core::ptr::eq(&*linked.terminal_transductions, &CONTRACTS));
    let allocation = AllocationSample::begin();
    for k in [&mut *inline as &mut dyn NormalTerminal, &mut *linked] {
        k.close_and_drain();
    }
    let cost = allocation.finish();
    assert_eq!(cost.allocations + cost.reallocations, 0);
    assert_eq!(
        inline.signs.events().collect::<std::vec::Vec<_>>(),
        linked.signs.events().collect::<std::vec::Vec<_>>()
    );
    let mut inline = bound::<1>(false);
    let mut linked = bound::<0>(true);
    let terminal = CanonicalValue::new(b"bad").unwrap();
    inline
        .close_remote_input_abnormal(RemoteEndpointId(1), CordId(1), terminal)
        .unwrap();
    linked
        .close_remote_input_abnormal(RemoteEndpointId(1), CordId(1), terminal)
        .unwrap();
    assert_eq!(inline.step(), linked.step());
    assert_eq!(
        inline.signs.events().collect::<std::vec::Vec<_>>(),
        linked.signs.events().collect::<std::vec::Vec<_>>()
    );
}
trait NormalTerminal {
    fn close_and_drain(&mut self);
}
impl<const M: usize, const B: usize> NormalTerminal for Kernel<M, B> {
    fn close_and_drain(&mut self) {
        self.close_remote_input(RemoteEndpointId(0), CordId(0))
            .unwrap();
        self.close_remote_input(RemoteEndpointId(1), CordId(1))
            .unwrap();
        assert_eq!(
            self.step(),
            Ok(SchedulerStatus::Progress { node: NodeId(0) })
        );
        assert_eq!(self.step(), Ok(SchedulerStatus::Drained));
    }
}
#[test]
fn linked_terminal_cancellation_and_same_law_rebind_preserve_checked_custody() {
    let mut inline = bound::<1>(false);
    let mut linked = bound::<0>(true);
    inline.bind_terminal_transductions(CONTRACTS).unwrap();
    linked.bind_terminal_transductions(CONTRACTS).unwrap();
    inline.cancel().unwrap();
    linked.cancel().unwrap();
    assert_eq!(inline.step(), Ok(SchedulerStatus::Cancelled));
    assert_eq!(inline.step(), linked.step());
    assert_eq!(
        inline.signs.events().collect::<std::vec::Vec<_>>(),
        linked.signs.events().collect::<std::vec::Vec<_>>()
    );
    // A by-value changed table cannot replace linked law custody.
    let mut different = CONTRACTS;
    different[0][0].as_mut().unwrap().normal_close = AssignedNormalCloseTransduction::Consume;
    // Even an actual Back that now reports matching new laws cannot replace
    // the immutable linked custody. The ordinary inline profile still can.
    let mut linked = bound::<0>(true);
    let mut inline = bound::<1>(false);
    linked.drivers[0].changed_law = true;
    inline.drivers[0].changed_law = true;
    inline.bind_terminal_transductions(different).unwrap();
    assert_eq!(
        linked.bind_terminal_transductions(different),
        Err(SchedulerError::InvalidPlan)
    );
    assert!(core::ptr::eq(&*linked.terminal_transductions, &CONTRACTS));
}
#[test]
fn linked_terminal_size_charge_is_exact_and_owned_entrance_refuses_zero_inline_rows() {
    let saving = size_of::<Kernel<1>>() - size_of::<Kernel<0>>();
    assert_eq!(
        saving,
        size_of::<[[Option<AssignedTerminalTransduction>; 2]; 1]>()
    );
    let exact = size_of::<Kernel<0>>();
    let prepared = prepare::<0>(true, exact, &CONTRACTS).unwrap();
    assert_eq!(prepared.storage_bytes(), exact);
    assert_eq!(
        prepare::<0>(true, exact - 1, &CONTRACTS).err(),
        Some(KernelPreparationError::Capacity {
            required: exact,
            available: exact - 1
        })
    );
    assert_eq!(
        prepare::<0>(false, usize::MAX, &CONTRACTS).err(),
        Some(KernelPreparationError::Topology(
            SchedulerError::InvalidPlan
        ))
    );
    std::eprintln!(
        "linked terminal CPU size inline={} linked={} saved={} table={}",
        size_of::<Kernel<1>>(),
        exact,
        saving,
        size_of::<[[Option<AssignedTerminalTransduction>; 2]; 1]>()
    );
}
#[test]
fn linked_terminal_mismatch_and_inactive_contracts_refuse_before_bind() {
    let mut wrong = CONTRACTS;
    wrong[0][0] = None;
    assert_eq!(
        prepare::<0>(true, usize::MAX, &wrong).err(),
        Some(KernelPreparationError::Topology(
            SchedulerError::InvalidPlan
        ))
    );
    type Capacity =
        FixedScheduler<Back, FixedValueStore<2, 8>, FixedSignLog<64>, 2, 2, 2, 2, 1, 1, 0, 1, 0>;
    static INACTIVE_CONTRACTS: [[Option<AssignedTerminalTransduction>; 2]; 2] =
        [CONTRACTS[0], CONTRACTS[0]];
    let result = Capacity::prepare_boxed_with_linked_table_writer(
        FixedValueStore::new(16).unwrap(),
        signs(),
        usize::MAX,
        &INACTIVE_CONTRACTS,
        &INACTIVE_CONTRACTS,
        |e| e,
        |nodes, cords, routes, calls| {
            let mut first = [nodes[0]];
            write(&mut first, cords, routes, calls)?;
            nodes[0] = first[0];
            Ok((1, 2))
        },
    );
    assert_eq!(
        result.err(),
        Some(KernelPreparationError::Topology(
            SchedulerError::InvalidPlan
        ))
    );
}

struct DroppedValues {
    inner: FixedValueStore<2, 8>,
    drops: Rc<Cell<usize>>,
}
impl Drop for DroppedValues {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
impl ValueStorage for DroppedValues {
    fn item_capacity(&self) -> u16 {
        self.inner.item_capacity()
    }
    fn byte_capacity(&self) -> u32 {
        self.inner.byte_capacity()
    }
    fn used_items(&self) -> u16 {
        self.inner.used_items()
    }
    fn used_bytes(&self) -> u32 {
        self.inner.used_bytes()
    }
    fn store(&mut self, bytes: &[u8]) -> Result<ValueRef, StorageError> {
        self.inner.store(bytes)
    }
    fn get(&self, value: ValueRef) -> Result<&[u8], StorageError> {
        self.inner.get(value)
    }
    fn reference_count(&self, value: ValueRef) -> Result<u16, StorageError> {
        self.inner.reference_count(value)
    }
    fn retain(&mut self, value: ValueRef) -> Result<(), StorageError> {
        self.inner.retain(value)
    }
    fn release(&mut self, value: ValueRef) -> Result<(), StorageError> {
        self.inner.release(value)
    }
    fn clear(&mut self) {
        self.inner.clear();
    }
}
#[test]
fn linked_terminal_unbound_drop_and_writer_refusal_release_owned_values_once() {
    type DropKernel =
        FixedScheduler<Back, DroppedValues, FixedSignLog<64>, 1, 2, 2, 2, 1, 1, 0, 1, 0>;
    for refused in [false, true] {
        let drops = Rc::new(Cell::new(0));
        let result = DropKernel::prepare_boxed_with_linked_table_writer(
            DroppedValues {
                inner: FixedValueStore::new(16).unwrap(),
                drops: drops.clone(),
            },
            signs(),
            usize::MAX,
            &CONTRACTS,
            &CONTRACTS,
            |e| e,
            |nodes, cords, routes, calls| {
                write(nodes, cords, routes, calls)?;
                if refused {
                    Err(SchedulerError::InvalidPlan.into())
                } else {
                    Ok((1, 2))
                }
            },
        );
        if refused {
            assert!(result.is_err());
            assert_eq!(drops.get(), 1);
        } else {
            let prepared = result.unwrap();
            assert_eq!(drops.get(), 0);
            drop(prepared);
            assert_eq!(drops.get(), 1);
        }
    }
}
