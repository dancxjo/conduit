//! Exact populated production reply bytes and finite work, before packing.
use super::super::{MAX_DECODED_JSON_BYTES, decode, transport_codec};
use super::*;
use crate::data::Initial;
use alloc::vec;
use patchbay_graph::PatchbayGraph;
use serde::{Deserialize, Serialize};
#[derive(Serialize)]
enum BorrowedReply<'a> {
    Graph { graph: Option<&'a PatchbayGraph> },
}
fn prove(graph: Option<&PatchbayGraph>) -> usize {
    let mut expected = vec![0; MAX_DECODED_JSON_BYTES];
    let length =
        transport_codec::encode_unvalidated(&BorrowedReply::Graph { graph }, &mut expected)
            .unwrap();
    let mut actual = vec![0; MAX_DECODED_JSON_BYTES];
    let mut encoder = GraphEncoding::new();
    let mut turns = 0;
    loop {
        let before = encoder.cursor();
        let complete = encoder.step(graph, &mut actual).unwrap();
        turns += 1;
        assert!(turns <= MAX_TURNS);
        let work = encoder.cursor() - before;
        assert!(work > 0 && work <= u64::from(STEP_WORK));
        if complete {
            break;
        }
        assert!(work >= u64::from(STEP_WORK - RESOLVE_RESERVE));
    }
    assert_eq!(encoder.length(), length);
    assert_eq!(&actual[..length], &expected[..length]);
    assert_eq!(encoder.step(graph, &mut actual), Err(Refusal::Order));
    turns as usize
}
#[test]
fn all_real_populated_graphs_and_absence_match_exact_existing_encoder() {
    #[derive(Deserialize)]
    enum Fixture {
        Init {
            applications: [Option<Initial>; crate::SLOTS],
        },
    }
    let Fixture::Init { applications } =
        decode(include_bytes!("../../../../tests/fixtures/startup.catz")).unwrap();
    let Some(Initial::Patchbay { targets, .. }) = &applications[0] else {
        panic!("actual populated fixture")
    };
    let mut populated = 0;
    for target in targets.iter().flatten() {
        assert!(!target.graph.gears.is_empty());
        assert!(prove(Some(&target.graph)) > 1);
        populated += 1;
    }
    assert!(populated > 0);
    assert!(prove(None) >= 1);
}
#[test]
fn capacity_and_terminal_failures_never_restart_or_publish() {
    let mut encoder = GraphEncoding::new();
    let mut output = [0; 1];
    assert_eq!(encoder.step(None, &mut output), Err(Refusal::Capacity));
    assert_eq!(encoder.step(None, &mut output), Err(Refusal::Order));
}

#[test]
fn oversized_output_slice_cannot_enlarge_the_decoded_ceiling() {
    let text = "x".repeat(MAX_DECODED_JSON_BYTES);
    let mut encoder = GraphEncoding::for_node();
    let mut output = vec![0; MAX_DECODED_JSON_BYTES + 128];
    let mut refused = false;
    for _ in 0..MAX_TURNS {
        match encoder.step_node(Node::Str(&text), &mut output) {
            Ok(false) => {}
            Err(Refusal::Capacity) => {
                refused = true;
                break;
            }
            result => panic!("oversized output must refuse, got {result:?}"),
        }
    }
    assert!(refused);
    assert_eq!(encoder.length(), MAX_DECODED_JSON_BYTES);
    assert_eq!(
        encoder.step_node(Node::Str(&text), &mut output),
        Err(Refusal::Order)
    );
}

#[test]
fn outer_wire_schema_covers_fronts_compositions_origins_and_optional_descriptors() {
    use conduit_core::{PortDirection, PortTemporal};
    use conduit_plot::{ExpandedConnectionOrigin, Span};
    use patchbay_graph::{
        PatchbayComposition, PatchbayCompositionBinding, PatchbayCordSource, PatchbayFrontPort,
    };
    #[derive(Deserialize)]
    enum Fixture {
        Init {
            applications: [Option<Initial>; crate::SLOTS],
        },
    }
    let Fixture::Init { applications } =
        decode(include_bytes!("../../../../tests/fixtures/startup.catz")).unwrap();
    let Some(Initial::Patchbay { targets, .. }) = &applications[0] else {
        panic!("fixture")
    };
    let mut graph = targets.iter().flatten().next().unwrap().graph.clone();
    let port = graph
        .gears
        .iter()
        .flat_map(|gear| gear.inputs.iter().chain(&gear.outputs))
        .next()
        .unwrap()
        .clone();
    let contract = conduit_core::CheckedValueContract {
        value_kind: port.descriptor.value_kind.clone(),
        maximum_bytes: 4096,
        constraints: vec![conduit_core::ValueConstraint::ByteLength {
            minimum: 0,
            maximum: 4096,
        }],
    };
    graph
        .gears
        .iter_mut()
        .flat_map(|gear| gear.inputs.iter_mut().chain(&mut gear.outputs))
        .next()
        .unwrap()
        .value_contract = Some(contract.clone());
    graph.gears[0].controls = vec![patchbay_graph::FaceControl {
        key: "enabled".into(),
        value: conduit_core::ConfigurationValue::Bool(false),
        kind: patchbay_graph::FaceControlKind::BooleanChoice {
            choices: ["false", "true"],
        },
        interaction: None,
    }];
    // These are wire fixtures, not a claim that arbitrary edits grant a checked
    // Plot. Real model admission is covered by the immutable production fixture.
    let mut input = PatchbayFrontPort {
        identity: "input/é\\\"".into(),
        descriptor: port.descriptor.clone(),
        value_contract: Some(contract),
    };
    input.descriptor.direction = PortDirection::Input;
    input.descriptor.abnormal_kind = Some(conduit_core::KindId::from("info/abnormal"));
    let mut output = input.clone();
    output.identity = "output/λ".into();
    output.descriptor.direction = PortDirection::Output;
    graph.front_inputs = vec![input.clone()];
    graph.front_outputs = vec![output.clone()];
    graph.compositions = vec![PatchbayComposition {
        identity: "composition/one".into(),
        gear_name: "gear/one".into(),
        back_name: "back/one".into(),
        checked_plot_id: graph.checked_plot_id.clone(),
        inputs: vec![input],
        outputs: vec![output],
        input_bindings: vec![PatchbayCompositionBinding {
            front_port: "in".into(),
            internal_port: "inner/in".into(),
        }],
        output_bindings: vec![PatchbayCompositionBinding {
            front_port: "out".into(),
            internal_port: "inner/out".into(),
        }],
    }];
    for temporal in [
        PortTemporal::Value,
        PortTemporal::Current,
        PortTemporal::Flow { closes: false },
        PortTemporal::Flow { closes: true },
    ] {
        graph.front_inputs[0].descriptor.temporal = temporal;
        for origin in [
            ExpandedConnectionOrigin::CordStages,
            ExpandedConnectionOrigin::RelationalOperand,
            ExpandedConnectionOrigin::SemanticExpression,
        ] {
            graph.cords[0].source_origin = Some(PatchbayCordSource {
                source_document_id: graph.source_document_id.clone(),
                checked_plot_id: graph.checked_plot_id.clone(),
                source_plot: "nested/é\\\"".into(),
                plot_path: vec!["outer".into(), "inner/λ".into()],
                source_span: Span {
                    start: 3,
                    end: 17,
                    line: 2,
                    column: 3,
                    end_line: 4,
                    end_column: 8,
                },
                origin,
            });
            prove(Some(&graph));
        }
    }
    graph.compositions[0].inputs.clear();
    graph.compositions[0].outputs.clear();
    graph.compositions[0].input_bindings.clear();
    graph.compositions[0].output_bindings.clear();
    graph.gears[0].plot_path = vec!["outer".into(), "inner/λ".into()];
    prove(Some(&graph));
    graph.front_inputs.clear();
    graph.front_outputs.clear();
    graph.compositions.clear();
    graph.gears.clear();
    graph.cords.clear();
    prove(Some(&graph));
}
