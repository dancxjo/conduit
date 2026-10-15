use conduit_ai::onnx_metadata::{inspect_onnx, OnnxDimension, OnnxMetadataRefusal};
use conduit_data::TensorElement;
use sha2::{Digest, Sha256};

// Synthetic metadata uses the pinned ONNX protobuf field numbers. No downloaded
// weights, provider library or network access participates in these tests.
fn varint(mut n: u64) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let byte = (n & 127) as u8;
        n >>= 7;
        out.push(byte | if n == 0 { 0 } else { 128 });
        if n == 0 {
            return out;
        }
    }
}
fn bytes(field: u64, value: &[u8]) -> Vec<u8> {
    [
        varint(field * 8 + 2),
        varint(value.len() as u64),
        value.to_vec(),
    ]
    .concat()
}
fn number(field: u64, value: u64) -> Vec<u8> {
    [varint(field * 8), varint(value)].concat()
}
fn port(name: &str, dtype: u64, dimension: Vec<u8>) -> Vec<u8> {
    let shape = bytes(1, &dimension);
    let tensor = [number(1, dtype), bytes(2, &shape)].concat();
    [bytes(1, name.as_bytes()), bytes(2, &bytes(1, &tensor))].concat()
}
fn model(extra_graph: &[u8]) -> Vec<u8> {
    let graph = [
        bytes(2, b"tiny-mixed-input"),
        bytes(11, &port("tokens", 7, bytes(2, b"sequence"))),
        bytes(11, &port("scale", 1, number(1, 1))),
        bytes(12, &port("result", 1, number(1, 4))),
        extra_graph.to_vec(),
    ]
    .concat();
    [number(1, 9), bytes(8, &number(2, 18)), bytes(7, &graph)].concat()
}
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
