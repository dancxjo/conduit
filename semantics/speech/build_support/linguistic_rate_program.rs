//! Retain the checked finite syllable-density program for preparation.
pub fn write(semantic: &conduit_plot::CheckedSyntaxDocument) {
    let expanded = conduit_plot::expand_canonical_plot_for_authoring(
        semantic,
        "speech/linguistic-rate",
        &conduit_plot::ProfileCatalog::new(),
    )
    .expect("checked linguistic rate expands");
    assert_eq!(expanded.expanded.gears.len(), 1);
    let [entry] = expanded.expanded.gears[0].configuration.as_slice() else {
        panic!("one exact rate program")
    };
    let conduit_core::ConfigurationValue::Text(program) = &entry.value else {
        panic!("rate program bytes")
    };
    std::fs::write(
        std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap())
            .join("linguistic_rate_program.hex"),
        program,
    )
    .expect("retain rate program");
}
