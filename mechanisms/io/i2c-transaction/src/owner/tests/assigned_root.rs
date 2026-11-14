//! Actual Root kernel construction with inert local possession, not MCU proof.
use super::*;
use conduit_kernel::{CordId, KernelEventKind, RemoteEndpointId, ValueStorage};
use root_native_preparation::{NativePossession, PreparedNativeKernel};
pub(super) fn run(
    slot: &mut Option<ReadyI2cBase<Provider>>,
    contract: &I2cContract,
    selected: SelectedAssignedForeOperationPlan<'_>,
    issue: impl Fn() -> (
        BaseCapabilityTable,
        BaseCapabilityHandle,
        BaseOperationClaim,
    ),
) {
    let selection = || SelectedAssignedForeOperationPlan {
        operation: SelectedAssignedOperationPlan {
            input: selected.operation.input,
            maxima: selected.operation.maxima,
            requirements: selected.operation.requirements,
            computations: selected.operation.computations,
            fragment_id: selected.operation.fragment_id,
            scope: selected.operation.scope,
            node: selected.operation.node,
        },
        fore: selected.fore,
    };
    let zero = possession(slot, &issue);
    let (refused, cost) = crate::allocation_probe::observe(|| {
        PreparedNativeKernel::prepare(zero, contract, selection(), 0)
    });
    assert!(refused.is_err());
    drop(refused);
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    let mut hidden = possession(slot, &issue);
    hidden.claim.host_id.0.reserve(16384);
    assert!(PreparedNativeKernel::prepare(hidden, contract, selection(), 16384).is_err());
    assert!(slot.is_some());
    let prepared =
        PreparedNativeKernel::prepare(possession(slot, &issue), contract, selection(), 16384)
            .unwrap();
    let charge = prepared.storage_bytes();
    assert!(charge <= 16384);
    drop(prepared);
    assert!(slot.is_some());
    assert!(PreparedNativeKernel::prepare(
        possession(slot, &issue),
        contract,
        selection(),
        charge - 1
    )
    .is_err());
    assert!(slot.is_some());
    let prepared =
        PreparedNativeKernel::prepare(possession(slot, &issue), contract, selection(), charge)
            .unwrap();
    let (mut bound, binding) = crate::allocation_probe::observe(|| prepared.bind());
    assert_eq!((binding.allocations, binding.reallocations), (0, 0));
    assert!(slot.is_none());
    assert_eq!(bound.call.provider.effects, 0);
    let input = request(contract, 0x76, &[0xd0], 1);
    let mut encoder = crate::result::PreparedI2cResultEncoder::new(contract).unwrap();
    let expected = encoder.completed(1, &[1]).unwrap().to_vec();
    let expected_second = encoder.completed(1, &[2]).unwrap().to_vec();
    let incoming = selected
        .fore
        .iter()
        .find(|f| f.direction() == AssignedForeDirection::Input)
        .unwrap();
    let outgoing = selected
        .fore
        .iter()
        .find(|f| f.direction() == AssignedForeDirection::Output)
        .unwrap();
    let (_, execution) = crate::allocation_probe::observe(|| {
        bound
            .kernel
            .admit_remote_input(
                RemoteEndpointId(incoming.endpoint()),
                CordId(incoming.cord()),
                0,
                &input,
            )
            .unwrap();
        bound.kernel.step().unwrap();
        assert_eq!(
            bound.dispatch_one().unwrap(),
            root_native_preparation::NativeDispatch::Completed
        );
        assert_eq!(
            bound.dispatch_one().unwrap(),
            root_native_preparation::NativeDispatch::Idle
        );
        bound.kernel.step().unwrap();
        let offer = bound
            .kernel
            .remote_egress_offer(
                RemoteEndpointId(outgoing.endpoint()),
                CordId(outgoing.cord()),
            )
            .unwrap()
            .unwrap();
        assert_eq!(bound.kernel.host_value(offer.value).unwrap(), expected);
        // Keep the first result retained while a second request arrives.
        bound
            .kernel
            .admit_remote_input(
                RemoteEndpointId(incoming.endpoint()),
                CordId(incoming.cord()),
                1,
                &input,
            )
            .unwrap();
        bound.kernel.step().unwrap();
        for _ in 0..3 {
            assert_eq!(
                bound.dispatch_one().unwrap(),
                root_native_preparation::NativeDispatch::Pressure
            );
            assert_eq!(bound.call.provider.effects, 1);
        }
        bound
            .kernel
            .remote_egress_accept(
                RemoteEndpointId(outgoing.endpoint()),
                CordId(outgoing.cord()),
                offer.sequence,
            )
            .unwrap();
        bound
            .kernel
            .remote_egress_delivered(
                RemoteEndpointId(outgoing.endpoint()),
                CordId(outgoing.cord()),
                offer.sequence,
            )
            .unwrap();
        assert_eq!(
            bound.dispatch_one().unwrap(),
            root_native_preparation::NativeDispatch::Completed
        );
        assert_eq!(bound.call.provider.effects, 2);
        bound.kernel.step().unwrap();
        let second = bound
            .kernel
            .remote_egress_offer(
                RemoteEndpointId(outgoing.endpoint()),
                CordId(outgoing.cord()),
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            bound.kernel.host_value(second.value).unwrap(),
            expected_second
        );
        bound
            .kernel
            .remote_egress_accept(
                RemoteEndpointId(outgoing.endpoint()),
                CordId(outgoing.cord()),
                second.sequence,
            )
            .unwrap();
        bound
            .kernel
            .remote_egress_delivered(
                RemoteEndpointId(outgoing.endpoint()),
                CordId(outgoing.cord()),
                second.sequence,
            )
            .unwrap();
        // A malformed third input is a typed failed completion, not a stuck
        // dispatched request or an opportunity to repeat the physical effect.
        bound
            .kernel
            .admit_remote_input(
                RemoteEndpointId(incoming.endpoint()),
                CordId(incoming.cord()),
                2,
                b"invalid",
            )
            .unwrap();
        bound.kernel.step().unwrap();
        assert!(matches!(
            bound.dispatch_one().unwrap(),
            root_native_preparation::NativeDispatch::Refused(
                root_native_preparation::NativeDispatchRefusal::Call(_)
            )
        ));
        assert_eq!(
            bound.dispatch_one().unwrap(),
            root_native_preparation::NativeDispatch::Idle
        );
        assert!(matches!(
            bound.kernel.step(),
            Err(conduit_kernel::scheduler::SchedulerError::BackFailed(_))
        ));
        bound.kernel.cancel().unwrap();
        let retired = (*bound.kernel)
            .try_retire()
            .unwrap_or_else(|_| panic!("native Root kernel not drained"));
        assert!(retired
            .signs
            .events()
            .any(|e| e.kind == KernelEventKind::RunCancelled));
        assert_eq!(retired.values.used_items(), 0);
        assert_eq!(bound.call.provider.effects, 2);
    });
    assert_eq!((execution.allocations, execution.reallocations), (0, 0));
    std::println!("actual Root native kernel: {charge} retained backing/kernel bytes; budget/cancellation preserve ready owner, typed Fore/Host Call/egress and terminal retirement, zero binding/execution allocation; CPU-only, no whole-workset admission or device Start claim");
}

fn possession<'a>(
    slot: &'a mut Option<ReadyI2cBase<Provider>>,
    issue: &impl Fn() -> (
        BaseCapabilityTable,
        BaseCapabilityHandle,
        BaseOperationClaim,
    ),
) -> NativePossession<'a, Provider> {
    let (table, handle, claim) = issue();
    NativePossession {
        slot,
        table,
        handle,
        claim,
    }
}
