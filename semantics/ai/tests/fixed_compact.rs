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
    assert_eq!(
        kernel.resources(),
        [Some(&weights), Some(&scale_tensor), Some(&bias_tensor)]
    );
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

#[test]
fn unbiased_compact_linear_retains_only_declared_resources() {
    let weights_bytes = [1u8; 32];
    let scales_bytes = packed(&[0.25; 8]);
    let weights = tensor(TensorElement::I8, &[4, 8], &weights_bytes);
    let scales = tensor(TensorElement::F32, &[8], &scales_bytes);
    let kernel = FixedCompactTensorLinear::<4, 8>::prepare_unbiased(
        &weights,
        &weights_bytes,
        &scales,
        &scales_bytes,
        CompactMatrixPacking::Input4Output8Tiles,
    )
    .unwrap();
    assert_eq!(kernel.resources(), [Some(&weights), Some(&scales), None]);
    let mut output = [0.; 8];
    kernel.evaluate(&[1, 2, 3, 4], &mut output).unwrap();
    assert_eq!(output, [2.5; 8]);
}

// Development-only operator comparison. The layer list describes this oracle's
// records; it is not a product network driver or an execution-order contract.
#[test]
#[ignore = "requires explicitly supplied pinned local compact model and scalar oracle"]
fn pinned_compact_linear_differential() {
    use sha2::{Digest, Sha256};
    let root = std::path::PathBuf::from(
        std::env::var_os("CONDUIT_FARGAN_DEVELOPMENT_FIXTURE")
            .expect("explicit local fixture path"),
    );
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("resources/manifest.json")).unwrap())
            .unwrap();
    assert_eq!(
        manifest["upstream_sha"],
        "503d81b138d76621aae4b12786e90de48aa8db3a"
    );
    let blob = std::fs::read(root.join("resources/compact.bin")).unwrap();
    assert_eq!(blob.len(), 872_164);
    assert_eq!(
        format!("{:x}", Sha256::digest(&blob)),
        "57afaa2df88a95fb09f0649934f1558ff8c46d82fa7627895535c65ed22e5891"
    );
    let trace = std::fs::read(root.join("compact-linear.trace")).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&trace)),
        "a7cc9ddf71bdb661141dab60929e2e0ce716f9490c04d7e66f0238a5ad00480e"
    );
    let array = |name: &str| {
        let a = manifest["arrays"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["name"] == name)
            .unwrap();
        let offset = a["profiles"]["compact"]["offset"].as_u64().unwrap() as usize;
        let length = a["profiles"]["compact"]["bytes"].as_u64().unwrap() as usize;
        let bytes = &blob[offset..offset + length];
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            a["sha256"].as_str().unwrap()
        );
        bytes
    };
    let mut cursor = 0;
    let mut maximum = 0f32;
    type Comparison<'a> = dyn FnMut(&[f32], &[f32], &mut f32) + 'a;
    let mut compare = |case: usize, name: &str, run: &mut Comparison<'_>| {
        for test in 0..3 {
            let header: Vec<u32> = trace[cursor..cursor + 16]
                .as_chunks::<4>()
                .0
                .iter()
                .map(|b| u32::from_le_bytes(*b))
                .collect();
            cursor += 16;
            assert_eq!((header[0], header[1]), (case as u32, test));
            let mut read = |count: usize| {
                let result: Vec<f32> = trace[cursor..cursor + count * 4]
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|b| f32::from_le_bytes(*b))
                    .collect();
                cursor += count * 4;
                result
            };
            let input = read(header[2] as usize);
            let expected = read(header[3] as usize);
            run(&input, &expected, &mut maximum);
        }
        eprintln!("compact operator {name}: cumulative maximum absolute error {maximum}");
    };
    macro_rules! check {
        ($case:expr, $name:literal, $inputs:literal, $outputs:literal, $bias:expr) => {{
            let w = array(concat!($name, "_weights_int8"));
            let s = array(concat!($name, "_scale"));
            let weights = tensor(TensorElement::I8, &[$inputs, $outputs], w);
            let scales = tensor(TensorElement::F32, &[$outputs], s);
            let b = if $bias {
                Some(array(concat!($name, "_bias")))
            } else {
                None
            };
            let bias = b.map(|b| tensor(TensorElement::F32, &[$outputs], b));
            let kernel = match (bias.as_ref(), b) {
                (Some(bias), Some(bytes)) => {
                    FixedCompactTensorLinear::<$inputs, $outputs>::prepare(
                        &weights,
                        w,
                        &scales,
                        s,
                        bias,
                        bytes,
                        CompactMatrixPacking::Input4Output8Tiles,
                    )
                }
                _ => FixedCompactTensorLinear::<$inputs, $outputs>::prepare_unbiased(
                    &weights,
                    w,
                    &scales,
                    s,
                    CompactMatrixPacking::Input4Output8Tiles,
                ),
            }
            .unwrap();
            compare($case, $name, &mut |input, expected, maximum| {
                let input: &[f32; $inputs] = input.try_into().unwrap();
                let mut quantized = [0; $inputs];
                fixed_signed_q7(input, &mut quantized).unwrap();
                let mut actual = [0.; $outputs];
                kernel.evaluate(&quantized, &mut actual).unwrap();
                for (got, want) in actual.iter().zip(expected) {
                    let error = (got - want).abs();
                    *maximum = maximum.max(error);
                    assert!(error <= 2e-6, "{}: {got} != {want}", $name);
                }
            });
        }};
    }
    check!(0, "cond_net_fconv1", 192, 128, true);
    check!(1, "cond_net_fdense2", 128, 320, true);
    check!(2, "sig_net_fwc0_conv", 328, 192, true);
    check!(3, "sig_net_fwc0_glu_gate", 192, 192, true);
    check!(4, "sig_net_gru1_input", 272, 480, false);
    check!(5, "sig_net_gru1_recurrent", 160, 480, false);
    check!(6, "sig_net_gru2_input", 240, 384, false);
    check!(7, "sig_net_gru2_recurrent", 128, 384, false);
    check!(8, "sig_net_gru3_input", 208, 384, false);
    check!(9, "sig_net_gru3_recurrent", 128, 384, false);
    check!(10, "sig_net_gru1_glu_gate", 160, 160, true);
    check!(11, "sig_net_gru2_glu_gate", 128, 128, true);
    check!(12, "sig_net_gru3_glu_gate", 128, 128, true);
    check!(13, "sig_net_skip_glu_gate", 128, 128, true);
    check!(14, "sig_net_skip_dense", 688, 128, true);
    check!(15, "sig_net_sig_dense_out", 128, 40, true);
    assert_eq!(cursor, trace.len());
    eprintln!("48 pinned compact operator cases: maximum absolute error {maximum}");
}
