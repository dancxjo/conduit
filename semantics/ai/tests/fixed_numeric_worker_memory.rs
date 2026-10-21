#![cfg(feature = "kernel-operation-owners")]
use conduit_ai::{
    fixed_numeric_operations_back::{fixed_tanh_offer, FixedTanhBack},
    operation_owners::fixed_numeric::FixedNumericOperationFactory,
};
use conduit_composite::KernelOperationFactory;
use conduit_core::*;
use conduit_kernel::HostedValueStore;
use std::collections::BTreeMap;
#[path = "../../../architecture/core/tests/common/sealed_state.rs"]
mod common;
const PORTS: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
fn check<const W: usize>() {
    let offer = fixed_tanh_offer::<W>().unwrap();
    let mut fragment = common::fragment();
    fragment.states.clear();
    fragment.sign_storage_budget =
        mandatory_sign_storage_requirement(&fragment.expected_sign).unwrap();
    let gear = &mut fragment.placements[0];
    gear.kind_id = offer.kind_id.clone();
    gear.kind_contract_revision = offer.kind_contract_revision.clone();
    gear.execution_profile_id = offer.implementation.execution_profile_id.clone();
    gear.capability_id = offer.capability_id.clone();
    gear.implementation_id = offer.implementation.implementation_id.clone();
    gear.artifact_id = offer.implementation.artifact_id.clone();
    gear.realization_properties = offer.realization_properties.clone();
    gear.semantic_contract = offer.semantic_contract.clone();
    gear.limits = offer.limits.clone();
    gear.inputs = offer.inputs.clone();
    gear.outputs = offer.outputs.clone();
    let plan = common::seal(fragment);
    let owners = FixedNumericOperationFactory::for_plan(&plan, &BTreeMap::new()).unwrap();
    let gear = &plan.fragments[0].placements[0];
    let owner = FixedTanhBack::<W>::prepare_planned::<PORTS>(gear, 2).unwrap();
    let expected = core::mem::size_of_val(&owner) + owner.worker_owned_heap_bytes();
    let mut values = HostedValueStore::new(1, 16384, 16384).unwrap();
    let prepared = owners[0].prepare_worker(gear, &mut values).unwrap();
    assert_eq!(prepared.retained_bytes(), expected);
    assert!(expected > core::mem::size_of_val(&owner));
    let mut forged = gear.clone();
    forged.artifact_id = "other".into();
    assert!(owners[0].prepare_worker(&forged, &mut values).is_err());
}
#[test]
fn fixed_tanh_workers_account_for_inline_state_and_owned_codec_backing() {
    check::<4>();
    check::<40>();
    check::<64>();
    check::<128>();
    check::<160>();
    check::<192>();
    check::<320>();
}
