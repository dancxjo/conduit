use conduit_ai::{fixed_neural::*, fixed_tensor::*};
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
fn input_major_and_output_major_share_one_exact_affine_meaning() {
    let column_bytes = bytes(&[1.0, 4.0, 2.0, 5.0, 3.0, 6.0]);
    let row_bytes = bytes(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    let bias_bytes = bytes(&[0.5, -0.5]);
    let columns = resource_tensor(&[3, 2], &column_bytes);
    let rows = resource_tensor(&[2, 3], &row_bytes);
    let bias = resource_tensor(&[2], &bias_bytes);
    let column = FixedTensorAffine::<3, 2>::prepare(
        &columns,
        &column_bytes,
        &bias,
        &bias_bytes,
        FixedMatrixOrder::InputMajor,
    )
    .unwrap();
    let row = FixedTensorAffine::<3, 2>::prepare(
        &rows,
        &row_bytes,
        &bias,
        &bias_bytes,
        FixedMatrixOrder::OutputMajor,
    )
    .unwrap();
    let mut column_output = [0.0; 2];
    let mut row_output = [0.0; 2];
    column.apply(&[2.0, -1.0, 0.5], &mut column_output).unwrap();
    row.apply(&[2.0, -1.0, 0.5], &mut row_output).unwrap();
    assert_eq!(column_output, [2.0, 5.5]);
    assert_eq!(column_output, row_output);
    assert_eq!(column.weights(), &columns);
    assert!(matches!(
        FixedTensorAffine::<3, 2>::prepare(
            &columns,
            &column_bytes,
            &bias,
            &bias_bytes,
            FixedMatrixOrder::OutputMajor
        ),
        Err(FixedTensorRefusal::Shape)
    ));
}

#[test]
fn corrupted_truncated_or_nonfinite_resources_refuse_before_execution() {
    let payload = bytes(&[1.0, 2.0]);
    let tensor = resource_tensor(&[1, 2], &payload);
    let bias_bytes = bytes(&[0.0]);
    let bias = resource_tensor(&[1], &bias_bytes);
    let mut corrupt = payload.clone();
    corrupt[0] ^= 1;
    for input in [&corrupt[..], &payload[..4]] {
        assert!(matches!(
            FixedTensorAffine::<2, 1>::prepare(
                &tensor,
                input,
                &bias,
                &bias_bytes,
                FixedMatrixOrder::OutputMajor
            ),
            Err(FixedTensorRefusal::ContentIdentity)
        ));
    }
    let invalid = bytes(&[f32::NAN, 0.0]);
    let invalid_tensor = resource_tensor(&[1, 2], &invalid);
    assert!(matches!(
        FixedTensorAffine::<2, 1>::prepare(
            &invalid_tensor,
            &invalid,
            &bias,
            &bias_bytes,
            FixedMatrixOrder::OutputMajor
        ),
        Err(FixedTensorRefusal::Numeric(
            FixedNumericRefusal::NonfiniteWeight
        ))
    ));
}
