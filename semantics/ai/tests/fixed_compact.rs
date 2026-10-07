use conduit_ai::fixed_compact::*;
use conduit_core::*;
use conduit_data::*;
use conduit_plot::rust_binding::BoundedSequence;
fn tensor(element: TensorElement, shape: &[u64], bytes: &[u8]) -> TensorValue {
    let digest = tensor_content_digest(bytes);
    let profile = match element {
        TensorElement::I8 => "tensor/elements-i8@1",
        TensorElement::F32 => "tensor/elements-ieee754-f32-le@1",
        _ => panic!("fixture"),
    };
    TensorValue {
        element,
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
            content_profile: kind_id(profile),
            access_class: "test/read@1".into(),
            extent: ResourceExtent {
                bytes: bytes.len() as u64,
                items: Some(shape.iter().product()),
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest([3; 32]),
                expires_at: None,
            },
        }),
    }
}
fn packed(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|x| x.to_le_bytes()).collect()
}
#[test]
fn signed_q7_admits_unit_domain_and_preserves_output_on_refusal() {
    let mut result = [11; 8];
    fixed_signed_q7(
        &[-1., -0.5, -0.5 / 127., 0., 0.5 / 127., 0.5, 1., -0.0],
        &mut result,
    )
    .unwrap();
    assert_eq!(result, [-127, -63, 0, 0, 1, 64, 127, 0]);
    let saved = result;
    assert_eq!(
        fixed_signed_q7(&[0., 0., 0., 0., 0., 0., 0., 1.001], &mut result),
        Err(CompactTensorRefusal::InputDomain)
    );
    assert_eq!(result, saved);
    assert_eq!(
        fixed_signed_q7(&[f32::NAN; 8], &mut result),
        Err(CompactTensorRefusal::Nonfinite)
    );
    assert_eq!(result, saved);
}
#[test]
fn tiled_resource_linear_matches_independent_logical_matrix_reference() {
    const INPUT: usize = 12;
    const OUTPUT: usize = 16;
    let logical: Vec<i8> = (0..INPUT * OUTPUT)
        .map(|i| ((i * 7 % 251) as i16 - 125) as i8)
        .collect();
    // Independent traversal writes each tile; reference does not reuse kernel offsets.
    let mut tile = Vec::new();
    for outputs in (0..OUTPUT).step_by(8) {
        for inputs in (0..INPUT).step_by(4) {
            for o in outputs..outputs + 8 {
                for i in inputs..inputs + 4 {
                    tile.push(logical[i * OUTPUT + o] as u8);
                }
            }
        }
    }
    let scale: Vec<f32> = (0..OUTPUT).map(|i| 0.00001 * (i + 1) as f32).collect();
    let scales = packed(&scale);
    let bias: Vec<f32> = (0..OUTPUT).map(|i| i as f32 * 0.02 - 0.1).collect();
    let biases = packed(&bias);
    let weights = tensor(TensorElement::I8, &[INPUT as u64, OUTPUT as u64], &tile);
    let scale_tensor = tensor(TensorElement::F32, &[OUTPUT as u64], &scales);
    let bias_tensor = tensor(TensorElement::F32, &[OUTPUT as u64], &biases);
    let kernel = FixedCompactTensorLinear::<INPUT, OUTPUT>::prepare(
        &weights,
        &tile,
        &scale_tensor,
        &scales,
        &bias_tensor,
        &biases,
        CompactMatrixPacking::Input4Output8Tiles,
    )
    .unwrap();
    assert_eq!(kernel.resources(), [&weights, &scale_tensor, &bias_tensor]);
    let input = core::array::from_fn(|i| (i as i16 * 19 - 100) as i8);
    let mut actual = [0.; OUTPUT];
    kernel.evaluate(&input, &mut actual).unwrap();
    for o in 0..OUTPUT {
        let sum: i64 = (0..INPUT)
            .map(|i| i64::from(input[i]) * i64::from(logical[i * OUTPUT + o]))
            .sum();
        let expected = sum as f64 * f64::from(scale[o]) + f64::from(bias[o]);
        assert!((f64::from(actual[o]) - expected).abs() < 2e-6);
    }
    let old = actual;
    let mut invalid = input;
    invalid[0] = i8::MIN;
    assert_eq!(
        kernel.evaluate(&invalid, &mut actual),
        Err(CompactTensorRefusal::InputDomain)
    );
    assert_eq!(actual, old);
}
#[test]
fn compact_tensor_refuses_shape_content_scale_and_nonfinite_result_atomically() {
    let bytes = vec![127; 32];
    let weights = tensor(TensorElement::I8, &[4, 8], &bytes);
    let scale = packed(&[f32::MAX; 8]);
    let scales = tensor(TensorElement::F32, &[8], &scale);
    let bias = packed(&[0.; 8]);
    let biases = tensor(TensorElement::F32, &[8], &bias);
    let prepare = |w: &TensorValue, s: &TensorValue, b: &TensorValue| {
        FixedCompactTensorLinear::<4, 8>::prepare(
            w,
            &bytes,
            s,
            &scale,
            b,
            &bias,
            CompactMatrixPacking::Input4Output8Tiles,
        )
        .map(|_| ())
    };
    let mut corrupt = weights.clone();
    corrupt.content_digest = [0; 32];
    assert!(prepare(&corrupt, &scales, &biases).is_err());
    let wrong = tensor(TensorElement::I8, &[8, 4], &bytes);
    assert_eq!(
        prepare(&wrong, &scales, &biases),
        Err(CompactTensorRefusal::Shape)
    );
    let negative = packed(&[-0.1; 8]);
    let negative_tensor = tensor(TensorElement::F32, &[8], &negative);
    assert!(matches!(
        FixedCompactTensorLinear::<4, 8>::prepare(
            &weights,
            &bytes,
            &negative_tensor,
            &negative,
            &biases,
            &bias,
            CompactMatrixPacking::Input4Output8Tiles
        ),
        Err(CompactTensorRefusal::Scale)
    ));
    let kernel = FixedCompactTensorLinear::<4, 8>::prepare(
        &weights,
        &bytes,
        &scales,
        &scale,
        &biases,
        &bias,
        CompactMatrixPacking::Input4Output8Tiles,
    )
    .unwrap();
    let mut output = [4.; 8];
    assert_eq!(
        kernel.evaluate(&[127; 4], &mut output),
        Err(CompactTensorRefusal::Nonfinite)
    );
    assert_eq!(output, [4.; 8]);
}
