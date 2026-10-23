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
        presence: conduit_ai::ModelPortPresence::Required,
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
        .observe(&graph, &bindings, &reader.read(&input).unwrap(), true)
        .unwrap();
    assert!(symbols
        .observe(
            &graph,
            &bindings,
            &reader.read(&wrong_output).unwrap(),
            false
        )
        .is_err());
    assert!(symbols
        .observe(
            &graph,
            &bindings,
            &reader.read(&correct_output).unwrap(),
            false
        )
        .is_ok());
    assert_eq!(symbols.count, 1);
}

#[test]
fn optional_input_absence_does_not_bind_or_retain_a_symbol() {
    let graph = vec![OnnxPort {
        name: "checkpoint-input".into(),
        presence: conduit_ai::ModelPortPresence::Optional,
        element: TensorElement::F32,
        dimensions: vec![OnnxDimension::Symbol("sequence".into())],
    }];
    let bindings = vec![ImportedPortBinding {
        semantic_port: "semantic".into(),
        graph_name: "checkpoint-input".into(),
    }];
    let absent = NamedTensorBundle::from_values(vec![])
        .unwrap()
        .encode()
        .unwrap();
    let present = encoded("semantic", 1);
    let other = encoded("semantic", 2);
    let reader = PreparedNamedTensorBundleView::prepare().unwrap();
    for input in [&absent, &present, &absent] {
        // Each invocation creates its own fixed symbol table.
        let mut symbols = ImportedSymbols {
            model: None,
            values: [None; 256],
            count: 0,
        };
        symbols
            .observe(&graph, &bindings, &reader.read(input).unwrap(), true)
            .unwrap();
        assert_eq!(symbols.count, usize::from(input == &present));
        let different_width =
            symbols.observe(&graph, &bindings, &reader.read(&other).unwrap(), false);
        assert_eq!(different_width.is_ok(), input == &absent);
        // Even an Optional metadata flag cannot exempt an output.
        assert!(symbols
            .observe(&graph, &bindings, &reader.read(&absent).unwrap(), false,)
            .is_err());
    }
    let mut required = graph;
    required[0].presence = conduit_ai::ModelPortPresence::Required;
    let mut symbols = ImportedSymbols {
        model: None,
        values: [None; 256],
        count: 0,
    };
    assert!(symbols
        .observe(&required, &bindings, &reader.read(&absent).unwrap(), true,)
        .is_err());
}
