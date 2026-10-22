//! Rank zero is one exact element, never an empty or rank-one tensor.
use conduit_data::*;
use conduit_plot::rust_binding::{BoundedBytes, BoundedSequence, NativeRustBinding};
fn scalar(element: TensorElement, payload: &[u8]) -> TensorValue {
    TensorValue {
        element,
        dimensions: BoundedSequence::new(),
        axes: BoundedSequence::new(),
        content_digest: tensor_content_digest(payload),
        backing: TensorBacking::Inline(BoundedBytes::new(payload).unwrap()),
    }
}
#[test]
fn scalar_integer_and_float_round_trip_raw_native_and_borrowed_without_coercion() {
    for tensor in [
        scalar(TensorElement::I64, &(-7_i64).to_le_bytes()),
        scalar(TensorElement::F32, &1.5_f32.to_le_bytes()),
        scalar(TensorElement::F64, &2.5_f64.to_le_bytes()),
    ] {
        assert_eq!(tensor.element_count(), Ok(1));
        assert_eq!(tensor.byte_count(), Ok(tensor.element.byte_width()));
        assert_eq!(tensor.summary().unwrap().elements, 1);
        let encoded = TensorValue::encode(&tensor).unwrap();
        assert_eq!(encoded[2], 0);
        assert_eq!(TensorValue::decode(&encoded).unwrap(), tensor);
        let bundle = NamedTensorBundle::from_values(vec![NamedTensor {
            name: "scalar".into(),
            value: tensor.clone(),
        }])
        .unwrap();
        let encoded = bundle.clone().encode().unwrap();
        assert_eq!(NamedTensorBundle::decode(&encoded).unwrap(), bundle);
        let reader = PreparedNamedTensorBundleView::prepare().unwrap();
        let view = reader.read(&encoded).unwrap();
        let value = view.values().next().unwrap();
        assert!(value.dimensions().is_empty());
        assert_eq!(value.byte_count(), tensor.element.byte_width());
        let output = PreparedNamedTensorBundleOutput::prepare(&bundle).unwrap();
        assert!(output.maximum_encoded_len() >= encoded.len());
        let mut rank_one = tensor.clone();
        rank_one.dimensions = BoundedSequence::try_from_iter([1]).unwrap();
        rank_one.axes = BoundedSequence::try_from_iter([TensorAxis {
            role: TensorAxisRole::Feature,
            identity: None,
            unit: None,
        }])
        .unwrap();
        rank_one.validate().unwrap();
        assert_ne!(rank_one.semantic_digest(), tensor.semantic_digest());
        assert_eq!(
            TensorValue::decode(&TensorValue::encode(&rank_one).unwrap()),
            Ok(rank_one)
        );
    }
}
#[test]
fn scalar_storage_has_exactly_one_dtype_width_and_zero_axes() {
    for (element, width) in [(TensorElement::I64, 8), (TensorElement::F32, 4)] {
        for bytes in [0, width - 1, width + 1, 2 * width] {
            let value = scalar(element, &vec![0; bytes]);
            assert_eq!(value.validate(), Err(TensorRefusal::PayloadLengthMismatch));
            // The raw decoder rechecks the same shape/storage invariant.
            let mut wire = TensorValue::encode(&scalar(element, &vec![0; width])).unwrap();
            wire[36..40].copy_from_slice(&(bytes as u32).to_le_bytes());
            wire.truncate(40);
            wire.extend_from_slice(&vec![0; bytes]);
            assert_eq!(
                TensorValue::decode(&wire),
                Err(TensorRefusal::PayloadLengthMismatch)
            );
        }
    }
    let mut value = scalar(TensorElement::F32, &[0; 4]);
    value.axes = BoundedSequence::try_from_iter([TensorAxis {
        role: TensorAxisRole::Feature,
        identity: None,
        unit: None,
    }])
    .unwrap();
    assert_eq!(value.validate(), Err(TensorRefusal::AxisCountMismatch));
    value.dimensions = BoundedSequence::try_from_iter([0]).unwrap();
    assert_eq!(value.validate(), Err(TensorRefusal::ZeroDimension));
}

#[test]
fn scalar_resource_extent_declares_exactly_one_element() {
    use conduit_core::{
        BoundedResourceRef, ResourceExtent, ResourceLifetime, ResourceSemanticIdentity,
        ResourceVersionIdentity,
    };
    let mut value = scalar(TensorElement::I64, &5_i64.to_le_bytes());
    let reference = BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest(value.content_digest),
        content_profile: value.resource_profile().into(),
        access_class: "test/scalar-read".into(),
        extent: ResourceExtent {
            bytes: 8,
            items: Some(1),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([7; 32]),
            expires_at: None,
        },
    };
    value.backing = TensorBacking::Resource(reference.clone());
    value.validate().unwrap();
    assert_eq!(
        TensorValue::decode(&TensorValue::encode(&value).unwrap()),
        Ok(value.clone())
    );
    for items in [None, Some(0), Some(2)] {
        let mut bad = reference.clone();
        bad.extent.items = items;
        value.backing = TensorBacking::Resource(bad);
        assert_eq!(value.validate(), Err(TensorRefusal::ResourceExtentMismatch));
    }
}
