#![cfg(feature = "kernel-step")]
use conduit_ai::fixed_numeric_catalog::*;
use conduit_ai::fixed_numeric_pair_catalog::install_fixed_numeric_pair_catalogs;
use conduit_plot::*;

fn source() -> String {
    [
        include_str!("../../speech/fargan_conditioning.conduit"),
        include_str!("../../speech/fargan_signal.conduit"),
        include_str!("../../speech/fargan_pitch_history.conduit"),
        include_str!("../../speech/fargan_subframe.conduit"),
        include_str!("../../speech/fargan_frame.conduit"),
    ]
    .join("\n")
}

#[test]
fn pinned_conditioning_and_complete_recurrent_subframe_are_checked_source() {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    install_fixed_numeric_catalogs_capacity64(&mut startup, &mut profiles).unwrap();
    install_fixed_numeric_pair_catalogs(&mut startup, &mut profiles).unwrap();
    let source = source();
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    for name in [
        "speech/fargan-conditioning",
        "speech/fargan-subframe",
        "speech/fargan-frame",
    ] {
        let graph = expand_canonical_plot_for_authoring(&checked, name, &profiles).unwrap();
        assert!(!graph.expanded.gears.is_empty());
        assert!(graph
            .expanded
            .gears
            .iter()
            .all(|gear| !gear.kind_id.as_str().starts_with("speech/fargan")));
        eprintln!(
            "{name}: {} gears, {} cords",
            graph.expanded.gears.len(),
            graph.expanded.connections.len()
        );
    }
}

#[test]
fn extended_catalog_preserves_existing_type_identities_and_native_laws() {
    let mut startup = StartupCatalog::new();
    startup
        .insert_value_kind_alias(
            "ResourceRef",
            conduit_core::kind_id(conduit_core::RESOURCE_REFERENCE_INFO_ID),
        )
        .unwrap();
    let baseline =
        check_syntax_document(&parse_syntax_document(FIXED_NUMERIC_SOURCE), &startup).unwrap();
    let extended = fixed_numeric_types().unwrap();
    for old in baseline.native_types {
        let current = extended.iter().find(|ty| ty.name == old.name).unwrap();
        assert_eq!(current.value_type, old.value_type);
        assert_eq!(current.value_contracts, old.value_contracts);
        assert_eq!(current.invariants, old.invariants);
    }
    let mut profiles = ProfileCatalog::new();
    install_fixed_numeric_catalogs_capacity64(&mut startup, &mut profiles).unwrap();
    let rebuild = "plot rebuild (
 value: NumericF32MatrixRef3x2 >> result: NumericF32MatrixRef3x2
) = ({resource: .resource, content_digest: .content_digest, element: .element, columns: .columns, rows: .rows})
";
    let identity = "plot rebuild (\n value: NumericF32MatrixRef3x2 >> result: NumericF32MatrixRef3x2\n) = (.)\n";
    let checked = check_syntax_document(&parse_syntax_document(identity), &startup).unwrap();
    expand_canonical_plot_for_authoring(&checked, "rebuild", &profiles).unwrap();
    let invalid = rebuild.replace("columns: .columns", "columns: .rows");
    let checked = check_syntax_document(&parse_syntax_document(&invalid), &startup).unwrap();
    assert_eq!(
        expand_canonical_plot_for_authoring(&checked, "rebuild", &profiles)
            .unwrap_err()
            .code,
        "CND-FRM-046"
    );
}
