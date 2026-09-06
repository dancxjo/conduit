//! Checked catalogs and recursive backs of the installed browser profile.
use super::{linguistics, quantity_output};
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PresentationProfile {
    Annotation,
    Quantity,
    NormalizedDurations,
    PatternComparison,
}

pub(crate) fn catalogs(
) -> Result<(conduit_form::StartupCatalog, conduit_form::ProfileCatalog), String> {
    catalogs_for_presentation(PresentationProfile::Annotation)
}

pub(crate) fn catalogs_for_presentation(
    presentation: PresentationProfile,
) -> Result<(conduit_form::StartupCatalog, conduit_form::ProfileCatalog), String> {
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile)?;
    conduit_text::install_morse_catalogs(&mut startup, &mut profile)?;
    conduit_web::install_json_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_resource_snapshot_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_indicator_presentation_catalog(&mut startup, &mut profile)?;
    match presentation {
        PresentationProfile::Annotation => {
            linguistics::install_catalogs(&mut startup, &mut profile)?
        }
        PresentationProfile::Quantity => {
            conduit_language::install_linguistics_catalogs(&mut startup, &mut profile)?;
            quantity_output::install_catalogs(&mut startup, &mut profile)?;
        }
        PresentationProfile::PatternComparison => {
            conduit_language::install_linguistics_catalogs(&mut startup, &mut profile)?;
            super::comparison_presentation::install_catalogs(&mut startup, &mut profile)?;
        }
        PresentationProfile::NormalizedDurations => {
            conduit_language::install_linguistics_catalogs(&mut startup, &mut profile)?;
            super::normalized_presentation::install_catalogs(&mut startup, &mut profile)?;
        }
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
    conduit_semantic_catalog::install_pattern_comparison_catalogs(&mut startup, &mut profile)?;
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
    if presentation == PresentationProfile::Quantity {
        startup.insert_value_kind_alias(
            "Scalar",
            conduit_core::kind_id(conduit_core::SCALAR_INFO_ID),
        )?;
        startup.insert_value_kind_alias(
            "Quantity",
            conduit_core::kind_id(conduit_core::QUANTITY_INFO_ID),
        )?;
    }
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
