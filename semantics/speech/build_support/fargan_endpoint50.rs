//! Separate checked endpoint Source: does not extend main Speech/Language Source.
pub fn write(owner: &conduit_plot::CheckedSyntaxDocument) {
    use conduit_plot::*;
    let cycle = owner
        .native_types
        .iter()
        .find(|t| t.name == "SpeechFundamentalCycle")
        .expect("Speech cycle owner");
    let mut startup = StartupCatalog::new();
    startup
        .insert_checked_native_type("SpeechFundamentalCycle", cycle)
        .unwrap();
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("../fargan_endpoint_policy.conduit")),
        &startup,
    )
    .expect("checked endpoint Source imports exact cycle owner");
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let bindings = rust_binding::generate_rust_bindings_with_external_bindings(
        &checked.native_types,
        std::slice::from_ref(&cycle.value_type),
        &[rust_binding::ExternalNativeRustBinding {
            semantic_identity: cycle.identity.as_str(),
            rust_type_path: "crate::semantic::SpeechFundamentalCycle",
        }],
        &rust_binding::RustBindingOptions::default(),
    )
    .expect("endpoint bindings reuse cycle owner");
    std::fs::write(output.join("fargan_endpoint50_types.rs"), bindings.source).unwrap();
    for (entry, name) in [
        ("speech/fargan-endpoint-constant-cycles50", "gate"),
        ("speech/fargan-endpoint-step50", "step"),
    ] {
        let expanded = expand_canonical_plot_for_authoring(&checked, entry, &ProfileCatalog::new())
            .expect("endpoint Source expands");
        assert_eq!(expanded.expanded.gears.len(), 1);
        let [config] = expanded.expanded.gears[0].configuration.as_slice() else {
            panic!("one endpoint expression program")
        };
        let conduit_core::ConfigurationValue::Text(hex) = &config.value else {
            panic!("endpoint program hex")
        };
        std::fs::write(output.join(format!("fargan_endpoint50_{name}.hex")), hex).unwrap();
    }
    std::fs::write(
        output.join("fargan_endpoint50_source_id.txt"),
        checked.source_document_id.as_str(),
    )
    .unwrap();
}
