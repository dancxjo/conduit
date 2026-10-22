//! Connection origins must survive composition, fan-out and identity validation.
use super::*;

#[test]
fn connection_origin_selected_back_keeps_its_own_document_and_declaration() {
    let (mut startup, mut profile) = catalogs();
    startup
        .insert(KindSignature {
            kind: "test/high".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    let high = kind_id("test/high");
    profile
        .insert_kind(canonical_kind(KindProjection {
            kind_id: high.clone(),
            kind_contract_revision: KindIdentity::from("test/high@1"),
            inputs: vec![port("in", PortDirection::Input)],
            outputs: vec![port("out", PortDirection::Output)],
            configuration: vec![],
        }))
        .unwrap();
    let caller = "plot main {\n source: test/source\n high: test/high\n sink: test/sink\n source >> high >> sink\n}\n";
    let back = "plot test/high (\n in: test/value >> out: test/value\n) {\n plot helper (\n input: test/value >> output: test/value\n ) {\n left: test/pass\n right: test/pass\n input >> left >> right >> output\n }\n child: helper\n in >> child.input\n child.output >> out\n}\n";
    let caller = check_syntax_document(&parse_syntax_document(caller), &startup).unwrap();
    let document = check_syntax_document(&parse_syntax_document(back), &startup).unwrap();
    let mut backs = crate::CanonicalBackCatalog::new();
    backs
        .insert(
            profile.canonical_kind(&high).unwrap(),
            &document,
            "test/high",
        )
        .unwrap();
    let expanded =
        crate::expand_canonical_plot_with_backs(&caller, "main", &profile, &backs).unwrap();
    assert_eq!(
        expanded.connection_provenance.len(),
        expanded.connections.len()
    );
    let internal = expanded
        .connection_provenance
        .iter()
        .find(|row| {
            row.connection.source_gear_id.as_str() == "main/high/child/left"
                && row.connection.sink_gear_id.as_str() == "main/high/child/right"
        })
        .unwrap();
    assert_eq!(internal.source_document_id, document.source_document_id);
    assert_eq!(internal.plot_path, ["main", "high", "child"]);
    let helper = document
        .plots
        .iter()
        .find(|plot| plot.checked_plot_id == internal.checked_plot_id)
        .unwrap();
    assert_eq!(internal.source_plot, helper.name);
    assert_eq!(
        &back[internal.source_span.start..internal.source_span.end],
        "input >> left >> right >> output"
    );
    for row in expanded
        .connection_provenance
        .iter()
        .filter(|row| row != &internal)
    {
        assert_eq!(row.source_document_id, caller.source_document_id);
        assert_eq!(row.checked_plot_id, caller.plots[0].checked_plot_id);
    }
    expanded.validate_expansion().unwrap();
}

#[test]
fn connection_origin_fanout_and_mutation_are_bound_to_exact_graph() {
    let source = "plot fan (\n >> input: test/value\n) {\n left: test/pass\n right: test/pass\n input >> left\n input >> right\n}\nplot main {\n source: test/source\n fan: fan\n source >> fan.input\n}\n";
    let baseline = expand(source, "main");
    assert_eq!(baseline.connections.len(), 2);
    assert_eq!(baseline.connection_provenance.len(), 2);
    for row in &baseline.connection_provenance {
        assert_eq!(row.origin, crate::ExpandedConnectionOrigin::CordStages);
        assert_eq!(row.plot_path, ["main"]);
        assert_eq!(
            source[row.source_span.start..row.source_span.end].trim(),
            "source >> fan.input"
        );
    }
    let mut changed = baseline.clone();
    changed.connection_provenance[0].source_span.start += 1;
    assert!(changed.validate_expansion().is_err());
    assert_eq!(changed.expanded_plot_id, baseline.expanded_plot_id);
    let mut changed = baseline.clone();
    changed.connection_provenance[0].connection.track = conduit_core::ConnectionTrack::NormalClose;
    assert!(changed.validate_expansion().is_err());
    let mut changed = baseline;
    changed.connection_provenance.pop();
    assert!(changed.validate_expansion().is_err());
}

#[test]
fn connection_origin_root_fore_keeps_its_cord_and_refuses_rewritten_mapping() {
    let (startup, profile) = catalogs();
    let source = "plot main (\n input: test/value >> output: test/value\n) {\n worker: test/pass\n input >> worker >> output\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let baseline = crate::expand_canonical_plot_for_authoring(&checked, "main", &profile).unwrap();
    baseline.validate_boundary_provenance().unwrap();
    for binding in baseline
        .input_bindings
        .iter()
        .chain(&baseline.output_bindings)
    {
        let origin = binding
            .source_origin
            .as_ref()
            .expect("authored Cord origin");
        assert_eq!(
            source[origin.source_span.start..origin.source_span.end].trim(),
            "input >> worker >> output"
        );
        assert_eq!(origin.origin, crate::ExpandedConnectionOrigin::CordStages);
    }
    assert_eq!(baseline.input_bindings.len(), 1);
    assert_eq!(baseline.output_bindings.len(), 1);
    let mut changed = baseline.clone();
    changed.input_bindings[0]
        .source_origin
        .as_mut()
        .unwrap()
        .source_span
        .line += 1;
    assert!(changed.validate_boundary_provenance().is_err());
    changed.expanded.validate_expansion().unwrap();
    assert_eq!(
        changed.expanded.expanded_plot_id,
        baseline.expanded.expanded_plot_id
    );
    let mut changed = baseline.clone();
    changed.output_bindings[0].track = conduit_core::ConnectionTrack::NormalClose;
    assert!(changed.validate_boundary_provenance().is_err());
    let mut changed = baseline.clone();
    changed.input_bindings[0].source_origin = None;
    assert!(changed.validate_boundary_provenance().is_err());
}
