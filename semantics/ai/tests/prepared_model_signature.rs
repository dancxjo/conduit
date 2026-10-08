use conduit_ai::ModelSignature;
use conduit_data::{TensorAxisRole, TensorElement};
use conduit_plot::rust_binding::{
    NativeRustBinding, PreparedNativeFamily, PreparedNativeFamilyLimits, PreparedNativeRustBinding,
};

fn limits() -> PreparedNativeFamilyLimits {
    PreparedNativeFamilyLimits {
        maximum_types: 64,
        maximum_laws_per_type: 256,
        maximum_input_bytes: 262_144,
        maximum_retained_bytes: 128 * 1024 * 1024,
        maximum_preparation_peak_bytes: 256 * 1024 * 1024,
        maximum_conversion_requested_bytes: 1024 * 1024 * 1024,
    }
}

#[test]
fn full_signature_imported_descriptors_and_archived_categorical_signature_admit_exactly() {
    let bytes = include_bytes!("fixtures/categorical_model_signature.native.bin");
    let reference = ModelSignature::decode(bytes).unwrap();
    let mut family =
        PreparedNativeFamily::prepare(&[ModelSignature::PREPARED_DESCRIPTOR], limits()).unwrap();
    assert!(family.contains_descriptor(TensorElement::PREPARED_DESCRIPTOR));
    assert!(family.contains_descriptor(TensorAxisRole::PREPARED_DESCRIPTOR));
    assert_eq!(family.decode::<ModelSignature>(bytes).unwrap(), reference);
    assert_eq!(reference.encode().unwrap().as_slice(), bytes);
    assert!(family.decode::<TensorElement>(bytes).is_err());
    assert!(family
        .decode::<ModelSignature>(&bytes[..bytes.len() - 1])
        .is_err());
    let receipt = family.storage_receipt();
    eprintln!("full ModelSignature family: {receipt:?}");
    let mut under = limits();
    under.maximum_retained_bytes = receipt.retained_heap_bytes_bound - 1;
    assert!(PreparedNativeFamily::prepare(&[ModelSignature::PREPARED_DESCRIPTOR], under).is_err());
}

#[test]
fn full_signature_accepts_every_imported_tensor_element_and_axis_role() {
    use conduit_ai::{
        ModelAxisConstraint, ModelDimensionConstraint, ModelOperation, ModelPortConstraint,
        ModelPortPresence, ModelTensorConstraint, ModelValueConstraint,
    };
    let mut family =
        PreparedNativeFamily::prepare(&[ModelSignature::PREPARED_DESCRIPTOR], limits()).unwrap();
    let elements = [
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
    ];
    let roles = [
        TensorAxisRole::Batch,
        TensorAxisRole::Time,
        TensorAxisRole::Feature,
        TensorAxisRole::Sensor,
        TensorAxisRole::SpatialCoordinate,
        TensorAxisRole::Frequency,
        TensorAxisRole::Channel,
        TensorAxisRole::other("axis".into()).unwrap(),
    ];
    for element in elements {
        for role in &roles {
            let tensor = ModelTensorConstraint::from_parts(
                vec![element],
                vec![ModelAxisConstraint::new(
                    ModelDimensionConstraint::fixed(1).unwrap(),
                    role.clone(),
                )
                .unwrap()],
                4096,
            )
            .unwrap();
            let port = ModelPortConstraint::from_parts(
                "input".into(),
                "tensor@1".into(),
                ModelPortPresence::Required,
                ModelValueConstraint::tensor(tensor).unwrap(),
            )
            .unwrap();
            let value = ModelSignature::from_parts(
                "model/prepared-test@1".into(),
                1,
                vec![ModelOperation::Infer],
                vec![port.clone()],
                vec![port],
            )
            .unwrap();
            let bytes = value.encode().unwrap();
            assert_eq!(
                family.decode::<ModelSignature>(&bytes).unwrap(),
                ModelSignature::decode(&bytes).unwrap()
            );
        }
    }
}
