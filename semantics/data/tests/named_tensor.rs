use conduit_data::{
    tensor_content_digest, NamedTensor, NamedTensorBundle, NamedTensorBundleRefusal, TensorAxis,
    TensorAxisRole, TensorBacking, TensorElement, TensorRefusal, TensorValue,
};
use conduit_plot::rust_binding::{BoundedBytes, BoundedSequence, NativeRustBinding};

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

#[test]
fn mixed_tensor_bundle_round_trips_names_types_shapes_and_content() {
    let ids = [7_i64.to_le_bytes(), 11_i64.to_le_bytes()].concat();
    let values = vec![
        tensor("identifiers", TensorElement::I64, &ids),
        tensor("gain", TensorElement::F32, &0.5_f32.to_le_bytes()),
    ];
    let bundle = NamedTensorBundle::from_values(values.clone()).unwrap();
    assert_eq!(bundle.tensor_bytes(), Ok(20));
    let canonical = bundle.clone().encode().unwrap();
    assert_eq!(
        conduit_core::semantic_digest(conduit_data::NAMED_TENSOR_BUNDLE_INFO_ID, &canonical),
        bundle.semantic_digest().unwrap(),
    );
    let structured = bundle.clone().into_structured().unwrap();
    assert_eq!(
        NamedTensorBundle::from_structured(structured).unwrap(),
        bundle
    );
    let mut reversed = values.clone();
    reversed.reverse();
    assert_ne!(
        bundle.semantic_digest().unwrap(),
        NamedTensorBundle::from_values(reversed)
            .unwrap()
            .semantic_digest()
            .unwrap()
    );
    let mut renamed = values;
    renamed[0].name = "other-identifiers".into();
    assert_ne!(
        bundle.semantic_digest().unwrap(),
        NamedTensorBundle::from_values(renamed)
            .unwrap()
            .semantic_digest()
            .unwrap()
    );
    assert_eq!(
        NamedTensorBundle::from_values(vec![])
            .unwrap()
            .tensor_bytes(),
        Ok(0)
    );
}

#[test]
fn duplicates_forged_payloads_names_and_excess_count_refuse() {
    let value = tensor("x", TensorElement::F32, &1_f32.to_le_bytes());
    assert_eq!(
        NamedTensorBundle::from_values(vec![value.clone(), value.clone()]).err(),
        Some(NamedTensorBundleRefusal::DuplicateName)
    );
    let mut forged = value.clone();
    forged.name = "y".into();
    forged.value.content_digest = [0; 32];
    assert_eq!(
        NamedTensorBundle::from_values(vec![value.clone(), forged]).err(),
        Some(NamedTensorBundleRefusal::Tensor {
            index: 1,
            refusal: TensorRefusal::ContentIdentityMismatch
        })
    );
    let mut nul = value.clone();
    nul.name = "x\0y".into();
    assert_eq!(
        NamedTensorBundle::from_values(vec![nul]).err(),
        Some(NamedTensorBundleRefusal::InvalidName)
    );
    let too_many = (0..33)
        .map(|i| {
            let mut v = value.clone();
            v.name = format!("value-{i}");
            v
        })
        .collect();
    assert_eq!(
        NamedTensorBundle::from_values(too_many).err(),
        Some(NamedTensorBundleRefusal::Count)
    );
}
