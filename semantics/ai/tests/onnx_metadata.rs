use conduit_ai::onnx_metadata::{inspect_onnx, OnnxDimension, OnnxMetadataRefusal};
use conduit_data::TensorElement;
use sha2::{Digest, Sha256};

#[path = "common/onnx_fixture.rs"]
mod fixture;
use fixture::{bytes, model, model_with_versions, number, port};

fn inspect(bytes: &[u8]) -> Result<conduit_ai::onnx_metadata::OnnxMetadata, OnnxMetadataRefusal> {
    inspect_onnx(bytes, Sha256::digest(bytes).into())
}

#[test]
fn mixed_named_inputs_keep_exact_types_and_symbolic_dimensions() {
    let metadata = inspect(&model(&[])).expect("finite metadata");
    assert_eq!(metadata.inputs.len(), 2);
    assert_eq!(metadata.inputs[0].name, "tokens");
    assert_eq!(metadata.inputs[0].element, TensorElement::I64);
    assert_eq!(
        metadata.inputs[0].dimensions,
        [OnnxDimension::Symbol("sequence".into())]
    );
    assert_eq!(metadata.inputs[1].element, TensorElement::F32);
    assert_eq!(metadata.outputs[0].dimensions, [OnnxDimension::Fixed(4)]);
    assert_eq!(metadata.opsets, [(String::new(), 18)]);
}

#[test]
fn altered_digest_duplicate_names_and_external_storage_refuse() {
    let original = model(&[]);
    assert_eq!(
        inspect_onnx(&original, [0; 32]),
        Err(OnnxMetadataRefusal::DigestMismatch)
    );
    let duplicate = bytes(11, &port("tokens", 7, number(1, 1)));
    assert_eq!(
        inspect(&model(&duplicate)),
        Err(OnnxMetadataRefusal::DuplicateField)
    );
    for tensor in [number(14, 1), bytes(13, &bytes(1, b"location"))] {
        assert_eq!(
            inspect(&model(&bytes(5, &tensor))),
            Err(OnnxMetadataRefusal::ExternalData)
        );
    }
    // An attribute could contain nested graphs/tensors. This profile refuses
    // the enclosing node instead of ignoring a hidden external resource.
    assert_eq!(
        inspect(&model(&bytes(1, &bytes(5, &[])))),
        Err(OnnxMetadataRefusal::Unsupported)
    );
}

#[test]
fn malformed_shapes_types_and_port_bounds_refuse() {
    for (port, expected) in [
        (
            port("bad", 1, number(1, 0)),
            OnnxMetadataRefusal::InvalidDimension,
        ),
        (
            port("bad", 1, number(1, u64::MAX)),
            OnnxMetadataRefusal::InvalidDimension,
        ),
        (
            port("bad", 8, number(1, 1)),
            OnnxMetadataRefusal::Unsupported,
        ),
        (port("", 1, number(1, 1)), OnnxMetadataRefusal::InvalidName),
    ] {
        assert_eq!(inspect(&model(&bytes(11, &port))), Err(expected));
    }
    let many = (0..30)
        .flat_map(|i| bytes(11, &port(&format!("extra-{i}"), 1, number(1, 1))))
        .collect::<Vec<_>>();
    assert_eq!(inspect(&model(&many)), Err(OnnxMetadataRefusal::PortBound));
    assert_eq!(inspect(&[58, 255]), Err(OnnxMetadataRefusal::Malformed));
}

#[test]
fn ir_and_opset_versions_are_positive_signed_integers() {
    for version in [0, i64::MAX as u64 + 1, u64::MAX] {
        assert_eq!(
            inspect(&model_with_versions(version, 18, &[])),
            Err(OnnxMetadataRefusal::InvalidVersion)
        );
        assert_eq!(
            inspect(&model_with_versions(9, version, &[])),
            Err(OnnxMetadataRefusal::InvalidVersion)
        );
    }
}

#[test]
fn explicit_scalar_shape_differs_from_missing_shape_and_zero_dimension() {
    let scalar = [
        bytes(1, b"scalar"),
        bytes(2, &bytes(1, &[number(1, 7), bytes(2, &[])].concat())),
    ]
    .concat();
    let metadata = inspect(&model(&bytes(11, &scalar))).unwrap();
    assert!(metadata.inputs.last().unwrap().dimensions.is_empty());
    let unknown_tensor = number(1, 7);
    let unknown = [bytes(1, b"unknown"), bytes(2, &bytes(1, &unknown_tensor))].concat();
    assert_eq!(
        inspect(&model(&bytes(11, &unknown))),
        Err(OnnxMetadataRefusal::MissingField)
    );
    assert_eq!(
        inspect(&model(&bytes(11, &port("zero", 7, number(1, 0))))),
        Err(OnnxMetadataRefusal::InvalidDimension)
    );
}
