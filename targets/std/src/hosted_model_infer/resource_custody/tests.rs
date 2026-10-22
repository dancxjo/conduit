//! Actual borrowed canonical views retain imported symbol equality.
use super::*;
use conduit_ai::{
    imported_onnx::ImportedPortBinding,
    onnx_metadata::{OnnxDimension, OnnxPort},
};
use conduit_data::*;
use conduit_plot::rust_binding::{BoundedBytes, BoundedSequence, NativeRustBinding};

fn encoded(name: &str, length: u64) -> Vec<u8> {
    let bytes = vec![0; length as usize * 4];
    NamedTensorBundle::from_values(vec![NamedTensor {
        name: name.into(),
        value: TensorValue {
            element: TensorElement::F32,
            dimensions: BoundedSequence::try_from_iter([length]).unwrap(),
            axes: BoundedSequence::try_from_iter([TensorAxis {
                role: TensorAxisRole::Feature,
                identity: None,
                unit: None,
            }])
            .unwrap(),
            content_digest: tensor_content_digest(&bytes),
            backing: TensorBacking::Inline(BoundedBytes::new(&bytes).unwrap()),
        },
    }])
    .unwrap()
    .encode()
    .unwrap()
}
#[test]
fn borrowed_outputs_cannot_change_an_actual_input_symbol() {
    let graph = vec![OnnxPort {
        name: "checkpoint-input".into(),
        element: TensorElement::F32,
        dimensions: vec![OnnxDimension::Symbol("sequence".into())],
    }];
    let bindings = vec![ImportedPortBinding {
        semantic_port: "semantic".into(),
        graph_name: "checkpoint-input".into(),
    }];
    let input = encoded("semantic", 1);
    let wrong_output = encoded("semantic", 2);
    let correct_output = encoded("semantic", 1);
    let reader = PreparedNamedTensorBundleView::prepare().unwrap();
    let mut symbols = ImportedSymbols {
        model: None,
        values: [None; 256],
        count: 0,
    };
    symbols
        .observe(&graph, &bindings, &reader.read(&input).unwrap())
        .unwrap();
    assert!(symbols
        .observe(&graph, &bindings, &reader.read(&wrong_output).unwrap())
        .is_err());
    assert!(symbols
        .observe(&graph, &bindings, &reader.read(&correct_output).unwrap())
        .is_ok());
    assert_eq!(symbols.count, 1);
}
