//! Actual Root kernel construction with inert local possession, not MCU proof.
use super::*;
use conduit_kernel::{
    BoundedValueRef, CordId, HostCallDisposition, HostCallOutcome, KernelEventKind,
    RemoteEndpointId, ValueStorage,
};
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
    let (bound, binding) = crate::allocation_probe::observe(|| prepared.bind());
    assert_eq!((binding.allocations, binding.reallocations), (0, 0));
    assert!(slot.is_none());
    let root_native_preparation::BoundNativeKernel {
        mut kernel,
        mut call,
    } = bound;
    assert_eq!(call.provider.effects, 0);
    let input = request(contract, 0x76, &[0xd0], 1);
    let mut encoder = crate::result::PreparedI2cResultEncoder::new(contract).unwrap();
    let expected = encoder.completed(1, &[1]).unwrap().to_vec();
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
        kernel
            .admit_remote_input(
                RemoteEndpointId(incoming.endpoint()),
                CordId(incoming.cord()),
                0,
                &input,
            )
            .unwrap();
        kernel.step().unwrap();
        let request = kernel.next_host_request().unwrap();
        let output = call
            .invoke(
                request.node,
                request.call,
                request.request,
                kernel.host_value(request.input.value).unwrap(),
            )
            .unwrap();
        let value = kernel.store_host_value(output).unwrap();
        kernel
            .complete_host_call(
                request.node,
                request.request,
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: Some(BoundedValueRef::new(value, I2C_MAXIMUM_BYTES).unwrap()),
                    failure: None,
                },
            )
            .unwrap();
        kernel.step().unwrap();
        let offer = kernel
            .remote_egress_offer(
                RemoteEndpointId(outgoing.endpoint()),
                CordId(outgoing.cord()),
            )
            .unwrap()
            .unwrap();
        assert_eq!(kernel.host_value(offer.value).unwrap(), expected);
        kernel
            .remote_egress_accept(
                RemoteEndpointId(outgoing.endpoint()),
                CordId(outgoing.cord()),
                offer.sequence,
            )
            .unwrap();
        kernel
            .remote_egress_delivered(
                RemoteEndpointId(outgoing.endpoint()),
                CordId(outgoing.cord()),
                offer.sequence,
            )
            .unwrap();
        kernel.cancel().unwrap();
        let retired = (*kernel)
            .try_retire()
            .unwrap_or_else(|_| panic!("native Root kernel not drained"));
        assert!(retired
            .signs
            .events()
            .any(|e| e.kind == KernelEventKind::RunCancelled));
        assert_eq!(retired.values.used_items(), 0);
        assert_eq!(call.provider.effects, 1);
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
