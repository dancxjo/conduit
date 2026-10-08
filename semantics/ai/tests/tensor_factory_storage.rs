#![cfg(feature = "fixed-numeric-owners")]
use conduit_ai::{
    fixed_tensor_resource::AdmittedFixedTensorResource, operation_owners as owners, *,
};
use conduit_core::*;
use conduit_data::*;
use conduit_plot::rust_binding::{
    BoundedSequence, PreparedNativeFamily, PreparedNativeFamilyLimits, PreparedNativeRustBinding,
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};
#[path = "../src/numeric_allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
const SIGNATURE_ID: [u8; 32] = [
    195, 51, 206, 70, 197, 40, 164, 97, 113, 227, 253, 4, 163, 116, 180, 71, 102, 154, 139, 199,
    65, 221, 244, 200, 147, 122, 246, 223, 149, 52, 165, 201,
];
fn original_model_parts(
    bytes: Arc<[u8]>,
) -> (
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

fn pinned(path: PathBuf, hash: &str) -> Vec<u8> {
    let bytes = std::fs::read(path).unwrap();
    assert_eq!(format!("{:x}", Sha256::digest(&bytes)), hash);
    bytes
}
fn resource(
    shape: &[u64],
    backing: Arc<[u8]>,
    range: std::ops::Range<usize>,
) -> Arc<AdmittedFixedTensorResource> {
    let content = &backing[range.clone()];
    let digest = tensor_content_digest(content);
    let reference = BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest(digest),
        content_profile: kind_id("tensor/elements-ieee754-f32-le@1"),
        access_class: "private-development-tensor/read@1".into(),
        extent: ResourceExtent {
            bytes: content.len() as u64,
            items: Some(shape.iter().product()),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([
                104, 186, 110, 135, 49, 0, 127, 113, 132, 190, 134, 45, 125, 26, 254, 76, 239, 9,
                106, 124, 155, 203, 250, 17, 89, 98, 52, 15, 114, 94, 0, 76,
            ]),
            expires_at: None,
        },
    };
    let tensor = Arc::new(TensorValue {
        element: TensorElement::F32,
        dimensions: BoundedSequence::try_from_iter(shape.iter().copied()).unwrap(),
        axes: BoundedSequence::try_from_iter(shape.iter().map(|_| TensorAxis {
            identity: None,
            role: TensorAxisRole::Feature,
            unit: None,
        }))
        .unwrap(),
        backing: TensorBacking::Resource(reference.clone()),
        content_digest: digest,
    });
    let access = ResourceReferenceBinding {
        identity: reference.identity,
        version: reference.lifetime.version,
        content_profile: reference.content_profile.clone(),
        access_class: reference.access_class.clone(),
        handle: "private-fargan/storage-test".into(),
        authority_contract: conduit_ai::fixed_numeric_preparation::TENSOR_READ_AUTHORITY.into(),
        authority_grant: "private-fargan-read/storage-test".into(),
        maximum_bytes: reference.extent.bytes,
        maximum_items: reference.extent.items,
        availability: ResourceReferenceAvailability::Available,
    };
    Arc::new(
        AdmittedFixedTensorResource::adopt_shared_slice(tensor, backing, range, &access).unwrap(),
    )
}
#[test]
#[ignore = "requires exact archived original FARGAN artifact/model/source roots"]
fn full_original_tensor_factory_arrays_and_shared_owner_custody() {
    let project =
        PathBuf::from(std::env::var_os("CONDUIT_FARGAN_PREPARATION_ARTIFACT_ROOT").unwrap());
    let source = PathBuf::from(std::env::var_os("CONDUIT_FARGAN_PREPARATION_SOURCE_ROOT").unwrap());
    let fixture = PathBuf::from(std::env::var_os("CONDUIT_FARGAN_MODEL_FIXTURE").unwrap());
    let original_plan_bytes: Arc<[u8]> = pinned(
        project.join("outputs/committed-common-fargan-greeting/sealed-epoch-plan.json"),
        "da25c8c1718fd387cd7a2504b129ba5cb326a269c920414f045df92662d162e8",
    )
    .into();
    let plan: Plan = serde_json::from_slice(&original_plan_bytes).unwrap();
    let blob: Arc<[u8]> = pinned(
        fixture.join("resources/f32.bin"),
        "d35f0510da476716183a33caafe45cc095e48d496f06de2db7655e780a385e47",
    )
    .into();
    let mut named = BTreeMap::new();
    for (file, hash, prefix) in [
        (
            "signal-layout-f32.json",
            "db7dc32dedaddd4aa924521cca834116b839e6236b370a3121e80d943901a4d9",
            "",
        ),
        (
            "conditioning-layout-f32.json",
            "d80fda0e9f9731e4b0a3091cb2f25c492ccaa1601d870b7cd10de18897af2c5c",
            "conditioning_",
        ),
    ] {
        let layout: serde_json::Value =
            serde_json::from_slice(&pinned(source.join("proof/fargan").join(file), hash)).unwrap();
        for row in layout["resources"].as_array().unwrap() {
            let offset = row["offset"].as_u64().unwrap() as usize;
            let end = offset + row["bytes"].as_u64().unwrap() as usize;
            assert_eq!(
                format!("{:x}", Sha256::digest(&blob[offset..end])),
                row["raw_sha256"].as_str().unwrap()
            );
            let shape: Vec<u64> = row["shape"]
                .as_array()
                .unwrap()
                .iter()
                .map(|n| n.as_u64().unwrap())
                .collect();
            let value = resource(&shape, blob.clone(), offset..end);
            assert!(value.shares_storage_arc(&blob));
            assert!(
                named
                    .insert(format!("{prefix}{}", row["port"].as_str().unwrap()), value)
                    .is_none()
            );
        }
    }
    for (name, file, hash, shape) in [
        (
            "band_weights",
            "bands161x18.bin",
            "f3e181ecfdf3a579b76699a0924efebbb2ba6e0a9837cb265a87e929c18da1c3",
            vec![161, 18],
        ),
        (
            "band_bias",
            "band_bias18.bin",
            "ccc8ef501158d683a793b5de106853c5d7a556205bc98003893c0c9d5c12f123",
            vec![18],
        ),
        (
            "dct_weights",
            "dct18x18.bin",
            "0ee4290f4ec2b3408882aefe5d379cd9a8e0050b319a0e67d297d22442b928a5",
            vec![18, 18],
        ),
    ] {
        let backing: Arc<[u8]> =
            pinned(source.join("proof/fargan/feature-profile").join(file), hash).into();
        let length = backing.len();
        assert!(
            named
                .insert(name.into(), resource(&shape, backing, 0..length))
                .is_none()
        );
    }
    assert_eq!(named.len(), 36);
    let mut sources = BTreeMap::new();
    for gear in &plan.fragments[0].placements {
        if let Some(name) = gear.kind_id.as_str().strip_prefix("epoch-proof/")
            && let Some(resource) = named.get(name)
        {
            sources.insert(gear.placement_id.clone(), resource.clone());
        }
    }
    assert_eq!(sources.len(), 36);
    let shared_visits = std::cell::Cell::new(0);
    let mut visitor = |value: &Arc<AdmittedFixedTensorResource>| {
        assert!(
            sources
                .values()
                .any(|original| Arc::ptr_eq(value, original))
        );
        shared_visits.set(shared_visits.get() + 1);
    };
    macro_rules! test_factory {
        ($label:literal, $factory:expr) => {{
            let (factory, o) = allocation_probe::observe(|| $factory.unwrap());
            assert_eq!(o.live_bytes, factory.local_owned_payload_bytes().unwrap());
            let (_, visits) =
                allocation_probe::observe(|| factory.visit_shared_tensors(&mut visitor));
            assert_eq!((visits.allocations, visits.reallocations), (0, 0));
            println!(
                "{} requested={} peak={} retained={} (preparation measurement, not admission)",
                $label, o.requested_bytes, o.peak_bytes, o.live_bytes
            );
            factory
        }};
    }
    let affine = test_factory!(
        "affine",
        owners::fixed_numeric_flow::FixedAffineFlowOperationFactory::for_plan(&plan, &sources)
    );
    let linear = test_factory!(
        "linear",
        owners::fixed_numeric_linear_flow::FixedLinearFlowOperationFactory::for_plan(
            &plan, &sources
        )
    );
    let embedding = test_factory!(
        "embedding",
        owners::fixed_numeric_embedding_flow::FixedEmbeddingFlowOperationFactory::for_plan(
            &plan, &sources
        )
    );
    let compact = test_factory!(
        "compact-empty-original-plan",
        owners::fixed_numeric_compact::FixedCompactOperationFactory::for_plan(&plan, &sources)
    );
    let (numeric, o) = allocation_probe::observe(|| {
        owners::fixed_numeric::FixedNumericOperationFactory::for_plan_capacity64(&plan, &sources)
            .unwrap()
    });
    let payload = numeric
        .iter()
        .try_fold(
            numeric
                .capacity()
                .checked_mul(core::mem::size_of::<
                    owners::fixed_numeric::FixedNumericOperationFactory,
                >())
                .unwrap(),
            |n, factory| n.checked_add(factory.local_owned_payload_bytes().unwrap()),
        )
        .unwrap();
    assert_eq!(o.live_bytes, payload);
    for factory in &numeric {
        factory.visit_shared_tensors(&mut visitor);
    }
    println!(
        "numeric {} owners requested={} peak={} retained={} (preparation measurement, not admission)",
        numeric.len(),
        o.requested_bytes,
        o.peak_bytes,
        o.live_bytes
    );
    assert!(shared_visits.get() >= 36);
    fn original_and_foreign_guard(
        factory: &impl conduit_composite::KernelOperationFactory,
        plan: &Plan,
    ) -> bool {
        let Some(gear) = plan.fragments[0]
            .placements
            .iter()
            .find(|gear| &gear.implementation_id == factory.implementation_id())
        else {
            return false;
        };
        factory.budget(gear).unwrap();
        let mut foreign = gear.clone();
        foreign.kind_contract_revision = "foreign/original-kind-revision".into();
        assert!(factory.budget(&foreign).is_err());
        true
    }
    assert!(original_and_foreign_guard(&affine, &plan));
    assert!(original_and_foreign_guard(&linear, &plan));
    assert!(original_and_foreign_guard(&embedding, &plan));
    assert!(!original_and_foreign_guard(&compact, &plan));
    for factory in &numeric {
        assert!(original_and_foreign_guard(factory, &plan));
    }
    // Compose the actual complete model backing and all36 admitted original
    // resource descriptors, while every factory above retains its original Arcs.
    use conduit_ai::numeric_resource_owners::*;
    let (artifact, signature, model_bytes, access, signature_frame) =
        original_model_parts(blob.clone());
    assert!(Arc::ptr_eq(&model_bytes, &blob));
    let model =
        Arc::new(AdmittedModelResource::adopt(artifact, signature, model_bytes, &access).unwrap());
    let source_text: Arc<str> = String::from_utf8(pinned(
        project.join("outputs/committed-common-fargan-greeting/checked-epoch-source.conduit"),
        "289b43e8b629053656795afae566842bbb4ae06838dfd6ddc1418102feccc733",
    ))
    .unwrap()
    .into();
    assert_eq!(
        conduit_plot::syntax_source_document_identity(&source_text),
        plan.source_document_id
    );
    let plan = Arc::new(plan);
    let tensors: Vec<_> = sources.values().cloned().collect();
    let signature_frame: Arc<[u8]> = signature_frame.into();
    let buffers = [original_plan_bytes.clone(), signature_frame.clone()];
    let (r, o) = allocation_probe::observe(|| {
        PreparedNumericResourceOwners::storage_reservation(
            &model,
            &plan,
            &source_text,
            &tensors,
            &buffers,
        )
        .unwrap()
    });
    assert_eq!((o.allocations, o.reallocations), (0, 0));
    assert_eq!(r.unique_tensor_resources(), 36);
    assert_eq!(r.unique_tensor_descriptors(), 36);
    assert_eq!(r.unique_byte_backings(), 6); // shared model +3feature +2original buffers
    let limits = NumericResourceStorageLimits {
        maximum_preparation_requested_bytes: r.preparation_requested_bytes_bound(),
        maximum_retained_payload_bytes: r.retained_payload_bytes(),
        maximum_shared_allocations: r.shared_allocations(),
    };
    for which in 0..3 {
        let mut under = limits;
        match which {
            0 => under.maximum_preparation_requested_bytes -= 1,
            1 => under.maximum_retained_payload_bytes -= 1,
            _ => under.maximum_shared_allocations -= 1,
        }
        let (result, o) = allocation_probe::observe(|| {
            PreparedNumericResourceOwners::prepare(
                &model,
                &plan,
                &source_text,
                &tensors,
                &buffers,
                under,
            )
        });
        assert!(matches!(
            result,
            Err(NumericResourceStorageRefusal::Capacity)
        ));
        assert_eq!((o.allocations, o.reallocations), (0, 0));
    }
    let (result, o) = allocation_probe::observe(|| {
        PreparedNumericResourceOwners::prepare(
            &model,
            &plan,
            &source_text,
            &tensors,
            &buffers,
            limits,
        )
    });
    let owner = result.unwrap();
    let receipt = owner.receipt();
    assert_eq!(receipt, r);
    assert_eq!(o.live_bytes, r.preparation_requested_bytes_bound());
    for (actual, original) in owner.buffers().iter().zip(&buffers) {
        assert!(Arc::ptr_eq(actual, original));
    }
    assert!(o.requested_bytes <= receipt.preparation_requested_bytes_bound());
    assert!(Arc::ptr_eq(owner.model(), &model));
    assert!(Arc::ptr_eq(owner.plan(), &plan));
    assert!(Arc::ptr_eq(owner.source(), &source_text));
    assert_eq!(owner.tensors().len(), 36);
    for (actual, original) in owner.tensors().iter().zip(&tensors) {
        assert!(Arc::ptr_eq(actual, original));
    }
    println!(
        "actual full resource aggregate newarrayrequested={} retained_payload={} sharedallocations={} tensorresources={} descriptors={} backings={}",
        o.requested_bytes,
        r.retained_payload_bytes(),
        r.shared_allocations(),
        r.unique_tensor_resources(),
        r.unique_tensor_descriptors(),
        r.unique_byte_backings()
    );
    // Original Plan and owners remain live throughout; no recurrent execution
    // or model-signature/resource correlation is claimed by this storage gate.
    assert!(affine.local_owned_payload_bytes().unwrap() > 0);
    assert!(linear.local_owned_payload_bytes().unwrap() > 0);
    assert!(embedding.local_owned_payload_bytes().unwrap() > 0);
    assert!(compact.local_owned_payload_bytes().unwrap() > 0);
}
