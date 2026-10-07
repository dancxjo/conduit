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
