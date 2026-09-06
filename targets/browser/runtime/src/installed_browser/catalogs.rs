//! Checked catalogs and recursive backs of the installed browser profile.
use super::{linguistics, quantity_output};

pub(crate) fn catalogs(
) -> Result<(conduit_form::StartupCatalog, conduit_form::ProfileCatalog), String> {
    catalogs_for_presentation(false)
}

pub(crate) fn catalogs_with_quantity_presentation(
) -> Result<(conduit_form::StartupCatalog, conduit_form::ProfileCatalog), String> {
    catalogs_for_presentation(true)
}

fn catalogs_for_presentation(
    quantity: bool,
) -> Result<(conduit_form::StartupCatalog, conduit_form::ProfileCatalog), String> {
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile)?;
    conduit_text::install_morse_catalogs(&mut startup, &mut profile)?;
    conduit_web::install_json_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_indicator_presentation_catalog(&mut startup, &mut profile)?;
    if quantity {
        conduit_language::install_linguistics_catalogs(&mut startup, &mut profile)?;
        quantity_output::install_catalogs(&mut startup, &mut profile)?;
    } else {
        linguistics::install_catalogs(&mut startup, &mut profile)?;
    }
    conduit_semantic_catalog::install_value_primitive_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_math_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_quantity_mapping_catalog(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_quantity_info_catalog(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_normalized_quantity_catalog(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_generalized_input_catalogs(&mut startup, &mut profile)?;
    super::pointer_selector::install_types(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_logic_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_timed_button_attempt_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_timing_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_timed_pattern_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_sequence_normalization_catalogs(&mut startup, &mut profile)?;
    conduit_time::install_time_every_catalog(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_count_pipeline_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_layout_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_keyboard_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_patchbay_presentation_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_button_indicator_catalogs(&mut startup, &mut profile)?;
    startup.insert(conduit_form::KindSignature {
        kind: conduit_semantic_catalog::BOOL_PRESENTATION_KIND.into(),
        startup_parameters: Vec::new(),
    })?;
    conduit_semantic_catalog::install_bool_presentation_catalog(&mut profile)?;
    Ok((startup, profile))
}

pub(crate) fn backs(
    startup: &conduit_form::StartupCatalog,
    profile: &conduit_form::ProfileCatalog,
) -> Result<conduit_form::CanonicalBackCatalog, String> {
    let mut backs = conduit_form::CanonicalBackCatalog::new();
    conduit_text::install_morse_backs(startup, profile, &mut backs)?;
    Ok(backs)
}
