//! Authored control meaning, separate from any initialized radio, authority or
//! page submission. Checking this Source makes no network availability claim.
pub fn compile_network_setup() -> Result<conduit_plot::ExpandedCanonicalPlot, String> {
    compile(
        include_str!("../../profiles/network.conduit"),
        "environment-network-setup",
        "network setup",
    )
}

pub fn compile_network_scan() -> Result<conduit_plot::ExpandedCanonicalPlot, String> {
    compile(
        include_str!("../../profiles/network-scan.conduit"),
        "environment-network-scan",
        "network scan",
    )
}

fn compile(
    source: &str,
    plot: &str,
    label: &str,
) -> Result<conduit_plot::ExpandedCanonicalPlot, String> {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    conduit_net::install_network_bootstrap_catalogs(&mut startup, &mut profiles)?;
    let syntax = conduit_plot::parse_syntax_document(source);
    if !syntax.diagnostics.is_empty() {
        return Err(format!("{label} Source: {:?}", syntax.diagnostics));
    }
    let checked = conduit_plot::check_syntax_document(&syntax, &startup)
        .map_err(|error| format!("{error:?}"))?;
    conduit_plot::expand_canonical_plot(&checked, plot, &profiles)
        .map_err(|error| format!("{error:?}"))
}
