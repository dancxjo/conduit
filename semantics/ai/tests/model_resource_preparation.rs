use conduit_ai::*;
use conduit_core::*;
use conduit_plot::rust_binding::{
    NativeRustBinding, PreparedNativeFamily, PreparedNativeFamilyLimits, PreparedNativeRustBinding,
};
use std::sync::Arc;
#[path = "../src/numeric_allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
const SIGNATURE_ID: [u8; 32] = [
    195, 51, 206, 70, 197, 40, 164, 97, 113, 227, 253, 4, 163, 116, 180, 71, 102, 154, 139, 199,
    65, 221, 244, 200, 147, 122, 246, 223, 149, 52, 165, 201,
];
const DESCRIPTOR_ID: [u8; 32] = [
    49, 180, 175, 29, 65, 90, 175, 178, 121, 72, 106, 133, 120, 160, 97, 26, 13, 128, 118, 200,
    131, 222, 170, 232, 243, 27, 8, 64, 84, 170, 53, 70,
];
fn fixture() -> (
    ModelArtifact,
    ModelSignature,
    Arc<[u8]>,
    ResourceReferenceBinding,
    Vec<u8>,
) {
    let frame = include_bytes!("fixtures/fargan-compound-model-signature.native.bin").to_vec();
    let limits = PreparedNativeFamilyLimits {
        maximum_types: 64,
        maximum_laws_per_type: 256,
        maximum_input_bytes: 262144,
        maximum_retained_bytes: 128 * 1024 * 1024,
        maximum_preparation_peak_bytes: 256 * 1024 * 1024,
        maximum_conversion_requested_bytes: 1024 * 1024 * 1024,
    };
    let mut family =
        PreparedNativeFamily::prepare(&[ModelSignature::PREPARED_DESCRIPTOR], limits).unwrap();
    let signature = family.decode::<ModelSignature>(&frame).unwrap();
    assert_eq!(signature.semantic_digest().unwrap(), SIGNATURE_ID);
    let bytes: Arc<[u8]> = std::fs::read(
        std::path::PathBuf::from(
            std::env::var_os("CONDUIT_FARGAN_MODEL_FIXTURE")
                .expect("set original FARGAN fixture root"),
        )
        .join("resources/f32.bin"),
    )
    .unwrap()
    .into();
    let version = ResourceVersionIdentity::from_digest([
        104, 186, 110, 135, 49, 0, 127, 113, 132, 190, 134, 45, 125, 26, 254, 76, 239, 9, 106, 124,
        155, 203, 250, 17, 89, 98, 52, 15, 114, 94, 0, 76,
    ]);
    let content = BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest(model_content_digest(&bytes)),
        content_profile: kind_id("model/fargan-array-bundle-f32-le@1"),
        access_class: ResourceClassId::from("private-development-model/read@1"),
        extent: ResourceExtent {
            bytes: bytes.len() as u64,
            items: None,
        },
        lifetime: ResourceLifetime {
            version,
            expires_at: None,
        },
    };
    let artifact = ModelArtifact {
        architecture_profile: "speech/fargan-xiph-503d81b1@1".into(),
        format_profile: "model/fargan-array-bundle-f32-le@1".into(),
        precision_profile: "number/ieee754-f32-libm-source-pcm16-nearest-away@1".into(),
        state_schema_version: 1,
        signature_identity: SIGNATURE_ID,
        content,
    };
    let binding = ResourceReferenceBinding {
        identity: artifact.content.identity,
        version,
        content_profile: artifact.content.content_profile.clone(),
        access_class: artifact.content.access_class.clone(),
        handle: ResourceHandleId::from("private-fargan/model"),
        authority_contract: AuthorityContractId::from(MODEL_READ_AUTHORITY),
        authority_grant: AuthorityGrantId::from("private-fargan-read/model"),
        maximum_bytes: bytes.len() as u64,
        maximum_items: None,
        availability: ResourceReferenceAvailability::Available,
    };
    (artifact, signature, bytes, binding, frame)
}
#[test]
#[ignore = "requires exact original FARGAN fixture root"]
fn exact_original_digest_and_bounded_adoption_preserve_custody() {
    let (artifact, signature, bytes, binding, frame) = fixture();
    let (len, noalloc) = allocation_probe::observe(|| signature.digest_encoding_length().unwrap());
    assert_eq!((noalloc.allocations, noalloc.reallocations), (0, 0));
    let (digest, observed) = allocation_probe::observe(|| signature.semantic_digest().unwrap());
    assert_eq!(digest, SIGNATURE_ID);
    assert_eq!(observed.requested_bytes, len);
    assert_eq!(observed.live_bytes, 0);
    let (reservation, o) = allocation_probe::observe(|| {
        AdmittedModelResource::storage_reservation(&artifact, &signature, &bytes, &binding).unwrap()
    });
    assert_eq!((o.allocations, o.reallocations), (0, 0));
    let original = Arc::clone(&bytes);
    let (result, o) = allocation_probe::observe(|| {
        AdmittedModelResource::adopt_with_storage_limits(
            artifact,
            signature,
            bytes,
            &binding,
            reservation.preparation_requested_bytes_bound(),
            reservation.retained_payload_bytes_bound(),
        )
    });
    let (model, receipt) = result.unwrap();
    assert!(o.requested_bytes <= receipt.preparation_requested_bytes_bound());
    assert_eq!(
        model.owned_payload_bytes(),
        receipt.retained_payload_bytes_bound()
    );
    assert_eq!(model.descriptor_identity(), DESCRIPTOR_ID);
    assert!(model.shares_storage_with(&original));
    assert_eq!(model.signature().clone().encode().unwrap(), frame);
    println!(
        "signature bytes={len}, adoption requested={} peak={} newlive={} fullretainedpayload={}",
        o.requested_bytes,
        o.peak_bytes,
        o.live_bytes,
        model.owned_payload_bytes()
    );
}
#[test]
#[ignore = "requires exact original FARGAN fixture root"]
fn both_one_under_ceilings_refuse_before_allocation() {
    for which in 0..2 {
        let (a, s, b, binding, _) = fixture();
        let r = AdmittedModelResource::storage_reservation(&a, &s, &b, &binding).unwrap();
        let p = r.preparation_requested_bytes_bound() - usize::from(which == 0);
        let h = r.retained_payload_bytes_bound() - usize::from(which == 1);
        let (result, o) = allocation_probe::observe(|| {
            AdmittedModelResource::adopt_with_storage_limits(a, s, b, &binding, p, h)
        });
        assert!(matches!(
            result,
            Err(ModelResourcePreparationRefusal::Capacity)
        ));
        assert_eq!((o.allocations, o.reallocations), (0, 0));
    }
}
#[test]
#[ignore = "requires exact original FARGAN fixture root"]
fn original_content_authority_and_signature_refusals_remain() {
    for which in 0..3 {
        let (mut a, s, mut b, mut binding, _) = fixture();
        match which {
            0 => {
                let mut v = b.to_vec();
                v[0] ^= 1;
                b = v.into()
            }
            1 => binding.authority_grant = AuthorityGrantId::from(""),
            _ => a.signature_identity = [1; 32],
        };
        let r = AdmittedModelResource::storage_reservation(&a, &s, &b, &binding).unwrap();
        let (result, _) = allocation_probe::observe(|| {
            AdmittedModelResource::adopt_with_storage_limits(
                a,
                s,
                b,
                &binding,
                r.preparation_requested_bytes_bound(),
                r.retained_payload_bytes_bound(),
            )
        });
        match (which, result) {
            (0, Err(ModelResourcePreparationRefusal::Adoption(ModelResourceRefusal::Content)))
            | (
                1,
                Err(ModelResourcePreparationRefusal::Adoption(ModelResourceRefusal::Authority(
                    ResourceReferenceAccessRefusal::EmptyAuthorityGrant,
                ))),
            )
            | (
                2,
                Err(ModelResourcePreparationRefusal::Adoption(
                    ModelResourceRefusal::Compatibility(
                        ModelCompatibilityRefusal::SignatureMismatch,
                    ),
                )),
            ) => (),
            _ => panic!("adoption guard changed"),
        }
    }
}

#[test]
#[ignore = "requires exact original FARGAN fixture and artifact roots"]
fn aggregate_original_owners_deduplicate_identity_and_reserve_before_copy() {
    use conduit_ai::{
        fixed_tensor_resource::AdmittedFixedTensorResource, numeric_resource_owners::*,
    };
    use conduit_data::{TensorAxis, TensorAxisRole, TensorBacking, TensorElement, TensorValue};
    use conduit_plot::rust_binding::BoundedSequence;
    let (artifact, signature, bytes, binding, _) = fixture();
    let model = Arc::new(
        AdmittedModelResource::adopt(artifact, signature, bytes.clone(), &binding).unwrap(),
    );
    let project = std::path::PathBuf::from(
        std::env::var_os("CONDUIT_FARGAN_PREPARATION_ARTIFACT_ROOT").unwrap(),
    );
    let plan = Arc::new(
        serde_json::from_slice::<Plan>(
            &std::fs::read(
                project.join("outputs/committed-common-fargan-greeting/sealed-epoch-plan.json"),
            )
            .unwrap(),
        )
        .unwrap(),
    );
    let source: Arc<str> = std::fs::read_to_string(
        project.join("outputs/committed-common-fargan-greeting/checked-epoch-source.conduit"),
    )
    .unwrap()
    .into();
    let digest = conduit_data::tensor_content_digest(&bytes[..4]);
    let reference = BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest(digest),
        content_profile: kind_id("tensor/elements-ieee754-f32-le@1"),
        access_class: "private-tensor/read@1".into(),
        extent: ResourceExtent {
            bytes: 4,
            items: Some(1),
        },
        lifetime: binding_version(&binding),
    };
    let tensor = Arc::new(TensorValue {
        element: TensorElement::F32,
        dimensions: BoundedSequence::try_from_iter([1]).unwrap(),
        axes: BoundedSequence::try_from_iter([TensorAxis {
            identity: None,
            role: TensorAxisRole::other("parameter".into()).unwrap(),
            unit: None,
        }])
        .unwrap(),
        backing: TensorBacking::Resource(reference.clone()),
        content_digest: digest,
    });
    let tensor_binding = ResourceReferenceBinding {
        identity: reference.identity,
        version: reference.lifetime.version,
        content_profile: reference.content_profile.clone(),
        access_class: reference.access_class.clone(),
        handle: "tensor-handle".into(),
        authority_contract: conduit_ai::fixed_numeric_preparation::TENSOR_READ_AUTHORITY.into(),
        authority_grant: "tensor-grant".into(),
        maximum_bytes: 4,
        maximum_items: Some(1),
        availability: ResourceReferenceAvailability::Available,
    };
    let first = Arc::new(
        AdmittedFixedTensorResource::adopt_shared_slice(
            tensor.clone(),
            bytes.clone(),
            0..4,
            &tensor_binding,
        )
        .unwrap(),
    );
    let second = Arc::new(
        AdmittedFixedTensorResource::adopt_shared_slice(
            tensor.clone(),
            bytes.clone(),
            0..4,
            &tensor_binding,
        )
        .unwrap(),
    );
    let tensors = [first.clone(), first.clone(), second.clone()];
    let equal_distinct: Arc<[u8]> = Arc::from(bytes.as_ref());
    let buffers = [bytes.clone(), bytes.clone(), equal_distinct.clone()];
    let (reservation, probe) = allocation_probe::observe(|| {
        PreparedNumericResourceOwners::storage_reservation(
            &model, &plan, &source, &tensors, &buffers,
        )
        .unwrap()
    });
    assert_eq!((probe.allocations, probe.reallocations), (0, 0));
    assert_eq!(reservation.unique_tensor_resources(), 2);
    assert_eq!(reservation.unique_tensor_descriptors(), 1);
    assert_eq!(reservation.unique_byte_backings(), 2);
    assert_eq!(reservation.shared_allocations(), 8);
    let limits = NumericResourceStorageLimits {
        maximum_preparation_requested_bytes: reservation.preparation_requested_bytes_bound(),
        maximum_retained_payload_bytes: reservation.retained_payload_bytes(),
        maximum_shared_allocations: reservation.shared_allocations(),
    };
    for which in 0..3 {
        let mut under = limits;
        match which {
            0 => under.maximum_preparation_requested_bytes -= 1,
            1 => under.maximum_retained_payload_bytes -= 1,
            _ => under.maximum_shared_allocations -= 1,
        };
        let (result, observed) = allocation_probe::observe(|| {
            PreparedNumericResourceOwners::prepare(
                &model, &plan, &source, &tensors, &buffers, under,
            )
        });
        assert!(matches!(
            result,
            Err(NumericResourceStorageRefusal::Capacity)
        ));
        assert_eq!((observed.allocations, observed.reallocations), (0, 0));
    }
    let (owner, observed) = allocation_probe::observe(|| {
        PreparedNumericResourceOwners::prepare(&model, &plan, &source, &tensors, &buffers, limits)
            .unwrap()
    });
    assert_eq!(
        observed.requested_bytes,
        reservation.preparation_requested_bytes_bound()
    );
    assert_eq!(
        observed.live_bytes,
        reservation.preparation_requested_bytes_bound()
    );
    assert_eq!(owner.receipt(), reservation);
    assert!(Arc::ptr_eq(owner.model(), &model));
    assert!(Arc::ptr_eq(owner.plan(), &plan));
    assert!(Arc::ptr_eq(owner.source(), &source));
    for (actual, original) in owner.tensors().iter().zip(&tensors) {
        assert!(Arc::ptr_eq(actual, original));
    }
    for (actual, original) in owner.buffers().iter().zip(&buffers) {
        assert!(Arc::ptr_eq(actual, original));
    }
    assert!(!Arc::ptr_eq(&buffers[0], &buffers[2]));
    let more = Arc::new(
        AdmittedFixedTensorResource::adopt_shared_slice(
            Arc::new((*tensor).clone()),
            bytes.clone(),
            0..4,
            &tensor_binding,
        )
        .unwrap(),
    );
    let distinct = [first, second, more];
    let r = PreparedNumericResourceOwners::storage_reservation(
        &model, &plan, &source, &distinct, &buffers,
    )
    .unwrap();
    assert_eq!(r.unique_tensor_descriptors(), 2);
    assert_eq!(r.unique_tensor_resources(), 3);
    assert_eq!(r.unique_byte_backings(), 2);
    assert!(r.retained_payload_bytes() > reservation.retained_payload_bytes());
    println!(
        "aggregate original owners requested={} retained_payload={} shared_allocations={} resources={} descriptors={} backings={}",
        reservation.preparation_requested_bytes_bound(),
        reservation.retained_payload_bytes(),
        reservation.shared_allocations(),
        reservation.unique_tensor_resources(),
        reservation.unique_tensor_descriptors(),
        reservation.unique_byte_backings()
    );
}
fn binding_version(binding: &ResourceReferenceBinding) -> ResourceLifetime {
    ResourceLifetime {
        version: binding.version,
        expires_at: None,
    }
}
