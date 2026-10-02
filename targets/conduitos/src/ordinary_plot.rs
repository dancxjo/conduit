//! Checking and expansion boundary for the ordinary text Plot.

use crate::ordinary_plan::PreparationError;

pub(crate) fn checked_expanded_text_plot_named(
    source: &str,
    plot_name: &str,
) -> Result<conduit_plot::ExpandedCanonicalPlot, PreparationError> {
    let syntax = conduit_plot::parse_syntax_document(source);
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile)
        .map_err(|_| PreparationError::PlotRejected)?;
    conduit_text::install_morse_catalogs(&mut startup, &mut profile)
        .map_err(|_| PreparationError::PlotRejected)?;
    conduit_semantic_catalog::install_indicator_presentation_catalog(&mut startup, &mut profile)
        .map_err(|_| PreparationError::PlotRejected)?;
    let checked = conduit_plot::check_syntax_document(&syntax, &startup)
        .map_err(|_| PreparationError::PlotRejected)?;
    conduit_plot::expand_canonical_plot(&checked, plot_name, &profile)
        .map_err(|_| PreparationError::PlotRejected)
}

pub(crate) fn checked_expanded_text_plot_with_morse_backs(
    source: &str,
    plot_name: &str,
) -> Result<conduit_plot::ExpandedCanonicalPlot, PreparationError> {
    let syntax = conduit_plot::parse_syntax_document(source);
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile)
        .map_err(|_| PreparationError::PlotRejected)?;
    conduit_text::install_morse_catalogs(&mut startup, &mut profile)
        .map_err(|_| PreparationError::PlotRejected)?;
    conduit_semantic_catalog::install_indicator_presentation_catalog(&mut startup, &mut profile)
        .map_err(|_| PreparationError::PlotRejected)?;
    let checked = conduit_plot::check_syntax_document(&syntax, &startup)
        .map_err(|_| PreparationError::PlotRejected)?;
    let mut backs = conduit_plot::CanonicalBackCatalog::new();
    conduit_text::install_morse_backs(&startup, &profile, &mut backs)
        .map_err(|_| PreparationError::PlotRejected)?;
    conduit_plot::expand_canonical_plot_for_authoring_with_backs(
        &checked, plot_name, &profile, &backs,
    )
    .map(|result| result.expanded)
    .map_err(|_| PreparationError::PlotRejected)
}

pub(crate) fn checked_expanded_tour_timer_plot(
    source: &str,
    plot_name: &str,
) -> Result<conduit_plot::ExpandedCanonicalPlot, PreparationError> {
    let syntax = conduit_plot::parse_syntax_document(source);
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    conduit_time::install_time_every_catalog(&mut startup, &mut profile)
        .map_err(|_| PreparationError::PlotRejected)?;
    conduit_semantic_catalog::install_count_pipeline_catalogs(&mut startup, &mut profile)
        .map_err(|_| PreparationError::PlotRejected)?;
    let checked = conduit_plot::check_syntax_document(&syntax, &startup)
        .map_err(|_| PreparationError::PlotRejected)?;
    conduit_plot::expand_canonical_plot(&checked, plot_name, &profile)
        .map_err(|_| PreparationError::PlotRejected)
}
