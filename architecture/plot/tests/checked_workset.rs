//! CPU fabrication evidence, without environmental Source, hardware or admission.
use conduit_plot as b;
use conduit_plot as p;

fn document() -> p::CheckedSyntaxDocument {
    let source = "plot increment (\n >> reading: U8\n result: U8 >>\n) = (reading + 1)\n\
                  plot decrement (\n >> reading: U8\n result: U8 >>\n) = (reading - 1)\n";
    p::check_syntax_document(&p::parse_syntax_document(source), &p::StartupCatalog::new()).unwrap()
}
fn bounds() -> b::CheckedWorksetBounds {
    b::CheckedWorksetBounds {
        maximum_entries: 2,
        maximum_nodes_per_entry: 1,
        maximum_cords_per_entry: 0,
    }
}
#[test]
fn explicit_entries_preserve_order_and_ordinary_expansion_identity() {
    let doc = document();
    let catalog = p::ProfileCatalog::new();
    let plots =
        b::prepare_checked_workset(&doc, &catalog, &["decrement", "increment"], bounds()).unwrap();
    assert_eq!(plots.len(), 2);
    for (plot, name) in plots.iter().zip(["decrement", "increment"]) {
        assert_eq!(
            *plot,
            p::expand_canonical_plot_for_authoring(&doc, name, &catalog).unwrap()
        );
        assert_eq!(plot.expanded.source_document_id, doc.source_document_id);
    }
}

#[test]
fn absent_duplicate_and_oversized_selection_never_produces_a_workset() {
    let doc = document();
    let catalog = p::ProfileCatalog::new();
    for entries in [
        &[][..],
        &["missing"][..],
        &["increment", "increment"][..],
        &["increment", "decrement", "third"][..],
    ] {
        assert!(b::prepare_checked_workset(&doc, &catalog, entries, bounds()).is_err());
    }
    let mut small = bounds();
    small.maximum_nodes_per_entry = 0;
    assert!(b::prepare_checked_workset(&doc, &catalog, &["increment"], small).is_err());
}

#[test]
fn expansion_uses_only_the_explicit_native_catalog() {
    let mut startup = p::StartupCatalog::new();
    startup
        .insert(p::KindSignature {
            kind: "fixture/noop".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    let doc = p::check_syntax_document(
        &p::parse_syntax_document("plot job {\n operation: fixture/noop\n}\n"),
        &startup,
    )
    .unwrap();
    assert!(
        b::prepare_checked_workset(&doc, &p::ProfileCatalog::new(), &["job"], bounds()).is_err()
    );
    let mut catalog = p::ProfileCatalog::new();
    catalog
        .insert(p::KindProjection {
            kind_id: "fixture/noop".into(),
            kind_contract_revision: "fixture/noop@1".into(),
            inputs: vec![],
            outputs: vec![],
            configuration: vec![],
        })
        .unwrap();
    let selected = b::prepare_checked_workset(&doc, &catalog, &["job"], bounds()).unwrap();
    assert_eq!(
        selected[0].expanded.gears[0].kind_id.as_str(),
        "fixture/noop"
    );
}

#[test]
fn cord_ceiling_is_independent_of_entry_and_node_counts() {
    let source = "plot increment (\n >> reading: U8\n result: U8 >>\n) = (reading + 1)\n\
                  plot twice (\n >> input: U8\n result: U8 >>\n) {\n first: increment\n second: increment\n input >> first.reading\n first.result >> second.reading\n second.result >> result\n}\n";
    let doc =
        p::check_syntax_document(&p::parse_syntax_document(source), &p::StartupCatalog::new())
            .unwrap();
    let mut limits = bounds();
    limits.maximum_nodes_per_entry = 2;
    let catalog = p::ProfileCatalog::new();
    assert!(b::prepare_checked_workset(&doc, &catalog, &["twice"], limits).is_err());
    limits.maximum_cords_per_entry = 1;
    let workset = b::prepare_checked_workset(&doc, &catalog, &["twice"], limits).unwrap();
    assert_eq!(workset[0].expanded.gears.len(), 2);
    assert_eq!(workset[0].expanded.connections.len(), 1);
}
