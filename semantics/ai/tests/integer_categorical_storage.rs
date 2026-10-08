#![cfg(all(feature = "kernel-step", target_has_atomic = "ptr"))]
use conduit_ai::{integer_categorical_step::*, *};
use conduit_core::*;
use std::sync::Arc;
#[path = "common/integer_categorical_fixture.rs"]
#[allow(dead_code)]
mod fixture;
use fixture::*;
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
#[allow(dead_code)]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[test]
fn bounded_model_custody_counts_original_capacity_and_refuses_before_allocation() {
    let (resource, mut residence) = fixture();
    let baseline = categorical_resource_retained_bytes(&resource).unwrap();
    let mut artifact = resource.artifact().clone();
    artifact.architecture_profile.reserve(4096);
    artifact.content.content_profile.0.reserve(2048);
    let mut signature = resource.signature().clone();
    signature.identity.reserve(4096);
    let port = &signature.inputs.get()[0];
    let mut name = port.identity().get().clone();
    name.reserve(8192);
    signature.inputs = ModelPorts::new(
        conduit_plot::rust_binding::BoundedSequence::try_from_iter([ModelPortConstraint::new(
            ModelPortIdentity::new(name).unwrap(),
            *port.presence(),
            port.semantic_kind().clone(),
            port.value().clone(),
        )
        .unwrap()])
        .unwrap(),
    )
    .unwrap();
    let reference = &artifact.content;
    let binding = ResourceReferenceBinding {
        identity: reference.identity,
        version: reference.lifetime.version,
        content_profile: reference.content_profile.clone(),
        access_class: reference.access_class.clone(),
        handle: resource.access().handle.clone(),
        authority_contract: MODEL_READ_AUTHORITY.into(),
        authority_grant: resource.access().authority_grant.clone(),
        maximum_bytes: reference.extent.bytes,
        maximum_items: reference.extent.items,
        availability: ResourceReferenceAvailability::Available,
    };
    let resource = Arc::new(
        AdmittedModelResource::adopt(artifact, signature, resource.shared_storage(), &binding)
            .unwrap(),
    );
    let (retained, observed) =
        allocation_probe::observe(|| categorical_resource_retained_bytes(&resource));
    let retained = retained.unwrap();
    assert!(retained > baseline + 16 * 1024);
    assert_eq!(observed.allocations, 0);
    assert_eq!(observed.reallocations, 0);
    residence.owner_host.0.reserve(4096);
    let mut pool = ResourcePoolId::from("test/model-pool");
    pool.0.reserve(2048);
    let (reservation, observed) = allocation_probe::observe(|| {
        PreparedCategoricalStep::storage_reservation(&resource, &pool, &residence)
    });
    let reservation = reservation.unwrap();
    assert_eq!(observed.allocations, 0);
    assert_eq!(observed.reallocations, 0);
    let mut limits = CategoricalStepStorageLimits {
        maximum_retained_bytes: reservation.retained_heap_bytes_bound,
        maximum_preparation_peak_bytes: reservation.preparation_peak_heap_bytes_bound,
    };
    limits.maximum_preparation_peak_bytes -= 1;
    let inputs = (resource.clone(), pool.clone(), residence.clone());
    // Clones can shrink capacity. Use their own exact reservation for this gate.
    let under =
        PreparedCategoricalStep::storage_reservation(&inputs.0, &inputs.1, &inputs.2).unwrap();
    limits.maximum_preparation_peak_bytes = under.preparation_peak_heap_bytes_bound - 1;
    let (refused, observed) = allocation_probe::observe(|| {
        PreparedCategoricalStep::prepare_bounded(inputs.0, inputs.1, inputs.2, limits)
    });
    assert!(matches!(refused, Err(CategoricalStorageRefusal::Pressure)));
    assert_eq!(observed.allocations, 0);
    assert_eq!(observed.reallocations, 0);
    let inputs = (resource.clone(), pool.clone(), residence.clone());
    let quota =
        PreparedCategoricalStep::storage_reservation(&inputs.0, &inputs.1, &inputs.2).unwrap();
    let under_retained = CategoricalStepStorageLimits {
        maximum_retained_bytes: quota.retained_heap_bytes_bound - 1,
        maximum_preparation_peak_bytes: quota.preparation_peak_heap_bytes_bound,
    };
    let (refused, observed) = allocation_probe::observe(|| {
        PreparedCategoricalStep::prepare_bounded(inputs.0, inputs.1, inputs.2, under_retained)
    });
    assert!(matches!(refused, Err(CategoricalStorageRefusal::Pressure)));
    assert_eq!(observed.allocations, 0);
    assert_eq!(observed.reallocations, 0);
    limits.maximum_preparation_peak_bytes = reservation.preparation_peak_heap_bytes_bound;
    let (owner, observed) = allocation_probe::observe(|| {
        PreparedCategoricalStep::prepare_bounded(resource.clone(), pool, residence, limits)
    });
    let owner = owner.unwrap();
    assert!(observed.peak_bytes <= reservation.preparation_peak_heap_bytes_bound);
    let (receipt, observed) = allocation_probe::observe(|| owner.storage_receipt());
    let receipt = receipt.unwrap();
    assert_eq!(observed.allocations, 0);
    assert_eq!(observed.reallocations, 0);
    assert_eq!(receipt.original_resource_retained_bytes, retained);
    assert_eq!(receipt.weights_retained_bytes, 12);
    assert!(receipt.type_retained_bytes > 0);
    assert!(receipt.execution_metadata_retained_bytes > 0);
    assert_eq!(
        receipt.retained_heap_bytes_bound,
        receipt.original_resource_retained_bytes
            + receipt.weights_retained_bytes
            + receipt.type_retained_bytes
            + receipt.execution_metadata_retained_bytes
    );
    assert!(core::ptr::eq(owner.resource(), resource.as_ref()));
    let mut scores = [0; 2];
    owner.infer_indices_into(&[0, 2], &mut scores).unwrap();
    assert_eq!(scores, [3, 8]);
}

#[test]
fn largest_valid_model_has_bounded_private_construction_and_full_weights() {
    let mut bytes = b"CI16SUM1".to_vec();
    for n in [256u32, 128, 64] {
        bytes.extend_from_slice(&n.to_le_bytes());
    }
    for _ in 0..32768 {
        bytes.extend_from_slice(&1i16.to_le_bytes());
    }
    let (model, residence) = fixture_model(bytes, 64, 128);
    let pool = "test/model-pool".into();
    let r = PreparedCategoricalStep::storage_reservation(&model, &pool, &residence).unwrap();
    let (owner, observed) = allocation_probe::observe(|| {
        PreparedCategoricalStep::prepare_bounded(
            model,
            pool,
            residence,
            CategoricalStepStorageLimits {
                maximum_retained_bytes: r.retained_heap_bytes_bound,
                maximum_preparation_peak_bytes: r.preparation_peak_heap_bytes_bound,
            },
        )
    });
    let owner = owner.unwrap();
    assert!(observed.peak_bytes <= r.preparation_peak_heap_bytes_bound);
    let receipt = owner.storage_receipt().unwrap();
    assert_eq!(receipt.weights_retained_bytes, 65536);
    assert!(receipt.retained_heap_bytes_bound <= r.retained_heap_bytes_bound);
    let mut scores = [0; 128];
    let (result, observed) =
        allocation_probe::observe(|| owner.infer_indices_into(&[255; 64], &mut scores));
    result.unwrap();
    assert_eq!(observed.allocations, 0);
    assert_eq!(observed.reallocations, 0);
    assert_eq!(scores, [64; 128]);
}

#[test]
fn independent_model_metadata_counts_capacity_without_allocation() {
    let (resource, _) = fixture();
    let mut artifact = resource.artifact().clone();
    let mut signature = resource.signature().clone();
    let before = categorical_model_metadata_storage_receipt(&artifact, &signature).unwrap();
    artifact.architecture_profile.reserve(4096);
    artifact.content.content_profile.0.reserve(2048);
    signature.identity.reserve(8192);
    let (receipt, observed) = allocation_probe::observe(|| {
        categorical_model_metadata_storage_receipt(&artifact, &signature)
    });
    let receipt = receipt.unwrap();
    assert_eq!(observed.allocations, 0);
    assert_eq!(observed.reallocations, 0);
    assert_eq!(
        receipt.inline_bytes,
        core::mem::size_of::<ModelArtifact>() + core::mem::size_of::<ModelSignature>()
    );
    assert_eq!(
        receipt.combined_bytes_bound,
        receipt.inline_bytes + receipt.owned_heap_bytes_bound
    );
    assert!(receipt.owned_heap_bytes_bound >= before.owned_heap_bytes_bound + 12 * 1024);
}
