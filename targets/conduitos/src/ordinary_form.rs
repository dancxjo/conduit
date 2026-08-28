//! Checking and expansion boundary for the ordinary text Form.

use crate::ordinary_plan::PreparationError;

pub(crate) fn checked_expanded_text_form(
    source: &str,
) -> Result<conduit_form::ExpandedCanonicalForm, PreparationError> {
    let syntax = conduit_form::parse_syntax_document(source);
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile)
        .map_err(|_| PreparationError::FormRejected)?;
    let checked = conduit_form::check_syntax_document(&syntax, &startup)
        .map_err(|_| PreparationError::FormRejected)?;
    conduit_form::expand_canonical_form(&checked, "conduitos-text-upper", &profile)
        .map_err(|_| PreparationError::FormRejected)
}
