//! Optional presence does not weaken shared owned/borrowed numeric contracts.
use conduit_ai::*;
use conduit_core::{
    BoundedResourceRef, ResourceExtent, ResourceLifetime, ResourceSemanticIdentity,
    ResourceVersionIdentity,
};
use conduit_data::*;
use conduit_plot::rust_binding::{BoundedBytes, BoundedSequence, NativeRustBinding};

fn signature(element: TensorElement, policy: ModelTensorNumericPolicy) -> ModelSignature {
    let constraint =
        ModelTensorConstraint::from_parts(vec![element], vec![], element.byte_width(), policy)
            .unwrap();
    let port = |name: &str, presence| {
        ModelPortConstraint::from_parts(
            name.into(),
            "data/tensor@1".into(),
            presence,
            ModelValueConstraint::tensor(constraint.clone()).unwrap(),
        )
        .unwrap()
    };
    ModelSignature::from_parts(
        "test/optional-numeric@1".into(),
        1,
        vec![ModelOperation::Infer],
        vec![port("hint", ModelPortPresence::Optional)],
        vec![port("result", ModelPortPresence::Required)],
    )
    .unwrap()
}
fn bundle(element: TensorElement, bytes: &[u8], resource: bool) -> NamedTensorBundle {
    let digest = tensor_content_digest(bytes);
    let backing = if resource {
        TensorBacking::Resource(BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest(digest),
            content_profile: match element {
                TensorElement::F32 => "data/tensor-f32-le@1",
                TensorElement::F64 => "data/tensor-f64-le@1",
                _ => unreachable!(),
            }
            .into(),
            access_class: "test/unopened-optional".into(),
            extent: ResourceExtent {
                bytes: element.byte_width(),
                items: Some(1),
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest([9; 32]),
                expires_at: None,
            },
        })
    } else {
        TensorBacking::Inline(BoundedBytes::new(bytes).unwrap())
    };
    let mut tensor = TensorValue {
        element,
        dimensions: BoundedSequence::new(),
        axes: BoundedSequence::new(),
        content_digest: digest,
        backing,
    };
    if resource {
        let profile = tensor.resource_profile().to_owned();
        if let TensorBacking::Resource(reference) = &mut tensor.backing {
            reference.content_profile = profile.into();
        }
    }
    NamedTensorBundle::from_values(vec![NamedTensor {
        name: "hint".into(),
        value: tensor,
    }])
    .unwrap()
}
#[test]
fn optional_f32_f64_owned_and_borrowed_presence_retains_numeric_laws() {
    for element in [TensorElement::F32, TensorElement::F64] {
        let patterns: Vec<Vec<u8>> = if element == TensorElement::F32 {
            [0_u32, 1, 0x7fc0_0001, 0x7f80_0000, 0xff80_0000]
                .into_iter()
                .map(|v| v.to_le_bytes().to_vec())
                .collect()
        } else {
            [
                0_u64,
                1,
                0x7ff8_0000_0000_0001,
                0x7ff0_0000_0000_0000,
                0xfff0_0000_0000_0000,
            ]
            .into_iter()
            .map(|v| v.to_le_bytes().to_vec())
            .collect()
        };
        for policy in [
            ModelTensorNumericPolicy::Ieee,
            ModelTensorNumericPolicy::FiniteOnly,
        ] {
            let signature = signature(element, policy);
            let prepared = PreparedModelTensorIo::prepare(&signature).unwrap();
            let reader = PreparedNamedTensorBundleView::prepare().unwrap();
            let empty = NamedTensorBundle::from_values(vec![]).unwrap();
            assert_eq!(signature.validate_named_inputs(&empty), Ok(()));
            let encoded = empty.encode().unwrap();
            assert_eq!(
                prepared.validate_inputs(&reader.read(&encoded).unwrap()),
                Ok(())
            );
            for (index, bytes) in patterns.iter().enumerate() {
                for resource in [false, true] {
                    let value = bundle(element, bytes, resource);
                    let expected = if policy == ModelTensorNumericPolicy::Ieee {
                        Ok(())
                    } else if resource {
                        Err(ModelTensorIoRefusal::NumericPolicyUnverifiable)
                    } else if index >= 2 {
                        Err(ModelTensorIoRefusal::NonFinite)
                    } else {
                        Ok(())
                    };
                    assert_eq!(signature.validate_named_inputs(&value), expected);
                    let encoded = value.encode().unwrap();
                    let original = encoded.clone();
                    assert_eq!(
                        prepared.validate_inputs(&reader.read(&encoded).unwrap()),
                        expected
                    );
                    assert_eq!(encoded, original);
                }
            }
        }
    }
}
