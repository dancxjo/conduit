//! Compile only existing checked Source expressions; Rust owns no timing policy.
pub fn write(document: &conduit_plot::CheckedSyntaxDocument) {
    for name in ["advance", "phases", "curve", "energy"] {
        let plot = format!("speech/phone-gesture-{name}");
        let expanded = conduit_plot::expand_canonical_plot_for_authoring(
            document,
            &plot,
            &conduit_plot::ProfileCatalog::new(),
        )
        .expect("checked phone gesture Source expands");
        let [gear] = expanded.expanded.gears.as_slice() else {
            panic!("one Source expression")
        };
        let [entry] = gear.configuration.as_slice() else {
            panic!("one Source program")
        };
        let conduit_core::ConfigurationValue::Text(program) = &entry.value else {
            panic!("canonical Source program")
        };
        std::fs::write(
            std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap())
                .join(format!("phone_gesture_{name}.hex")),
            program,
        )
        .unwrap();
    }
    std::fs::write(
        std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap())
            .join("phone_gesture_source_id.txt"),
        document.source_document_id.as_str(),
    )
    .unwrap();
}
