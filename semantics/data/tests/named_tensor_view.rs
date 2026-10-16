//! Borrowed tensor parity, Source custody and bounded read storage.
use conduit_data::*;
use conduit_plot::rust_binding::{BoundedBytes, BoundedSequence, NativeRustBinding};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
thread_local! { static ENABLED: Cell<bool> = const { Cell::new(false) }; static CALLS: Cell<usize> = const { Cell::new(0) }; }
struct Counted;
fn record() {
    let _ = ENABLED.try_with(|v| {
        if v.get() {
            let _ = CALLS.try_with(|v| v.set(v.get() + 1));
        }
    });
}
unsafe impl GlobalAlloc for Counted {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        record();
        unsafe { System.alloc(l) }
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        record();
        unsafe { System.alloc_zeroed(l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        record();
        unsafe { System.realloc(p, l, n) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
}
#[global_allocator]
static ALLOCATOR: Counted = Counted;
fn tensor(name: &str, element: TensorElement, bytes: &[u8]) -> NamedTensor {
    NamedTensor {
        name: name.into(),
        value: TensorValue {
            element,
            dimensions: BoundedSequence::try_from_iter([bytes.len() as u64 / element.byte_width()])
                .unwrap(),
            axes: BoundedSequence::try_from_iter([TensorAxis {
                role: TensorAxisRole::Feature,
                identity: None,
                unit: None,
            }])
            .unwrap(),
            content_digest: tensor_content_digest(bytes),
            backing: TensorBacking::Inline(BoundedBytes::new(bytes).unwrap()),
        },
    }
}
fn canonical(values: Vec<NamedTensor>) -> Vec<u8> {
    // Native construction checks Source laws, not TensorValue's additional
    // content/shape laws. Invalid business data must reach the read boundary.
    NamedTensorBundle::new(BoundedSequence::try_from_iter(values).unwrap())
        .unwrap()
        .encode()
        .unwrap()
}
fn measured<'a>(
    prepared: &PreparedNamedTensorBundleView,
    bytes: &'a [u8],
) -> Result<NamedTensorBundleView<'a>, NamedTensorViewRefusal> {
    CALLS.with(|v| v.set(0));
    ENABLED.with(|v| v.set(true));
    let result = prepared.read(bytes);
    ENABLED.with(|v| v.set(false));
    assert_eq!(
        CALLS.with(Cell::get),
        0,
        "borrowed admission must not allocate"
    );
    result
}
#[test]
fn mixed_inline_bundle_preserves_names_dimensions_payload_axes_and_digest_without_allocation() {
    let prepared = PreparedNamedTensorBundleView::prepare().unwrap();
    let values = vec![
        tensor(
            "ids",
            TensorElement::I64,
            &[7_i64.to_le_bytes(), 11_i64.to_le_bytes()].concat(),
        ),
        tensor("gain", TensorElement::F32, &0.5_f32.to_le_bytes()),
    ];
    let owned = NamedTensorBundle::from_values(values.clone()).unwrap();
    let bytes = canonical(values);
    let borrowed = measured(&prepared, &bytes).unwrap();
    assert_eq!(borrowed.tensor_bytes(), owned.tensor_bytes().unwrap());
    assert_eq!(borrowed.semantic_digest(), owned.semantic_digest().unwrap());
    for (actual, expected) in borrowed.values().zip(owned.get().iter()) {
        assert_eq!(actual.name, expected.name);
        assert_eq!(actual.element, expected.value.element);
        assert_eq!(actual.dimensions(), expected.value.dimensions.as_slice());
        assert_eq!(actual.content_digest, expected.value.content_digest);
        assert_eq!(actual.axes().collection_length().unwrap(), 1);
        let TensorBacking::Inline(payload) = &expected.value.backing else {
            panic!("inline")
        };
        let TensorBackingView::Inline(actual_payload) = actual.backing else {
            panic!("inline")
        };
        assert_eq!(actual_payload, payload.as_slice());
        assert!(actual_payload.as_ptr() >= bytes.as_ptr());
        assert!(actual_payload.as_ptr_range().end <= bytes.as_ptr_range().end);
    }
    assert_eq!(
        measured(&prepared, &canonical(vec![]))
            .unwrap()
            .values()
            .len(),
        0
    );
}
#[test]
fn inline_profile_refuses_duplicate_names_bad_shapes_and_forged_content_without_allocation() {
    let prepared = PreparedNamedTensorBundleView::prepare().unwrap();
    let valid = tensor("x", TensorElement::F32, &1_f32.to_le_bytes());
    assert_eq!(
        measured(&prepared, &canonical(vec![valid.clone(), valid.clone()])).err(),
        Some(NamedTensorViewRefusal::DuplicateName)
    );
    let mut nul = valid.clone();
    nul.name = "x\0y".into();
    assert_eq!(
        measured(&prepared, &canonical(vec![nul])).err(),
        Some(NamedTensorViewRefusal::Name)
    );
    for (mut invalid, refusal) in [
        (valid.clone(), TensorRefusal::ZeroDimension),
        (valid.clone(), TensorRefusal::PayloadLengthMismatch),
        (valid.clone(), TensorRefusal::ContentIdentityMismatch),
        (valid.clone(), TensorRefusal::AxisCountMismatch),
    ] {
        match refusal {
            TensorRefusal::ZeroDimension => {
                invalid.value.dimensions = BoundedSequence::try_from_iter([0]).unwrap()
            }
            TensorRefusal::PayloadLengthMismatch => {
                invalid.value.dimensions = BoundedSequence::try_from_iter([2]).unwrap()
            }
            TensorRefusal::ContentIdentityMismatch => invalid.value.content_digest = [0; 32],
            TensorRefusal::AxisCountMismatch => {
                invalid.value.dimensions = BoundedSequence::try_from_iter([1, 1]).unwrap()
            }
            _ => unreachable!(),
        }
        assert_eq!(invalid.value.validate(), Err(refusal));
        assert_eq!(
            measured(&prepared, &canonical(vec![invalid])).err(),
            Some(NamedTensorViewRefusal::Tensor(refusal))
        );
    }
    let foreign = valid.encode().unwrap();
    assert_eq!(
        measured(&prepared, &foreign).err(),
        Some(NamedTensorViewRefusal::Encoding)
    );
    let mut trailing = canonical(vec![tensor("x", TensorElement::U8, &[1])]);
    trailing.push(0);
    assert_eq!(
        measured(&prepared, &trailing).err(),
        Some(NamedTensorViewRefusal::Encoding)
    );
}

fn resource_tensor() -> NamedTensor {
    use conduit_core::{
        kind_id, BoundedResourceRef, ResourceClassId, ResourceExtent, ResourceLifetime,
        ResourceSemanticIdentity, ResourceVersionIdentity,
    };
    let mut named = tensor("external", TensorElement::F32, &1_f32.to_le_bytes());
    named.value.backing = TensorBacking::Resource(BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest(named.value.content_digest),
        content_profile: kind_id(named.value.resource_profile()),
        access_class: ResourceClassId::from("content/read@1"),
        extent: ResourceExtent {
            bytes: 4,
            items: Some(1),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([2; 32]),
            expires_at: None,
        },
    });
    named
}
#[test]
fn resource_descriptor_preserves_custody_and_refuses_profile_extent_and_digest_without_allocating()
{
    let prepared = PreparedNamedTensorBundleView::prepare().unwrap();
    let named = resource_tensor();
    named.value.validate().unwrap();
    let bytes = canonical(vec![named.clone()]);
    let borrowed = measured(&prepared, &bytes).unwrap();
    assert_eq!(borrowed.tensor_bytes(), 4);
    let value = borrowed.values().next().unwrap();
    let TensorBackingView::Resource { reference, encoded } = value.backing else {
        panic!("resource")
    };
    let TensorBacking::Resource(expected) = &named.value.backing else {
        panic!("resource")
    };
    assert_eq!(encoded, expected.encode().unwrap());
    assert_eq!(reference.identity, expected.identity);
    assert_eq!(reference.version, expected.lifetime.version);
    assert_eq!(reference.access_class, expected.access_class.as_str());
    assert_eq!(reference.content_profile, expected.content_profile.as_str());
    assert_eq!(reference.extent, expected.extent);
    let output_template = NamedTensorBundle::from_values(vec![named.clone()]).unwrap();
    assert_eq!(
        PreparedNamedTensorBundleOutput::prepare(&output_template).err(),
        Some(NamedTensorOutputRefusal::UnsupportedBacking),
    );
    // No opener, Host, path, or authority is involved in descriptor admission.
    for refusal in [
        TensorRefusal::ResourceProfileMismatch,
        TensorRefusal::ResourceExtentMismatch,
        TensorRefusal::ContentIdentityMismatch,
    ] {
        let mut invalid = named.clone();
        let TensorBacking::Resource(reference) = &mut invalid.value.backing else {
            unreachable!()
        };
        match refusal {
            TensorRefusal::ResourceProfileMismatch => {
                reference.content_profile = conduit_core::kind_id("tensor/elements-i64-le@1")
            }
            TensorRefusal::ResourceExtentMismatch => reference.extent.items = Some(2),
            TensorRefusal::ContentIdentityMismatch => {
                reference.identity = conduit_core::ResourceSemanticIdentity::from_digest([3; 32])
            }
            _ => unreachable!(),
        }
        assert_eq!(invalid.value.validate(), Err(refusal));
        assert_eq!(
            measured(&prepared, &canonical(vec![invalid])).err(),
            Some(NamedTensorViewRefusal::Tensor(refusal))
        );
    }
}
#[test]
fn every_dtype_and_full_named_capacity_use_the_same_borrowed_boundary() {
    let prepared = PreparedNamedTensorBundleView::prepare().unwrap();
    for element in [
        TensorElement::I8,
        TensorElement::U8,
        TensorElement::I16,
        TensorElement::I24,
        TensorElement::U16,
        TensorElement::I32,
        TensorElement::U32,
        TensorElement::I64,
        TensorElement::U64,
        TensorElement::F32,
        TensorElement::F64,
    ] {
        let payload = vec![0; element.byte_width() as usize];
        let bytes = canonical(vec![tensor("value", element, &payload)]);
        let view = measured(&prepared, &bytes).unwrap();
        let value = view.values().next().unwrap();
        assert_eq!(value.element, element);
        assert_eq!(value.byte_count(), element.byte_width());
    }
    let values = (0..MAXIMUM_NAMED_TENSORS)
        .map(|i| tensor(&format!("port-{i}"), TensorElement::U8, &[1]))
        .collect();
    let bytes = canonical(values);
    let view = measured(&prepared, &bytes).unwrap();
    assert_eq!(view.values().len(), 32);
    assert_eq!(view.tensor_bytes(), 32);
}

fn without_allocations(f: impl FnOnce()) {
    CALLS.with(|v| v.set(0));
    ENABLED.with(|v| v.set(true));
    f();
    ENABLED.with(|v| v.set(false));
    assert_eq!(
        CALLS.with(Cell::get),
        0,
        "prepared output must not allocate"
    );
}

#[test]
fn prepared_output_reuses_exact_native_geometry_and_matches_owned_encoding() {
    let template = NamedTensorBundle::from_values(vec![
        tensor("score", TensorElement::F32, &0_f32.to_le_bytes()),
        tensor("ids", TensorElement::I64, &0_i64.to_le_bytes()),
    ])
    .unwrap();
    let expected = NamedTensorBundle::from_values(vec![
        tensor("score", TensorElement::F32, &0.5_f32.to_le_bytes()),
        tensor("ids", TensorElement::I64, &11_i64.to_le_bytes()),
    ])
    .unwrap()
    .encode()
    .unwrap();
    let mut output = PreparedNamedTensorBundleOutput::prepare(&template).unwrap();
    let capacity = output.maximum_encoded_len();
    without_allocations(|| {
        assert_eq!(output.encoded(), Err(NamedTensorOutputRefusal::Incomplete));
        for _ in 0..100 {
            output.reset();
            output.write("ids", &11_i64.to_le_bytes()).unwrap();
            assert_eq!(output.encoded(), Err(NamedTensorOutputRefusal::Incomplete));
            output.write("score", &0.5_f32.to_le_bytes()).unwrap();
            assert_eq!(output.maximum_encoded_len(), capacity);
            assert_eq!(output.encoded().unwrap(), expected);
            let view = output.view().unwrap();
            assert_eq!(view.tensor_bytes(), 12);
            assert_eq!(view.values().len(), 2);
        }
    });
}

#[test]
fn output_errors_and_reset_prevent_publishing_stale_or_mixed_invocations() {
    let template = NamedTensorBundle::from_values(vec![
        tensor("left", TensorElement::U8, &[0]),
        tensor("right", TensorElement::U8, &[0]),
    ])
    .unwrap();
    let mut output = PreparedNamedTensorBundleOutput::prepare(&template).unwrap();
    without_allocations(|| {
        output.write("left", &[1]).unwrap();
        assert_eq!(
            output.write("right", &[2, 3]),
            Err(NamedTensorOutputRefusal::PayloadLength)
        );
        output.write("right", &[2]).unwrap();
        assert_eq!(output.encoded(), Err(NamedTensorOutputRefusal::Incomplete));
        output.write("left", &[1]).unwrap();
        assert!(output.encoded().is_ok());
        assert_eq!(
            output.write("foreign", &[3]),
            Err(NamedTensorOutputRefusal::UnknownPort)
        );
        assert_eq!(output.encoded(), Err(NamedTensorOutputRefusal::Incomplete));
        output.write("left", &[4]).unwrap();
        assert_eq!(
            output.write("left", &[5]),
            Err(NamedTensorOutputRefusal::AlreadyWritten)
        );
        output.write("right", &[6]).unwrap();
        assert_eq!(output.encoded(), Err(NamedTensorOutputRefusal::Incomplete));
        output.write("left", &[7]).unwrap();
        let view = output.view().unwrap();
        for tensor in view.values() {
            let TensorBackingView::Inline(bytes) = tensor.backing else {
                panic!("inline")
            };
            assert_eq!(bytes, if tensor.name == "left" { &[7] } else { &[6] });
        }
        output.reset();
        assert_eq!(output.encoded(), Err(NamedTensorOutputRefusal::Incomplete));
    });
}

#[test]
fn output_mask_supports_all_32_ports_and_all_admitted_dtypes() {
    let elements = [
        TensorElement::I8,
        TensorElement::U8,
        TensorElement::I16,
        TensorElement::U16,
        TensorElement::I24,
        TensorElement::I32,
        TensorElement::U32,
        TensorElement::I64,
        TensorElement::U64,
        TensorElement::F32,
        TensorElement::F64,
    ];
    let names = (0..32).map(|i| format!("port-{i}")).collect::<Vec<_>>();
    let payloads = (0..32)
        .map(|i| vec![i as u8; elements[i % elements.len()].byte_width() as usize])
        .collect::<Vec<_>>();
    let template = NamedTensorBundle::from_values(
        (0..32)
            .map(|i| {
                tensor(
                    &names[i],
                    elements[i % elements.len()],
                    &vec![0; payloads[i].len()],
                )
            })
            .collect(),
    )
    .unwrap();
    let expected = NamedTensorBundle::from_values(
        (0..32)
            .map(|i| tensor(&names[i], elements[i % elements.len()], &payloads[i]))
            .collect(),
    )
    .unwrap()
    .encode()
    .unwrap();
    let mut output = PreparedNamedTensorBundleOutput::prepare(&template).unwrap();
    without_allocations(|| {
        for i in (0..32).rev() {
            output.write(&names[i], &payloads[i]).unwrap();
        }
        assert_eq!(output.encoded().unwrap(), expected);
        assert_eq!(output.view().unwrap().values().len(), 32);
        output.reset();
        output.write(&names[31], &payloads[31]).unwrap();
        assert_eq!(output.encoded(), Err(NamedTensorOutputRefusal::Incomplete));
    });
}

#[test]
fn provider_tensor_writes_preserve_full_metadata_and_refuse_forged_digest() {
    let named = tensor("score", TensorElement::F32, &0.5_f32.to_le_bytes());
    let template = NamedTensorBundle::from_values(vec![named.clone()]).unwrap();
    let mut output = PreparedNamedTensorBundleOutput::prepare(&template).unwrap();
    let mut changed = named.value.clone();
    changed.axes = BoundedSequence::try_from_iter([TensorAxis {
        role: TensorAxisRole::Batch,
        identity: None,
        unit: None,
    }])
    .unwrap();
    let mut forged = named.value.clone();
    forged.content_digest[0] ^= 1;
    without_allocations(|| {
        output.write_tensor("score", &named.value).unwrap();
        assert!(output.encoded().is_ok());
        output.reset();
        assert_eq!(
            output.write_tensor("score", &changed),
            Err(NamedTensorOutputRefusal::MetadataMismatch)
        );
        assert_eq!(output.encoded(), Err(NamedTensorOutputRefusal::Incomplete));
        assert_eq!(
            output.write_tensor("score", &forged),
            Err(NamedTensorOutputRefusal::InvalidTensor)
        );
        assert_eq!(output.encoded(), Err(NamedTensorOutputRefusal::Incomplete));
        output.write_tensor("score", &named.value).unwrap();
        assert!(output.view().is_ok());
    });
}

#[test]
fn borrowed_output_keeps_identity_units_digest_and_failure_mask_without_allocation() {
    let named = tensor("score", TensorElement::F32, &0.5_f32.to_le_bytes());
    let template = NamedTensorBundle::from_values(vec![named.clone()]).unwrap();
    let encoded = template.clone().encode().unwrap();
    let reader = PreparedNamedTensorBundleView::prepare().unwrap();
    let good = reader.read(&encoded).unwrap();
    let mut drift = named;
    drift.value.axes[0].identity = Some("changed".into());
    drift.value.axes[0].unit = Some(conduit_core::Unit::Moment);
    let drift = NamedTensorBundle::from_values(vec![drift])
        .unwrap()
        .encode()
        .unwrap();
    let drift = reader.read(&drift).unwrap();
    let mut output = PreparedNamedTensorBundleOutput::prepare(&template).unwrap();
    without_allocations(|| {
        output.write_view(good.values().next().unwrap()).unwrap();
        assert_eq!(output.encoded().unwrap(), encoded);
        assert_eq!(
            output.write_view(drift.values().next().unwrap()),
            Err(NamedTensorOutputRefusal::MetadataMismatch)
        );
        assert_eq!(output.encoded(), Err(NamedTensorOutputRefusal::Incomplete));
        let mut forged = *good.values().next().unwrap();
        forged.content_digest[0] ^= 1;
        assert_eq!(
            output.write_view(&forged),
            Err(NamedTensorOutputRefusal::InvalidTensor)
        );
        assert_eq!(output.encoded(), Err(NamedTensorOutputRefusal::Incomplete));
        output.write_view(good.values().next().unwrap()).unwrap();
        assert_eq!(output.encoded().unwrap(), encoded);
    });
}
