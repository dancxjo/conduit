//! Catalog assembly for checking the reviewed portable Form inventory.

pub(super) fn catalogs(
) -> Result<(conduit_form::StartupCatalog, conduit_form::ProfileCatalog), String> {
    let mut startup = conduit_signal::primary_signal_startup_catalog();
    let mut profile = conduit_signal::primary_signal_profile_catalog();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile)?;
    conduit_time::install_time_every_catalog(&mut startup, &mut profile)?;
    conduit_time::install_rhythm_catalog(&mut startup, &mut profile)?;
    conduit_time::install_historical_timeline_catalog(&mut startup, &mut profile)?;
    conduit_time::install_replay_source_catalog(&mut startup, &mut profile)?;
    conduit_time::install_replay_control_catalog(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_tick_presentation_catalog(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_timing_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_count_pipeline_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_flow_state_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_state_toggle_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_signal_garden_catalog(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_logic_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_math_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_quantity_mapping_catalog(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_layout_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_presentation_composition_catalogs(
        &mut startup,
        &mut profile,
    )?;
    conduit_semantic_catalog::install_graphics_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_graphics_presentation_catalog(&mut startup, &mut profile)?;
    conduit_presentation::install_bitmap_presentation_catalog(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_keyboard_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_input_semantic_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_button_indicator_catalogs(&mut startup, &mut profile)?;
    conduit_web::install_http_catalogs(&mut startup, &mut profile)?;
    conduit_web::install_json_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_recurrence_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_schedule_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_calendar_provider_catalogs(&mut startup, &mut profile)?;
    conduit_presentation::install_geometry_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_vision_catalogs(&mut startup, &mut profile)?;
    conduit_language::install_linguistics_catalogs(&mut startup, &mut profile)?;
    conduit_data::install_tabular_catalogs(&mut startup, &mut profile)?;
    conduit_data::install_finance_catalogs(&mut startup, &mut profile)?;
    conduit_data::install_measurement_window_catalog(&mut startup, &mut profile)?;
    conduit_data::install_measurement_summary_catalog(&mut startup, &mut profile)?;
    conduit_data::install_measurement_threshold_catalog(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_job_catalogs(&mut startup, &mut profile)?;
    conduit_net::install_application_network_catalogs(&mut startup, &mut profile)?;
    conduit_net::install_typed_record_catalogs(&mut startup, &mut profile)?;
    conduit_net::install_ordered_record_queue_catalog(&mut startup, &mut profile)?;
    conduit_net::install_record_transcript_catalog(&mut startup, &mut profile)?;
    conduit_net::install_record_delivery_status_catalog(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_robotics_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_robotics_structured_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_sound_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_education_catalogs(&mut startup, &mut profile)?;
    conduit_chat::install_messaging_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_generalized_input_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_timed_pattern_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_timed_button_attempt_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_sequence_normalization_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_template_storage_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_final_normalized_pattern_catalogs(
        &mut startup,
        &mut profile,
    )?;
    conduit_semantic_catalog::install_pattern_comparison_catalogs(&mut startup, &mut profile)?;
    startup.insert(conduit_form::KindSignature {
        kind: conduit_semantic_catalog::STRUCTURED_PRESENTATION_KIND.into(),
        startup_parameters: Vec::new(),
    })?;
    let comparison_presentation = conduit_semantic_catalog::structured_presentation_contract(
        conduit_semantic_catalog::PATTERN_COMPARISON_TYPE,
        &conduit_semantic_catalog::pattern_comparison_type(),
    );
    profile
        .insert(conduit_form::KindDefinition {
            kind_id: comparison_presentation.kind_id,
            kind_contract_revision: comparison_presentation.kind_contract_revision,
            inputs: comparison_presentation.inputs,
            outputs: comparison_presentation.outputs,
            configuration: Vec::new(),
        })
        .map_err(|error| error.to_string())?;
    conduit_alife::install_lenia_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_human_media_catalogs(&mut startup, &mut profile)?;
    conduit_chat::install_pool_chat_catalogs(&mut startup, &mut profile)?;
    conduit_net::install_network_bootstrap_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_reminder_catalogs(&mut startup, &mut profile)?;
    conduit_net::install_external_websocket_catalogs(&mut startup, &mut profile)?;
    conduit_chat::install_browser_chat_catalogs(&mut startup, &mut profile)?;
    conduit_tongues::install_research_catalogs(&mut startup, &mut profile)?;
    Ok((startup, profile))
}
