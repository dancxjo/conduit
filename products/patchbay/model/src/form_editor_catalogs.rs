//! Catalog assembly owned by the canonical Form editor entrance.

use conduit_form::{ProfileCatalog, StartupCatalog};

use crate::FormEditorError;

pub(crate) fn standard_catalogs() -> Result<(StartupCatalog, ProfileCatalog), FormEditorError> {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile)
        .map_err(FormEditorError::Catalog)?;
    conduit_time::install_time_every_catalog(&mut startup, &mut profile)
        .map_err(FormEditorError::Catalog)?;
    conduit_semantic_catalog::install_tick_presentation_catalog(&mut startup, &mut profile)
        .map_err(FormEditorError::Catalog)?;
    conduit_semantic_catalog::install_timing_catalogs(&mut startup, &mut profile)
        .map_err(FormEditorError::Catalog)?;
    conduit_semantic_catalog::install_count_pipeline_catalogs(&mut startup, &mut profile)
        .map_err(FormEditorError::Catalog)?;
    conduit_semantic_catalog::install_logic_catalogs(&mut startup, &mut profile)
        .map_err(FormEditorError::Catalog)?;
    conduit_semantic_catalog::install_math_catalogs(&mut startup, &mut profile)
        .map_err(FormEditorError::Catalog)?;
    conduit_semantic_catalog::install_layout_catalogs(&mut startup, &mut profile)
        .map_err(FormEditorError::Catalog)?;
    conduit_semantic_catalog::install_presentation_composition_catalogs(&mut startup, &mut profile)
        .map_err(FormEditorError::Catalog)?;
    conduit_semantic_catalog::install_graphics_catalogs(&mut startup, &mut profile)
        .map_err(FormEditorError::Catalog)?;
    conduit_semantic_catalog::install_keyboard_catalogs(&mut startup, &mut profile)
        .map_err(FormEditorError::Catalog)?;
    conduit_semantic_catalog::install_button_indicator_catalogs(&mut startup, &mut profile)
        .map_err(FormEditorError::Catalog)?;
    conduit_semantic_catalog::install_input_semantic_catalogs(&mut startup, &mut profile)
        .map_err(FormEditorError::Catalog)?;
    conduit_semantic_catalog::install_sound_catalogs(&mut startup, &mut profile)
        .map_err(FormEditorError::Catalog)?;
    Ok((startup, profile))
}
