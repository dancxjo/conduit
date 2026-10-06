//! Catalog assembly owned by the canonical Plot editor entrance.

use conduit_plot::{ProfileCatalog, StartupCatalog};

use crate::PlotEditorError;

pub(crate) fn standard_catalogs() -> Result<(StartupCatalog, ProfileCatalog), PlotEditorError> {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_presentation::install_mask_plot_value_aliases(&mut startup)
        .map_err(PlotEditorError::Catalog)?;
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile)
        .map_err(PlotEditorError::Catalog)?;
    conduit_semantic_catalog::install_text_state_catalogs(&mut startup, &mut profile)
        .map_err(PlotEditorError::Catalog)?;
    conduit_text::install_morse_catalogs(&mut startup, &mut profile)
        .map_err(PlotEditorError::Catalog)?;
    conduit_net::install_typed_record_catalogs(&mut startup, &mut profile)
        .map_err(PlotEditorError::Catalog)?;
    conduit_net::install_record_temporal_catalogs(&mut startup, &mut profile)
        .map_err(PlotEditorError::Catalog)?;
    conduit_net::install_ordered_record_queue_catalog(&mut startup, &mut profile)
        .map_err(PlotEditorError::Catalog)?;
    conduit_time::install_time_every_catalog(&mut startup, &mut profile)
        .map_err(PlotEditorError::Catalog)?;
    conduit_time::install_rhythm_catalog(&mut startup, &mut profile)
        .map_err(PlotEditorError::Catalog)?;
    conduit_semantic_catalog::install_tick_presentation_catalog(&mut startup, &mut profile)
        .map_err(PlotEditorError::Catalog)?;
    conduit_semantic_catalog::install_pulse_presentation_catalog(&mut startup, &mut profile)
        .map_err(PlotEditorError::Catalog)?;
    conduit_semantic_catalog::install_rhythm_presentation_catalog(&mut startup, &mut profile)
        .map_err(PlotEditorError::Catalog)?;
    conduit_semantic_catalog::install_timing_catalogs(&mut startup, &mut profile)
        .map_err(PlotEditorError::Catalog)?;
    conduit_semantic_catalog::install_count_pipeline_catalogs(&mut startup, &mut profile)
        .map_err(PlotEditorError::Catalog)?;
    conduit_semantic_catalog::install_logic_catalogs(&mut startup, &mut profile)
        .map_err(PlotEditorError::Catalog)?;
    conduit_semantic_catalog::install_math_catalogs(&mut startup, &mut profile)
        .map_err(PlotEditorError::Catalog)?;
    conduit_semantic_catalog::install_quantity_mapping_catalog(&mut startup, &mut profile)
        .map_err(PlotEditorError::Catalog)?;
    conduit_semantic_catalog::install_quantity_info_catalog(&mut startup, &mut profile)
        .map_err(PlotEditorError::Catalog)?;
    conduit_semantic_catalog::install_normalized_quantity_catalog(&mut startup, &mut profile)
        .map_err(PlotEditorError::Catalog)?;
    conduit_semantic_catalog::install_layout_catalogs(&mut startup, &mut profile)
        .map_err(PlotEditorError::Catalog)?;
    conduit_semantic_catalog::install_presentation_composition_catalogs(&mut startup, &mut profile)
        .map_err(PlotEditorError::Catalog)?;
    conduit_semantic_catalog::install_graphics_catalogs(&mut startup, &mut profile)
        .map_err(PlotEditorError::Catalog)?;
    conduit_semantic_catalog::install_keyboard_catalogs(&mut startup, &mut profile)
        .map_err(PlotEditorError::Catalog)?;
    conduit_semantic_catalog::install_button_indicator_catalogs(&mut startup, &mut profile)
        .map_err(PlotEditorError::Catalog)?;
    conduit_semantic_catalog::install_indicator_presentation_catalog(&mut startup, &mut profile)
        .map_err(PlotEditorError::Catalog)?;
    conduit_semantic_catalog::install_input_semantic_catalogs(&mut startup, &mut profile)
        .map_err(PlotEditorError::Catalog)?;
    conduit_semantic_catalog::install_sound_catalogs(&mut startup, &mut profile)
        .map_err(PlotEditorError::Catalog)?;
    conduit_data::install_measurement_window_catalog(&mut startup, &mut profile)
        .map_err(PlotEditorError::Catalog)?;
    conduit_data::install_measurement_summary_catalog(&mut startup, &mut profile)
        .map_err(PlotEditorError::Catalog)?;
    conduit_data::install_measurement_threshold_catalog(&mut startup, &mut profile)
        .map_err(PlotEditorError::Catalog)?;
    conduit_data::install_measurement_plot_catalog(&mut startup, &mut profile)
        .map_err(PlotEditorError::Catalog)?;
    conduit_little_seismograph_fixture::install_little_seismograph_fixture_catalog(
        &mut startup,
        &mut profile,
    )
    .map_err(PlotEditorError::Catalog)?;
    Ok((startup, profile))
}

#[cfg(test)]
mod tests {
    #[test]
    fn standard_editor_catalog_checks_the_canonical_morse_network() {
        let source = include_str!("../../../../../plots/morse-network/main.conduit");
        let editor = crate::PlotEditor::from_source(
            "plots/morse-network/main.conduit".into(),
            source.into(),
        )
        .unwrap();
        assert!(
            editor.view().checked.source_document_id.is_some(),
            "{:?}",
            editor.view().checked.diagnostics
        );
    }

    #[test]
    fn standard_editor_catalog_checks_the_canonical_little_seismograph() {
        let source = include_str!("../../../../../plots/little-seismograph/main.conduit");
        let editor = crate::PlotEditor::from_source(
            "plots/little-seismograph/main.conduit".into(),
            source.into(),
        )
        .unwrap();
        assert!(
            editor
                .view()
                .checked
                .plots
                .iter()
                .any(|plot| plot.name == "little-seismograph-display"),
            "{:?}",
            editor.view().checked.diagnostics
        );
    }
}
