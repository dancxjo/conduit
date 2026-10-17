use conduit_face::application_canvas::{ApplicationCanvas, CanvasNode};
use conduit_face::ApplicationViewRefusal;

fn canvas() -> ApplicationCanvas {
    ApplicationCanvas {
        checked_plot: "checked".into(),
        expanded_plot: "expanded".into(),
        plan: "plan".into(),
        body_plan: "body-plan".into(),
        play: "play".into(),
        selected: String::new(),
        nodes: vec![CanvasNode {
            identity: "gear".into(),
            label: "Gear".into(),
            kind: "kind".into(),
        }],
        ports: vec![],
        cords: vec![],
    }
}

#[test]
fn admission_refuses_oversized_provenance_and_wording() {
    let mut graph = canvas();
    graph.checked_plot = "x".repeat(1025);
    assert_eq!(
        graph.validate(),
        Err(ApplicationViewRefusal::InvalidControlValue)
    );
    let mut graph = canvas();
    graph.nodes[0].label = "é".repeat(513);
    assert_eq!(
        graph.validate(),
        Err(ApplicationViewRefusal::InvalidControlValue)
    );
}

#[test]
fn admission_refuses_a_total_envelope_even_when_individual_fields_fit() {
    let mut graph = canvas();
    graph.nodes = (0..128)
        .map(|index| CanvasNode {
            identity: format!("gear/{index}"),
            label: "x".repeat(1024),
            kind: "kind".into(),
        })
        .collect();
    assert_eq!(
        graph.validate(),
        Err(ApplicationViewRefusal::InvalidControlValue)
    );
}

#[test]
fn exact_multibyte_field_ceiling_still_round_trips() {
    let mut graph = canvas();
    graph.nodes[0].label = "é".repeat(512);
    assert_eq!(graph.validate(), Ok(()));
    assert_eq!(
        ApplicationCanvas::decode(&graph.encode().unwrap()),
        Ok(graph)
    );
}

#[test]
fn exact_canvas_envelope_is_admitted_and_one_more_byte_refuses() {
    let mut graph = canvas();
    graph.nodes = (0..63)
        .map(|index| CanvasNode {
            identity: format!("gear/{index}"),
            label: "x".repeat(if index == 62 { 715 } else { 1024 }),
            kind: "kind".into(),
        })
        .collect();
    assert_eq!(graph.validate(), Ok(()));
    let encoded = graph.encode().unwrap();
    assert_eq!(encoded.len(), 65_536);
    assert_eq!(ApplicationCanvas::decode(&encoded), Ok(graph.clone()));
    graph.nodes[62].label.push('x');
    assert_eq!(
        graph.validate(),
        Err(ApplicationViewRefusal::InvalidControlValue)
    );
}
