#![cfg(target_has_atomic = "ptr")]
use conduit_ai::{
    fixed_numeric_preparation::*, fixed_tensor::*, fixed_tensor_linear::*, fixed_tensor_resource::*,
};
use conduit_core::*;
use conduit_data::*;
use conduit_plot::rust_binding::BoundedSequence;
use std::sync::Arc;
fn tensor(shape: &[u64], bytes: &[u8]) -> TensorValue {
    let digest = tensor_content_digest(bytes);
    TensorValue {
        element: TensorElement::F32,
        dimensions: BoundedSequence::try_from_iter(shape.iter().copied()).unwrap(),
        axes: BoundedSequence::try_from_iter(shape.iter().map(|_| TensorAxis {
            role: TensorAxisRole::Feature,
            identity: None,
            unit: None,
        }))
        .unwrap(),
        content_digest: digest,
        backing: TensorBacking::Resource(BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest(digest),
            content_profile: kind_id("tensor/elements-ieee754-f32-le@1"),
            access_class: ResourceClassId::from("test/read@1"),
            extent: ResourceExtent {
                bytes: bytes.len() as u64,
                items: Some(shape.iter().product()),
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest([1; 32]),
                expires_at: None,
            },
        }),
    }
}
fn access(tensor: &TensorValue) -> ResourceReferenceBinding {
    let TensorBacking::Resource(reference) = &tensor.backing else {
        panic!("resource fixture")
    };
    ResourceReferenceBinding {
        identity: reference.identity,
        version: reference.lifetime.version,
        content_profile: reference.content_profile.clone(),
        access_class: reference.access_class.clone(),
        handle: ResourceHandleId::from("numeric-test-immutable"),
        authority_contract: AuthorityContractId::from(TENSOR_READ_AUTHORITY),
        authority_grant: AuthorityGrantId::from("numeric-test-read"),
        maximum_bytes: reference.extent.bytes,
        maximum_items: reference.extent.items,
        availability: ResourceReferenceAvailability::Available,
    }
}
fn packed(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|x| x.to_le_bytes()).collect()
}
#[test]
fn owned_resource_custody_outlives_loader_without_copying_content() {
    let bytes: Arc<[u8]> = Arc::from(packed(&[1., 4., 2., 5., 3., 6.]));
    let descriptor = Arc::new(tensor(&[3, 2], &bytes));
    let adoption = Arc::new(
        AdmittedFixedTensorResource::adopt(descriptor.clone(), bytes.clone(), &access(&descriptor))
            .unwrap(),
    );
    let pointer = adoption.bytes().as_ptr();
    let retained = adoption.content_and_inline_descriptor_bytes();
    let view =
        FixedTensorLinear::<3, 2>::prepare_owned(adoption.clone(), FixedMatrixOrder::InputMajor)
            .unwrap();
    assert_eq!(adoption.bytes().as_ptr(), pointer);
    assert_eq!(Arc::strong_count(&bytes), 2);
    drop(bytes);
    drop(descriptor);
    drop(adoption);
    let mut output = [0.; 2];
    view.apply(&[2., -1., 0.5], &mut output).unwrap();
    assert_eq!(output, [1.5, 6.]);
    println!(
        "tensor inline descriptor={}B content plus inline descriptor={}B; dynamic metadata and Arc headers additional",
        core::mem::size_of::<TensorValue>(),
        retained
    );
}
#[test]
fn resource_adoption_refuses_changed_content_and_lost_authority() {
    let bytes: Arc<[u8]> = Arc::from(packed(&[1., 2.]));
    let descriptor = Arc::new(tensor(&[2], &bytes));
    let mut grant = access(&descriptor);
    grant.availability = ResourceReferenceAvailability::Lost;
    assert!(AdmittedFixedTensorResource::adopt(descriptor.clone(), bytes.clone(), &grant).is_err());
    let changed: Arc<[u8]> = Arc::from(packed(&[2., 1.]));
    assert!(
        AdmittedFixedTensorResource::adopt(descriptor.clone(), changed, &access(&descriptor))
            .is_err()
    );
}

#[test]
fn exact_disjoint_slices_share_one_retained_blob_and_outlive_loader() {
    let blob: Arc<[u8]> = Arc::from(packed(&[1., 4., 2., 5., 3., 6., 0.5, -0.5]));
    let weights = Arc::new(tensor(&[3, 2], &blob[..24]));
    let bias = Arc::new(tensor(&[2], &blob[24..]));
    let weights = Arc::new(
        AdmittedFixedTensorResource::adopt_shared_slice(
            weights.clone(),
            blob.clone(),
            0..24,
            &access(&weights),
        )
        .unwrap(),
    );
    let bias = Arc::new(
        AdmittedFixedTensorResource::adopt_shared_slice(
            bias.clone(),
            blob.clone(),
            24..32,
            &access(&bias),
        )
        .unwrap(),
    );
    assert!(weights.shares_storage_with(&bias));
    assert_eq!(weights.retained_storage_bytes(), 32);
    assert_eq!(bias.retained_storage_bytes(), 32);
    assert_eq!(weights.bytes().as_ptr(), blob.as_ptr());
    assert_eq!(bias.bytes().as_ptr(), blob[24..].as_ptr());
    let linear =
        FixedTensorLinear::<3, 2>::prepare_owned(weights.clone(), FixedMatrixOrder::InputMajor)
            .unwrap();
    drop(blob);
    drop(weights);
    let mut result = [0.; 2];
    linear.apply(&[2., -1., 0.5], &mut result).unwrap();
    assert_eq!(result, [1.5, 6.]);
    assert_eq!(bias.bytes(), packed(&[0.5, -0.5]));
}
#[test]
fn shared_slices_refuse_bad_ranges_changed_bytes_extent_and_authority() {
    let blob: Arc<[u8]> = Arc::from(packed(&[1., 2., 3., 4.]));
    let descriptor = Arc::new(tensor(&[2], &blob[..8]));
    let grant = access(&descriptor);
    for range in [
        core::ops::Range { start: 8, end: 4 },
        0..17,
        usize::MAX..usize::MAX,
    ] {
        assert!(matches!(
            AdmittedFixedTensorResource::adopt_shared_slice(
                descriptor.clone(),
                blob.clone(),
                range,
                &grant
            ),
            Err(FixedResourceAdoptionRefusal::ContentRange)
        ));
    }
    for range in [8..16, 0..4] {
        assert!(matches!(
            AdmittedFixedTensorResource::adopt_shared_slice(
                descriptor.clone(),
                blob.clone(),
                range,
                &grant
            ),
            Err(FixedResourceAdoptionRefusal::Tensor(
                FixedTensorRefusal::ContentIdentity
            ))
        ));
    }
    let mut lost = grant;
    lost.availability = ResourceReferenceAvailability::Lost;
    assert!(matches!(
        AdmittedFixedTensorResource::adopt_shared_slice(
            descriptor.clone(),
            blob.clone(),
            0..8,
            &lost
        ),
        Err(FixedResourceAdoptionRefusal::Authority(_))
    ));
}
