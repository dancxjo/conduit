use conduit_ai::{fixed_neural::*, fixed_tensor::*, fixed_tensor_linear::*};
use conduit_core::*;
use conduit_data::*;
use conduit_plot::rust_binding::BoundedSequence;

fn resource_tensor(dimensions: &[u64], bytes: &[u8]) -> TensorValue {
    let digest = tensor_content_digest(bytes);
    TensorValue {
        element: TensorElement::F32,
        dimensions: BoundedSequence::try_from_iter(dimensions.iter().copied()).unwrap(),
        axes: BoundedSequence::try_from_iter(dimensions.iter().map(|_| TensorAxis {
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
                items: Some(dimensions.iter().product()),
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest([1; 32]),
                expires_at: None,
            },
        }),
    }
}
fn bytes(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

#[test]
fn unbiased_projection_has_no_synthetic_bias_and_exact_layout() {
    let packed = bytes(&[1., 4., 2., 5., 3., 6.]);
    let tensor = resource_tensor(&[3, 2], &packed);
    let linear =
        FixedTensorLinear::<3, 2>::prepare(&tensor, &packed, FixedMatrixOrder::InputMajor).unwrap();
    let mut output = [99.; 2];
    linear.apply(&[2., -1., 0.5], &mut output).unwrap();
    assert_eq!(output, [1.5, 6.]);
    assert_eq!(linear.tensor(), &tensor);
    assert!(matches!(
        FixedTensorLinear::<3, 2>::prepare(&tensor, &packed, FixedMatrixOrder::OutputMajor),
        Err(FixedTensorRefusal::Shape)
    ));
    let mut corrupt = packed.clone();
    corrupt[0] ^= 1;
    assert!(matches!(
        FixedTensorLinear::<3, 2>::prepare(&tensor, &corrupt, FixedMatrixOrder::InputMajor),
        Err(FixedTensorRefusal::ContentIdentity)
    ));
}
#[test]
fn unbiased_projection_refuses_nonfinite_input_weight_and_overflow_atomically() {
    let packed = bytes(&[f32::MAX, 0., 0., 1.]);
    let tensor = resource_tensor(&[2, 2], &packed);
    let linear =
        FixedTensorLinear::<2, 2>::prepare(&tensor, &packed, FixedMatrixOrder::InputMajor).unwrap();
    for (input, refusal) in [
        ([2., 1.], FixedNumericRefusal::NonfiniteOutput),
        ([f32::NAN, 1.], FixedNumericRefusal::NonfiniteInput),
    ] {
        let mut output = [9., 8.];
        assert_eq!(linear.apply(&input, &mut output), Err(refusal));
        assert_eq!(output, [9., 8.]);
    }
    let packed = bytes(&[f32::INFINITY, 0., 0., 1.]);
    let tensor = resource_tensor(&[2, 2], &packed);
    assert!(matches!(
        FixedTensorLinear::<2, 2>::prepare(&tensor, &packed, FixedMatrixOrder::InputMajor),
        Err(FixedTensorRefusal::Numeric(
            FixedNumericRefusal::NonfiniteWeight
        ))
    ));
}
