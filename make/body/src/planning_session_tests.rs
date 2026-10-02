//! Body-owned planning-session proof using ordinary checked and expanded Plots.

mod execution;
mod proposal;

fn hello_plot() -> conduit_plot::ExpandedCanonicalPlot {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile).unwrap();
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(include_str!("../../../plots/hello/main.conduit")),
        &startup,
    )
    .unwrap();
    conduit_plot::expand_canonical_plot(&checked, "hello", &profile).unwrap()
}
