//! Checking and expansion boundary for the ordinary text Form.

use crate::ordinary_plan::PreparationError;

pub(crate) fn checked_expanded_text_form_named(
    source: &str,
    form_name: &str,
) -> Result<conduit_form::ExpandedCanonicalForm, PreparationError> {
    let syntax = conduit_form::parse_syntax_document(source);
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile)
        .map_err(|_| PreparationError::FormRejected)?;
    conduit_text::install_morse_catalogs(&mut startup, &mut profile)
        .map_err(|_| PreparationError::FormRejected)?;
    conduit_semantic_catalog::install_indicator_presentation_catalog(&mut startup, &mut profile)
        .map_err(|_| PreparationError::FormRejected)?;
    let checked = conduit_form::check_syntax_document(&syntax, &startup)
        .map_err(|_| PreparationError::FormRejected)?;
    conduit_form::expand_canonical_form(&checked, form_name, &profile)
        .map_err(|_| PreparationError::FormRejected)
}

pub(crate) fn checked_expanded_text_form_with_morse_backs(
    source: &str,
    form_name: &str,
) -> Result<conduit_form::ExpandedCanonicalForm, PreparationError> {
    let syntax = conduit_form::parse_syntax_document(source);
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile)
        .map_err(|_| PreparationError::FormRejected)?;
    conduit_text::install_morse_catalogs(&mut startup, &mut profile)
        .map_err(|_| PreparationError::FormRejected)?;
    conduit_semantic_catalog::install_indicator_presentation_catalog(&mut startup, &mut profile)
        .map_err(|_| PreparationError::FormRejected)?;
    let checked = conduit_form::check_syntax_document(&syntax, &startup)
        .map_err(|_| PreparationError::FormRejected)?;
    let mut backs = conduit_form::CanonicalBackCatalog::new();
    conduit_text::install_morse_backs(&startup, &profile, &mut backs)
        .map_err(|_| PreparationError::FormRejected)?;
    conduit_form::expand_canonical_form_for_authoring_with_backs(
        &checked, form_name, &profile, &backs,
    )
    .map(|result| result.expanded)
    .map_err(|_| PreparationError::FormRejected)
}

pub(crate) fn checked_expanded_tour_timer_form(
    source: &str,
    form_name: &str,
) -> Result<conduit_form::ExpandedCanonicalForm, PreparationError> {
    let syntax = conduit_form::parse_syntax_document(source);
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_time::install_time_every_catalog(&mut startup, &mut profile)
        .map_err(|_| PreparationError::FormRejected)?;
    conduit_semantic_catalog::install_count_pipeline_catalogs(&mut startup, &mut profile)
        .map_err(|_| PreparationError::FormRejected)?;
    let checked = conduit_form::check_syntax_document(&syntax, &startup)
        .map_err(|_| PreparationError::FormRejected)?;
    conduit_form::expand_canonical_form(&checked, form_name, &profile)
        .map_err(|_| PreparationError::FormRejected)
}
