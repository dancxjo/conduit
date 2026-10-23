use super::push_canonical;
use crate::*;
use alloc::{vec, vec::Vec};
use sha2::{Digest, Sha256};

fn binding(pool: &str, class: &str, units: u32) -> ResourceBinding {
    ResourceBinding {
        pool_id: pool.into(),
        class_id: class.into(),
        units,
        protected: None,
        compute: None,
        content: None,
    }
}
fn region(scheduling: ExecutionScheduling) -> ExecutionRegion {
    let mut lane_resource = binding("pool/lane", "conduit.resource/execution-lane@1", 1);
    lane_resource.compute = Some(ComputeReservation {
        selected_lanes: 1,
        service_guarantee: ComputeServiceGuarantee::Shared,
        architecture_base_id: "base/test".into(),
        architecture_base_kind: ArchitectureBaseKind::BareMetal,
        topology_group_id: None,
        performance_class: None,
        nominal_clock_hz: None,
    });
    ExecutionRegion {
        region_id: "region/0".into(),
        admitted_placements: vec!["placement/0".into()],
        execution_profile_id: "test/execution@1".into(),
        scheduling,
        lane_count: 1,
        lane_resource,
        lane_base_id: "base/test".into(),
        requirements: ExecutionRegionRequirements {
            runtime_memory_bytes: 0,
            timer_slots: 0,
            cord_item_capacity: 0,
            cord_byte_capacity: 0,
            mandatory_sign_items: 0,
            mandatory_sign_bytes: 0,
        },
        preemption_required: false,
        isolation_required: false,
    }
}
fn bytes(region: &ExecutionRegion) -> Vec<u8> {
    let mut bytes = Vec::new();
    push_canonical(&mut bytes, core::slice::from_ref(region));
    bytes
}
fn protected() -> ExecutionScheduling {
    ExecutionScheduling::ProtectedPreemptibleStep {
        policy: ProtectedEntryBudget {
            accounting: ProtectedEntryAccounting::SourceInterruptsV1,
            quantum_interrupts: 16,
            total_interrupts: 64,
        },
        context: ExecutionContinuationStorage {
            binding: binding("pool/context", RUNTIME_MEMORY_RESOURCE_CLASS, 128),
            bytes: 128,
            alignment: 16,
        },
    }
}

#[test]
fn existing_execution_region_bytes_keep_prechange_golden_digests() {
    // Fixed vectors independently encoded from the 643c7649f0 contract, before
    // the new variant. These commit the complete region encoding, not its tag.
    let cases = [
        (
            ExecutionScheduling::CooperativeBoundedStep,
            177,
            [
                46, 237, 151, 213, 208, 113, 4, 162, 211, 227, 87, 238, 246, 149, 44, 124, 57, 251,
                150, 115, 103, 155, 248, 122, 139, 44, 73, 216, 230, 59, 33, 46,
            ],
        ),
        (
            ExecutionScheduling::PartitionedBoundedStep {
                storage: ExecutionWorkerStorage {
                    binding: binding("pool/worker", RUNTIME_MEMORY_RESOURCE_CLASS, 128),
                    stack_bytes: 32,
                    scratch_bytes: 16,
                },
            },
            243,
            [
                126, 217, 213, 99, 212, 254, 223, 84, 12, 196, 177, 90, 14, 51, 19, 141, 28, 131,
                210, 174, 86, 72, 129, 210, 236, 49, 201, 100, 149, 7, 106, 92,
            ],
        ),
    ];
    for (scheduling, length, expected) in cases {
        let encoded = bytes(&region(scheduling));
        assert_eq!(encoded.len(), length);
        let actual: [u8; 32] = Sha256::digest(encoded).into();
        assert_eq!(actual, expected);
    }
    assert_eq!(
        postcard::to_allocvec(&ExecutionScheduling::CooperativeBoundedStep).unwrap(),
        [0]
    );
}

#[test]
fn protected_policy_and_context_are_committed_without_partition_semantics() {
    let original = region(protected());
    assert!(!original.scheduling.is_partitioned());
    assert!(original.scheduling.worker_storage().is_none());
    let encoded = serde_json::to_vec(&original).unwrap();
    assert_eq!(
        serde_json::from_slice::<ExecutionRegion>(&encoded).unwrap(),
        original
    );
    for mutation in 0..8 {
        let mut changed = original.clone();
        let ExecutionScheduling::ProtectedPreemptibleStep { policy, context } =
            &mut changed.scheduling
        else {
            unreachable!()
        };
        match mutation {
            0 => policy.quantum_interrupts += 1,
            1 => policy.total_interrupts += 1,
            2 => context.binding.pool_id = "pool/other-context".into(),
            3 => context.binding.units += 1,
            4 => context.bytes += 16,
            5 => context.alignment *= 2,
            6 => changed.execution_profile_id = "test/other-profile@1".into(),
            _ => context.binding.class_id = "test/other-class".into(),
        }
        assert_ne!(bytes(&changed), bytes(&original), "mutation {mutation}");
    }
}

#[test]
fn optional_offer_keeps_existing_json_and_validates_finite_bounds() {
    let legacy = r#"{"service_guarantee":"Shared","architecture_base_id":"base/test","architecture_base_kind":"BareMetal","topology_groups":[]}"#;
    let mut contract: ComputePoolContract = serde_json::from_str(legacy).unwrap();
    assert_eq!(serde_json::to_string(&contract).unwrap(), legacy);
    assert!(contract.protected_preemption.is_none());
    let ExecutionScheduling::ProtectedPreemptibleStep { policy, context } = protected() else {
        unreachable!()
    };
    contract.protected_preemption = Some(ProtectedPreemptionOffer {
        execution_profile_id: "test/execution@1".into(),
        policy,
        context_bytes: context.bytes,
        context_alignment: context.alignment,
    });
    assert!(contract.is_valid_for_capacity(1));
    let encoded = serde_json::to_vec(&contract).unwrap();
    assert_eq!(
        serde_json::from_slice::<ComputePoolContract>(&encoded).unwrap(),
        contract
    );
    for mutation in 0..6 {
        let mut invalid = contract.clone();
        let offer = invalid.protected_preemption.as_mut().unwrap();
        match mutation {
            0 => offer.policy.quantum_interrupts = 0,
            1 => offer.policy.total_interrupts = 0,
            2 => offer.policy.quantum_interrupts = offer.policy.total_interrupts + 1,
            3 => offer.context_bytes = 0,
            4 => offer.context_alignment = 3,
            _ => offer.execution_profile_id = "".into(),
        }
        assert!(!invalid.is_valid_for_capacity(1), "mutation {mutation}");
    }
    assert!(serde_json::from_str::<ProtectedEntryAccounting>(r#""UnknownAccountingV2""#).is_err());
}
