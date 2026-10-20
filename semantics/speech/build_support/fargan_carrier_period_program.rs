//! Retain the checked bounded original-cycle period program for preparation.
pub fn write(semantic: &conduit_plot::CheckedSyntaxDocument) {
    let expanded = conduit_plot::expand_canonical_plot_for_authoring(
        semantic,
        "speech/fargan-carrier-period",
        &conduit_plot::ProfileCatalog::new(),
    )
    .expect("checked FARGAN carrier period expands");
    assert_eq!(expanded.expanded.gears.len(), 1);
    let [entry] = expanded.expanded.gears[0].configuration.as_slice() else {
        panic!("one exact period program")
    };
    let conduit_core::ConfigurationValue::Text(program) = &entry.value else {
        panic!("period program bytes")
    };
    std::fs::write(
        std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap())
            .join("fargan_carrier_period_program.hex"),
        program,
    )
    .expect("retain period program");
}
