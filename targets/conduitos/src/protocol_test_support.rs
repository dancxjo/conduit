//! Shared checked clock type for device-protocol fixtures; no offer or possession.
pub(crate) fn install_clock_result(startup: &mut conduit_plot::StartupCatalog) {
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(crate::monotonic_clock::contract::CLOCK_TYPES),
        &conduit_plot::StartupCatalog::new(),
    )
    .unwrap();
    startup
        .insert_checked_native_type(
            "machine/clock/at/result",
            checked
                .native_types
                .iter()
                .find(|ty| ty.name == "MonotonicClockResult")
                .unwrap(),
        )
        .unwrap();
}
