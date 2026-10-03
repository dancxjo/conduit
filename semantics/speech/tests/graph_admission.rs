//! The compiled Back accepts only exact, finite, value-only expression cords.
#[path = "../build_support/graph.rs"]
mod graph;
#[path = "../build_support/lower.rs"]
mod lower;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ExpandedAuthoringPlot, ProfileCatalog, StartupCatalog,
};

fn graph() -> ExpandedAuthoringPlot {
    let source = "plot identity (\n >> value: I64\n result: I64 >>\n) = (.)\nplot composed (\n >> value: I64\n result: I64 >>\n) {\n a: identity\n b: identity\n c: identity\n value >> a.value\n a.result >> b.value\n b.result >> c.value\n c.result >> result\n}\n";
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    expand_canonical_plot_for_authoring(&checked, "composed", &ProfileCatalog::new()).unwrap()
}
#[test]
fn checked_connections_determine_static_order_and_front_result() {
    let lowered = graph::function("composed", &graph(), &[]).unwrap();
    assert_eq!(
        lowered
            .graph
            .iter()
            .map(|(source, _)| *source)
            .collect::<Vec<_>>(),
        [usize::MAX, 0, 1]
    );
    assert_eq!(lowered.result, 2);
    assert_eq!(lowered.programs.len(), 3);
    assert!(lowered
        .source
        .contains("let step1 = composed_step_1(step0)?;"));
}
#[test]
fn unbound_cycles_duplicate_writers_temporal_edges_and_large_graphs_refuse() {
    let valid = graph();
    let mut cycle = valid.clone();
    cycle.expanded.connections[0].source_gear_id =
        cycle.expanded.connections[1].sink_gear_id.clone();
    assert!(graph::function("cycle", &cycle, &[]).is_err());
    let mut duplicate = valid.clone();
    duplicate
        .expanded
        .connections
        .push(duplicate.expanded.connections[0].clone());
    assert!(graph::function("duplicate", &duplicate, &[]).is_err());
    let mut unbound = valid.clone();
    unbound.expanded.connections.pop();
    assert!(graph::function("unbound", &unbound, &[]).is_err());
    let mut temporal = valid.clone();
    temporal.expanded.connections[0].temporal = conduit_core::PortTemporal::Current;
    assert!(graph::function("temporal", &temporal, &[]).is_err());
    let mut large = valid;
    large
        .expanded
        .gears
        .resize(257, large.expanded.gears[0].clone());
    assert!(graph::function("large", &large, &[]).is_err());
}

#[test]
fn profile_constants_require_closed_literal_trees() {
    let dynamic = graph::function("dynamic", &graph(), &[]).unwrap();
    let program =
        conduit_plot::PortableExpressionProgram::from_canonical_hex(&dynamic.programs[0].1)
            .unwrap();
    assert!(lower::constant(&program, &[]).is_err());
    let source = "plot literal (\n >> value: I64\n result: I64 >>\n) = (17)\n";
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let plot =
        expand_canonical_plot_for_authoring(&checked, "literal", &ProfileCatalog::new()).unwrap();
    let lowered = graph::function("literal", &plot, &[]).unwrap();
    let program =
        conduit_plot::PortableExpressionProgram::from_canonical_hex(&lowered.programs[0].1)
            .unwrap();
    assert_eq!(lower::constant(&program, &[]).unwrap(), "(17_i64)");
}
